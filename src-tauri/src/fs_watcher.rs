use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, mpsc};

struct WatcherState {
    watcher: RecommendedWatcher,
    paths: HashMap<PathBuf, RecursiveMode>,
}

static WATCHER: OnceLock<Mutex<WatcherState>> = OnceLock::new();

/// Clasifica un evento en "relevante" (afecta a la biblioteca) o "ignorable"
/// (accesos, metadatos, cierres). Solo los relevantes justifican un
/// `library-changed`.
fn is_relevant(event: &Event) -> bool {
    match &event.kind {
        EventKind::Create(_) => true,
        EventKind::Remove(_) => true,
        EventKind::Modify(kind) => match kind {
            ModifyKind::Name(_) => true,
            ModifyKind::Data(_) => true,
            ModifyKind::Any => true,
            // Metadata incluye accesos, cambios de permisos, timestamps: no
            // afectan a la biblioteca lógica.
            ModifyKind::Metadata(_) => false,
            _ => false,
        },
        EventKind::Access(_) => false,
        EventKind::Any => true,
        EventKind::Other => false,
    }
}

pub async fn start(app: AppHandle) -> Result<(), String> {
    if WATCHER.get().is_some() {
        return Ok(());
    }

    let (sender, mut receiver) = mpsc::unbounded_channel();
    let watcher = notify::recommended_watcher(move |result: notify::Result<Event>| match result {
        Ok(event) => {
            let _ = sender.send(event);
        }
        Err(error) => eprintln!("[fs-watcher] Error de vigilancia: {error}"),
    })
    .map_err(|error| error.to_string())?;

    WATCHER
        .set(Mutex::new(WatcherState {
            watcher,
            paths: HashMap::new(),
        }))
        .map_err(|_| "La vigilancia de archivos ya estaba iniciada".to_string())?;
    refresh().await?;

    tauri::async_runtime::spawn(async move {
        while let Some(first) = receiver.recv().await {
            let mut events = vec![first];
            tokio::time::sleep(Duration::from_millis(200)).await;
            while let Ok(event) = receiver.try_recv() {
                events.push(event);
            }

            let (renames, consumed_events) = rename_pairs(&events);
            // Solo consideramos "cambio" si hay eventos relevantes fuera de los
            // renombrados ya procesados, o si algún rename actualizó rutas.
            // Las rutas afectadas se pasan al sync para no reescanear raíces
            // ajenas al cambio (en especial, carpetas grandes ya cargadas).
            let mut changed = false;
            let mut changed_paths: Vec<PathBuf> = Vec::new();
            for (i, event) in events.iter().enumerate() {
                if consumed_events.contains(&i) {
                    continue;
                }
                if is_relevant(event) {
                    changed = true;
                    changed_paths.extend(event.paths.iter().cloned());
                }
            }

            for (old_path, new_path) in renames {
                changed_paths.push(old_path.clone());
                changed_paths.push(new_path.clone());
                match crate::commands::library::update_paths_after_rename(&old_path, &new_path)
                    .await
                {
                    Ok(updated) => changed |= updated,
                    Err(error) => eprintln!(
                        "[fs-watcher] No se pudo actualizar la ruta {} → {}: {error}",
                        old_path.display(),
                        new_path.display()
                    ),
                }
            }

            // Sin rutas cambiadas (eventos irrelevantes: accesos, metadatos) no
            // hay nada que resincronizar: antes se lanzaba un scan completo por
            // cada ráfaga, también por eventos que no afectan a la biblioteca.
            if !changed_paths.is_empty() {
                match crate::commands::library::sync_synced_folders_for_paths(&app, &changed_paths)
                    .await
                {
                    Ok(synced) => changed |= synced,
                    Err(error) => {
                        eprintln!("[fs-watcher] No se pudo sincronizar la biblioteca: {error}")
                    }
                }
            }
            if let Err(error) = refresh().await {
                eprintln!("[fs-watcher] No se pudieron actualizar las rutas vigiladas: {error}");
            }
            if changed && let Err(error) = app.emit("library-changed", ()) {
                eprintln!("[fs-watcher] No se notificó el cambio: {error}");
            }
        }
    });
    Ok(())
}

pub async fn refresh() -> Result<(), String> {
    let Some(state) = WATCHER.get() else {
        return Ok(());
    };
    let pool = crate::db::pool().await.map_err(|error| error.to_string())?;
    let roots: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM folders WHERE kind = 'synced' AND parent_id IS NULL AND path IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| error.to_string())?;
    let documents: Vec<String> =
        sqlx::query_scalar("SELECT path FROM documents WHERE in_library = 1 OR in_recent = 1")
            .fetch_all(pool)
            .await
            .map_err(|error| error.to_string())?;

    let mut desired = HashMap::new();
    let mut recursive_roots = Vec::new();
    for root in roots {
        let path = PathBuf::from(root);
        recursive_roots.push(path.clone());
        desired.insert(path.clone(), RecursiveMode::Recursive);
        if let Some(parent) = path.parent() {
            desired
                .entry(parent.to_path_buf())
                .or_insert(RecursiveMode::NonRecursive);
        }
    }
    for document in documents {
        let document_path = Path::new(&document);
        if recursive_roots
            .iter()
            .any(|root| document_path.starts_with(root))
        {
            continue;
        }
        let Some(parent) = document_path.parent() else {
            continue;
        };
        desired
            .entry(parent.to_path_buf())
            .or_insert(RecursiveMode::NonRecursive);
        if let Some(grandparent) = parent.parent() {
            desired
                .entry(grandparent.to_path_buf())
                .or_insert(RecursiveMode::NonRecursive);
        }
    }

    let mut state = state.lock().await;
    let removed: Vec<PathBuf> = state
        .paths
        .keys()
        .filter(|path| !desired.contains_key(*path))
        .cloned()
        .collect();
    for path in removed {
        if let Err(error) = state.watcher.unwatch(&path) {
            eprintln!(
                "[fs-watcher] No se pudo dejar de vigilar {}: {error}",
                path.display()
            );
        }
        state.paths.remove(&path);
    }

    for (path, mode) in desired {
        if state.paths.get(&path) == Some(&mode) {
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        if state.paths.contains_key(&path) {
            if let Err(error) = state.watcher.unwatch(&path) {
                eprintln!(
                    "[fs-watcher] No se pudo cambiar la vigilancia de {}: {error}",
                    path.display()
                );
            }
            state.paths.remove(&path);
        }
        match state.watcher.watch(&path, mode) {
            Ok(()) => {
                state.paths.insert(path, mode);
            }
            Err(error) => eprintln!(
                "[fs-watcher] No se pudo vigilar {}: {error}",
                path.display()
            ),
        }
    }
    Ok(())
}

fn rename_pairs(events: &[Event]) -> (Vec<(PathBuf, PathBuf)>, HashSet<usize>) {
    let mut pairs = Vec::new();
    let mut consumed = HashSet::new();
    let mut from_paths = VecDeque::new();

    for (index, event) in events.iter().enumerate() {
        let EventKind::Modify(ModifyKind::Name(mode)) = &event.kind else {
            continue;
        };
        match mode {
            RenameMode::Both | RenameMode::Any if event.paths.len() == 2 => {
                pairs.push((event.paths[0].clone(), event.paths[1].clone()));
                consumed.insert(index);
            }
            RenameMode::From if event.paths.len() == 1 => {
                from_paths.push_back((index, event.paths[0].clone()));
            }
            RenameMode::To if event.paths.len() == 1 => {
                if let Some((from_index, old_path)) = from_paths.pop_front() {
                    pairs.push((old_path, event.paths[0].clone()));
                    consumed.insert(from_index);
                    consumed.insert(index);
                }
            }
            _ => {}
        }
    }

    (pairs, consumed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_single_event_with_both_rename_paths() {
        let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(PathBuf::from("/docs/old.pdf"))
            .add_path(PathBuf::from("/docs/new.pdf"));

        let (pairs, consumed) = rename_pairs(&[event]);

        assert_eq!(
            pairs,
            vec![(
                PathBuf::from("/docs/old.pdf"),
                PathBuf::from("/docs/new.pdf")
            )]
        );
        assert_eq!(consumed, HashSet::from([0]));
    }

    #[test]
    fn pairs_separate_from_and_to_rename_events() {
        let from = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::From)))
            .add_path(PathBuf::from("/docs/old.pdf"));
        let to = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::To)))
            .add_path(PathBuf::from("/docs/new.pdf"));

        let (pairs, consumed) = rename_pairs(&[from, to]);

        assert_eq!(
            pairs,
            vec![(
                PathBuf::from("/docs/old.pdf"),
                PathBuf::from("/docs/new.pdf")
            )]
        );
        assert_eq!(consumed, HashSet::from([0, 1]));
    }

    #[test]
    fn access_and_metadata_events_are_ignored() {
        let access = Event::new(EventKind::Access(notify::event::AccessKind::Read))
            .add_path(PathBuf::from("/docs/file.pdf"));
        let metadata = Event::new(EventKind::Modify(ModifyKind::Metadata(
            notify::event::MetadataKind::AccessTime,
        )))
        .add_path(PathBuf::from("/docs/file.pdf"));

        assert!(!is_relevant(&access));
        assert!(!is_relevant(&metadata));
    }

    #[test]
    fn create_and_data_modifications_are_relevant() {
        let create = Event::new(EventKind::Create(notify::event::CreateKind::File))
            .add_path(PathBuf::from("/docs/file.pdf"));
        let data = Event::new(EventKind::Modify(ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(PathBuf::from("/docs/file.pdf"));

        assert!(is_relevant(&create));
        assert!(is_relevant(&data));
    }
}
