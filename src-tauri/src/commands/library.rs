use crate::db::{
    self,
    models::{FolderRecord, LibrarySnapshot},
};
use crate::ids::new_id;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

static SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static LAST_SYNC_AT: tokio::sync::Mutex<Option<std::time::Instant>> =
    tokio::sync::Mutex::const_new(None);
static SYNC_DIRTY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const MIN_SYNC_INTERVAL_MS: u64 = 1000;
/// Pausa entre un sync que terminó con el flag `SYNC_DIRTY` y el re-pase, para
/// no encadenar escaneos completos sin descanso.
const SYNC_RERUN_COOLDOWN_MS: u64 = 2000;

/// Fila de `folders` tal como se guarda en un snapshot de papelera.
type FolderSnapshotRow = (
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    bool,
    i64,
    i64,
);

/// Fila completa de `documents` tal como se guarda en un snapshot de papelera.
#[derive(sqlx::FromRow)]
struct DocumentSnapshotRow {
    id: String,
    path: String,
    title: String,
    kind: String,
    pages: i64,
    size_bytes: Option<i64>,
    hash: Option<String>,
    category_id: Option<String>,
    category_ids: Option<String>,
    last_page: i64,
    last_zoom: f64,
    last_opened: Option<i64>,
    in_recent: bool,
    in_library: bool,
    created_at: i64,
    view_json: String,
    view_state: String,
    position: i64,
}

fn kind_from_extension(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md") | Some("markdown") => "markdown",
        Some("typ") => "typst",
        Some("epub") => "epub",
        _ => "pdf",
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn like_prefix(path: &str) -> String {
    path.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

// ─────────────────────────────────────────────────────────────────────────────
// Sync de carpetas vinculadas
// ─────────────────────────────────────────────────────────────────────────────

/// El sync se puede cancelar: se consulta entre raíces y cada cierto número de
/// directorios, así que la transacción en curso termina pero no se empieza otra.
static SYNC_CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Impide que el watcher, el polling o un re-pase sucio inicien otro sync
/// después de que el usuario lo haya cancelado.
static SYNC_PAUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/** Cada cuántos elementos se comprueba la cancelación dentro de una raíz. */
const SYNC_CANCEL_CHECK_EVERY: usize = 64;

fn sync_cancelled() -> bool {
    SYNC_CANCEL.load(std::sync::atomic::Ordering::SeqCst)
}

fn sync_paused() -> bool {
    SYNC_PAUSED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Progreso del sync hacia la UI (`sync-progress`).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncProgress {
    phase: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed: Option<bool>,
}

fn emit_sync_progress(app: &tauri::AppHandle, progress: SyncProgress) {
    let _ = app.emit("sync-progress", progress);
}

pub async fn sync_synced_folders_throttled(app: &tauri::AppHandle) -> Result<bool, String> {
    sync_throttled(app, None).await
}

/// Variante del watcher: solo se resincronizan las raíces afectadas por las
/// rutas cambiadas. Evita reescanear carpetas grandes ajenas al cambio (la
/// causa de que una carpeta «ya cargada» se estuviera sincronizando sin parar).
pub async fn sync_synced_folders_for_paths(
    app: &tauri::AppHandle,
    paths: &[std::path::PathBuf],
) -> Result<bool, String> {
    sync_throttled(app, Some(paths)).await
}

async fn sync_throttled(
    app: &tauri::AppHandle,
    paths: Option<&[std::path::PathBuf]>,
) -> Result<bool, String> {
    if sync_paused() {
        return Ok(false);
    }
    let mut last_sync_at = LAST_SYNC_AT.lock().await;
    let now = std::time::Instant::now();
    if last_sync_at.is_some_and(|last| {
        now.duration_since(last) < std::time::Duration::from_millis(MIN_SYNC_INTERVAL_MS)
    }) {
        return Ok(false);
    }
    *last_sync_at = Some(now);
    drop(last_sync_at);

    // try_lock: si otro sync está corriendo (típicamente uno disparado por el
    // comando manual), no bloqueamos el hilo del watcher. Marcamos el flag y
    // volvemos; el sync activo hará un pase extra al terminar.
    match SYNC_LOCK.try_lock() {
        Ok(guard) => {
            let result = sync_synced_folders_inner(app, paths).await;
            drop(guard);
            schedule_dirty_rerun(app);
            emit_sync_result(app, &result);
            result
        }
        Err(_) => {
            SYNC_DIRTY.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(false)
        }
    }
}

/// Re-pase por `SYNC_DIRTY`, con enfriamiento.
///
/// Si durante un sync siguen llegando eventos del watcher, encadenar otro
/// escaneo completo inmediatamente dejaba la biblioteca escaneando sin descanso
/// en carpetas con actividad continua (p. ej. iCloud Drive). Se espera un
/// enfriamiento y se hace **un solo** pase completo: los eventos que lleguen
/// durante ese pase volverán a marcar el flag y provocarán otro re-pase, pero
/// siempre espaciado por el enfriamiento.
fn schedule_dirty_rerun(app: &tauri::AppHandle) {
    if sync_paused() || !SYNC_DIRTY.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(SYNC_RERUN_COOLDOWN_MS)).await;
        if sync_paused() {
            return;
        }
        let _ = sync_synced_folders_force(&handle).await;
    });
}

async fn sync_synced_folders_force(app: &tauri::AppHandle) -> Result<bool, String> {
    if sync_paused() {
        return Ok(false);
    }
    let _guard = SYNC_LOCK.lock().await;
    // El reloj del throttle se mueve también aquí: un pase forzado (el comando
    // manual o el re-pase por `SYNC_DIRTY`) ya ha hecho el trabajo, así que el
    // watcher no tiene por qué volver a escanear en el mismo segundo.
    *LAST_SYNC_AT.lock().await = Some(std::time::Instant::now());
    let result = sync_synced_folders_inner(app, None).await;
    emit_sync_result(app, &result);
    result
}

fn emit_sync_result(app: &tauri::AppHandle, result: &Result<bool, String>) {
    match result {
        Ok(changed) => emit_sync_progress(
            app,
            SyncProgress {
                phase: "done",
                path: None,
                message: None,
                changed: Some(*changed),
            },
        ),
        Err(error) => emit_sync_progress(
            app,
            SyncProgress {
                phase: "error",
                path: None,
                message: Some(error.clone()),
                changed: None,
            },
        ),
    }
}

/// Arranca el sync y vuelve enseguida: con miles de archivos el `invoke` se
/// quedaba esperando y la app parecía colgada. El progreso llega por
/// `sync-progress` y la cancelación por `cancel_sync`.
#[tauri::command]
pub async fn sync_synced_folders(app: tauri::AppHandle) -> Result<(), String> {
    // Este comando es también la acción explícita de reanudar.
    SYNC_PAUSED.store(false, std::sync::atomic::Ordering::SeqCst);
    SYNC_CANCEL.store(false, std::sync::atomic::Ordering::SeqCst);
    if let Err(error) = app.emit("sync-paused", false) {
        eprintln!("[library] No se pudo emitir sync-paused: {error}");
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = sync_synced_folders_force(&handle).await {
            eprintln!("[library] El sync falló: {error}");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn cancel_sync(app: tauri::AppHandle) -> Result<(), String> {
    SYNC_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
    SYNC_CANCEL.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Err(error) = app.emit("sync-paused", true) {
        eprintln!("[library] No se pudo emitir sync-paused: {error}");
    }
    Ok(())
}

#[tauri::command]
pub async fn is_sync_paused() -> Result<bool, String> {
    Ok(sync_paused())
}

async fn sync_synced_folders_inner(
    app: &tauri::AppHandle,
    paths: Option<&[std::path::PathBuf]>,
) -> Result<bool, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    sync_synced_folders_with_pool(pool, Some(app), paths).await
}

async fn sync_synced_folders_with_pool(
    pool: &sqlx::SqlitePool,
    app: Option<&tauri::AppHandle>,
    paths: Option<&[std::path::PathBuf]>,
) -> Result<bool, String> {
    if sync_paused() {
        return Ok(false);
    }
    // Cada sync empieza con el flag limpio: la cancelación afecta al sync en
    // curso, no a todos los siguientes. Se limpia aquí y no en el comando
    // porque los syncs del watcher, el poll periódico y el re-pase por
    // `SYNC_DIRTY` entran por este camino, y si no se limpiara, cancelar una vez
    // los dejaba a todos saliendo al instante hasta reiniciar la app.
    SYNC_CANCEL.store(false, std::sync::atomic::Ordering::SeqCst);
    match paths {
        Some(paths) => sync_roots_containing(pool, app, paths).await,
        None => sync_all_roots(pool, app).await,
    }
}

fn report(app: Option<&tauri::AppHandle>, progress: SyncProgress) {
    if let Some(app) = app {
        emit_sync_progress(app, progress);
    }
}

/// Sincroniza todas las raíces vinculadas. Del `AppHandle` solo hace falta para
/// emitir el progreso, así que los tests la llaman con `None` sobre un árbol
/// temporal y un pool en memoria.
///
/// Cada raíz se escribe en **una sola transacción** (`sync_root`): SQLite
/// serializa las escrituras, así que una transacción por archivo multiplicaba
/// por seis los commits de una biblioteca grande.
async fn sync_all_roots(
    pool: &sqlx::SqlitePool,
    app: Option<&tauri::AppHandle>,
) -> Result<bool, String> {
    let (roots, ignored_paths) = synced_roots(pool).await?;
    let mut changed = false;
    for (root_id, folder_path) in roots {
        if sync_cancelled() {
            break;
        }
        changed |= sync_one_root(pool, app, &root_id, &folder_path, &ignored_paths).await?;
    }
    Ok(changed)
}

/// Sincroniza únicamente las raíces que contienen alguno de los `changed_paths`.
///
/// El watcher sabe qué rutas cambiaron: volver a escanear todas las raíces (y en
/// especial una carpeta grande ya cargada) por un cambio en otra era la causa de
/// que la biblioteca se resincronizara sin parar en carpetas con actividad
/// continua (p. ej. iCloud Drive).
async fn sync_roots_containing(
    pool: &sqlx::SqlitePool,
    app: Option<&tauri::AppHandle>,
    changed_paths: &[std::path::PathBuf],
) -> Result<bool, String> {
    if changed_paths.is_empty() {
        return sync_all_roots(pool, app).await;
    }
    let (roots, ignored_paths) = synced_roots(pool).await?;
    let mut changed = false;
    for (root_id, folder_path) in roots {
        if sync_cancelled() {
            break;
        }
        let Ok(root_path) = std::fs::canonicalize(&folder_path) else {
            // Raíz inaccesible: se desvincula igual que en el sync completo.
            remove_root_subtree(pool, &root_id).await?;
            changed = true;
            continue;
        };
        let affected = changed_paths.iter().any(|changed| {
            let canonical = std::fs::canonicalize(changed).unwrap_or_else(|_| changed.clone());
            canonical.starts_with(&root_path)
        });
        if !affected {
            continue;
        }
        changed |= sync_one_root(pool, app, &root_id, &folder_path, &ignored_paths).await?;
    }
    Ok(changed)
}

/// Raíces sincronizadas y rutas ignoradas, para un pase de sync.
async fn synced_roots(
    pool: &sqlx::SqlitePool,
) -> Result<(Vec<(String, String)>, Vec<std::path::PathBuf>), String> {
    let roots: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, path FROM folders WHERE kind = 'synced' AND parent_id IS NULL AND path IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let ignored_paths: Vec<std::path::PathBuf> =
        sqlx::query_scalar::<_, String>("SELECT path FROM ignored_synced_paths")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
    Ok((roots, ignored_paths))
}

/// Sincroniza una única raíz. Comparte el cuerpo entre el pase completo y el
/// pase acotado a las rutas que cambió el watcher.
async fn sync_one_root(
    pool: &sqlx::SqlitePool,
    app: Option<&tauri::AppHandle>,
    root_id: &str,
    folder_path: &str,
    ignored_paths: &[std::path::PathBuf],
) -> Result<bool, String> {
    report(
        app,
        SyncProgress {
            phase: "scan",
            path: Some(folder_path.to_string()),
            message: None,
            changed: None,
        },
    );
    let root_path = match std::fs::canonicalize(folder_path) {
        Ok(path) => path,
        Err(error) => {
            eprintln!(
                "[library] No se puede acceder a la carpeta vinculada {folder_path}: {error}"
            );
            // La raíz ya no existe o no es accesible. Desvincular sus
            // documentos y eliminar el subárbol de carpetas para que no
            // queden fantasmas.
            remove_root_subtree(pool, root_id).await?;
            return Ok(true);
        }
    };
    let scan_root = root_path.clone();
    let (mut directories, mut files_by_directory) =
        match tokio::task::spawn_blocking(move || collect_folder_contents(&scan_root)).await {
            Ok(Ok(contents)) => contents,
            Ok(Err(error)) => {
                eprintln!(
                    "[library] No se pudo leer la carpeta sincronizada {folder_path}: {error}"
                );
                return Ok(false);
            }
            Err(error) => {
                eprintln!("[library] Falló la lectura de la carpeta sincronizada: {error}");
                return Ok(false);
            }
        };
    // La raíz vinculada no se filtra nunca: es el ancla del recorrido y, si su
    // propia ruta quedara marcada como ignorada (una fila obsoleta, o el borrado
    // de la carpeta antes de restaurarla), el sync daría por inexistente todo su
    // subárbol y lo borraría de la biblioteca.
    directories
        .retain(|directory| directory == &root_path || !is_ignored_path(directory, ignored_paths));
    files_by_directory.retain(|directory, files| {
        let directory_path = std::path::Path::new(directory);
        if is_ignored_path(directory_path, ignored_paths) {
            return false;
        }
        files.retain(|file| !is_ignored_path(std::path::Path::new(file), ignored_paths));
        !files.is_empty()
    });
    // Los padres se procesan antes que los hijos.
    directories.sort_by_key(|path| path.components().count());

    sync_root(pool, root_id, &root_path, &directories, &files_by_directory).await
}

/// Desvincula los documentos de un subárbol de carpetas y lo elimina.
async fn remove_root_subtree(pool: &sqlx::SqlitePool, root_id: &str) -> Result<(), String> {
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let child_folders = folder_subtree(&mut transaction, root_id).await?;
    let mut affected_docs = std::collections::HashSet::new();
    for folder in &child_folders {
        let docs: Vec<String> =
            sqlx::query_scalar("SELECT doc_id FROM folder_docs WHERE folder_id = ?")
                .bind(folder)
                .fetch_all(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        affected_docs.extend(docs);
    }
    for folder in &child_folders {
        sqlx::query("DELETE FROM folder_docs WHERE folder_id = ?")
            .bind(folder)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }
    // Después de borrar los vínculos: `detach_documents` solo suelta los
    // documentos que ya no estén en ninguna carpeta.
    detach_documents(&mut transaction, &affected_docs).await?;
    for folder in child_folders.iter().rev() {
        sqlx::query("DELETE FROM folders WHERE id = ?")
            .bind(folder)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }
    transaction.commit().await.map_err(|e| e.to_string())
}

/// Una carpeta y todo su subárbol.
async fn folder_subtree(
    conn: &mut sqlx::SqliteConnection,
    folder_id: &str,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar(
        "WITH RECURSIVE folder_tree(id) AS ( \
           SELECT id FROM folders WHERE id = ? \
           UNION ALL \
           SELECT child.id FROM folders child \
           JOIN folder_tree parent ON child.parent_id = parent.id \
         ) \
         SELECT id FROM folder_tree",
    )
    .bind(folder_id)
    .fetch_all(conn)
    .await
    .map_err(|e| e.to_string())
}

/// Saca del índice de la biblioteca los documentos indicados, respetando los que
/// sigan vinculados a otra carpeta.
async fn detach_documents(
    conn: &mut sqlx::SqliteConnection,
    doc_ids: &std::collections::HashSet<String>,
) -> Result<(), String> {
    for doc_id in doc_ids {
        sqlx::query(
            "UPDATE documents SET in_library = 0 WHERE id = ? \
             AND NOT EXISTS (SELECT 1 FROM folder_docs WHERE doc_id = ?)",
        )
        .bind(doc_id)
        .bind(doc_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Sincroniza una raíz completa dentro de **una única transacción**.
async fn sync_root(
    pool: &sqlx::SqlitePool,
    root_id: &str,
    root_path: &std::path::Path,
    directories: &[std::path::PathBuf],
    files_by_directory: &std::collections::HashMap<String, Vec<String>>,
) -> Result<bool, String> {
    let root = root_path.to_string_lossy().into_owned();
    // Contar páginas parsea el PDF entero. Se resuelve antes de abrir la
    // transacción para no retener el bloqueo de escritura de SQLite mientras se
    // leen cientos de PDFs.
    let pending = collect_pending_files(pool, directories, files_by_directory).await?;

    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let mut cancelled = false;
    let mut changed = false;
    let mut folder_ids = std::collections::HashMap::from([(root.clone(), root_id.to_string())]);
    let mut all_synced_paths: std::collections::HashSet<String> =
        std::collections::HashSet::from([root.clone()]);

    // Una sola pasada por `directories` para las dos tareas: crear la carpeta de
    // cada directorio y conciliar sus documentos. `directories` viene ordenado
    // por profundidad, así que cuando toca un directorio su carpeta superior ya
    // tiene id asignado.
    for (index, directory) in directories.iter().enumerate() {
        if index % SYNC_CANCEL_CHECK_EVERY == 0 && sync_cancelled() {
            cancelled = true;
            break;
        }
        let directory_path = directory.to_string_lossy().into_owned();

        // 1. Subcarpetas nuevas, renombradas o movidas de sitio. La raíz no se
        //    toca: es la fila que se está sincronizando.
        if directory != root_path {
            all_synced_paths.insert(directory_path.clone());

            let existing: Option<(String, Option<String>, String)> = sqlx::query_as(
                "SELECT id, parent_id, name FROM folders WHERE kind = 'synced' AND path = ?",
            )
            .bind(&directory_path)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;

            let parent_path = directory
                .parent()
                .ok_or("La ruta de subcarpeta no tiene padre")?;
            let parent = parent_path.to_string_lossy().into_owned();
            let parent_id = folder_ids
                .get(&parent)
                .ok_or("No se encontró la carpeta superior sincronizada")?
                .clone();
            let name = directory
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("Carpeta")
                .to_string();

            match existing {
                // Si la carpeta ya existía pero cambió de padre o de nombre,
                // actualizarla. Esto cubre el caso de mover una subcarpeta
                // dentro del árbol sincronizado.
                Some((id, old_parent, old_name)) => {
                    if old_parent.as_deref() != Some(parent_id.as_str()) || old_name != name {
                        sqlx::query("UPDATE folders SET parent_id = ?, name = ? WHERE id = ?")
                            .bind(&parent_id)
                            .bind(&name)
                            .bind(&id)
                            .execute(&mut *transaction)
                            .await
                            .map_err(|e| e.to_string())?;
                        changed = true;
                    }
                    folder_ids.insert(directory_path.clone(), id);
                }
                None => {
                    let record = add_folder_inner(
                        &mut transaction,
                        name,
                        "synced".into(),
                        Some(parent_id),
                        Some(directory_path.clone()),
                        false,
                    )
                    .await?;
                    folder_ids.insert(directory_path.clone(), record.id);
                    changed = true;
                }
            }
        }

        // 2. Documentos del directorio: añadir los nuevos o modificados y
        //    desvincular los que ya no están en disco.
        let folder_id = folder_ids
            .get(&directory_path)
            .ok_or("No se encontró el destino de sincronización")?
            .clone();
        let expected: std::collections::HashSet<&String> = files_by_directory
            .get(&directory_path)
            .map(|files| files.iter().collect())
            .unwrap_or_default();

        for (file_index, file) in pending
            .get(&directory_path)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if file_index.is_multiple_of(SYNC_CANCEL_CHECK_EVERY) && sync_cancelled() {
                cancelled = true;
                break;
            }
            changed |= apply_pending_file(&mut transaction, &folder_id, file).await?;
        }
        if cancelled {
            break;
        }

        let existing: Vec<(String, String)> = sqlx::query_as(
            "SELECT documents.id, documents.path FROM folder_docs \
             JOIN documents ON documents.id = folder_docs.doc_id \
             WHERE folder_docs.folder_id = ?",
        )
        .bind(&folder_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
        for (doc_id, path) in existing {
            if expected.contains(&path) {
                continue;
            }
            detach_synced_document(&mut transaction, &folder_id, &doc_id).await?;
            changed = true;
        }
    }

    // 3. Carpetas del árbol sincronizado que ya no existen en disco. Con el sync
    //    cancelado a medias no se toca nada: `all_synced_paths` estaría
    //    incompleto y borraríamos carpetas que sí existen.
    let stale_for_root: Vec<(String, String)> = if cancelled {
        Vec::new()
    } else {
        // Prefiltro en SQL: el `LIKE` es un superconjunto de `starts_with`, así
        // que puede traer de más pero nunca de menos, y evita materializar las
        // carpetas de todas las raíces en cada sync.
        let candidates: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, path FROM folders WHERE kind = 'synced' AND parent_id IS NOT NULL \
             AND path IS NOT NULL AND path LIKE ? || '%' ESCAPE '\\'",
        )
        .bind(like_prefix(&root))
        .fetch_all(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
        candidates
            .into_iter()
            .filter(|(_, path)| {
                std::path::Path::new(path).starts_with(root_path)
                    && !all_synced_paths.contains(path)
            })
            .collect()
    };
    if !stale_for_root.is_empty() {
        let mut affected_docs = std::collections::HashSet::new();
        for (id, _) in &stale_for_root {
            let child_docs: Vec<String> =
                sqlx::query_scalar("SELECT doc_id FROM folder_docs WHERE folder_id = ?")
                    .bind(id)
                    .fetch_all(&mut *transaction)
                    .await
                    .map_err(|e| e.to_string())?;
            affected_docs.extend(child_docs);
            sqlx::query("DELETE FROM folder_docs WHERE folder_id = ?")
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        }
        detach_documents(&mut transaction, &affected_docs).await?;
        for (id, _) in &stale_for_root {
            sqlx::query("DELETE FROM folders WHERE id = ?")
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        }
        changed = true;
    }

    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(changed)
}

/// Un archivo que hay que añadir o actualizar, con el recuento de páginas ya
/// resuelto.
struct PendingFile {
    path: String,
    size: i64,
    modified: i64,
    pages: i64,
    device_id: Option<i64>,
    inode: Option<i64>,
}

fn file_identity(metadata: &std::fs::Metadata) -> (Option<i64>, Option<i64>) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return (Some(metadata.dev() as i64), Some(metadata.ino() as i64));
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        (None, None)
    }
}

/// Archivos de la raíz que cambiaron desde el último sync, `directorio → archivos`.
async fn collect_pending_files(
    pool: &sqlx::SqlitePool,
    directories: &[std::path::PathBuf],
    files_by_directory: &std::collections::HashMap<String, Vec<String>>,
) -> Result<std::collections::HashMap<String, Vec<PendingFile>>, String> {
    let mut pending: std::collections::HashMap<String, Vec<PendingFile>> =
        std::collections::HashMap::new();
    let mut checked = 0usize;
    for directory in directories {
        let directory_path = directory.to_string_lossy().into_owned();
        let Some(expected) = files_by_directory.get(&directory_path) else {
            continue;
        };
        let linked: std::collections::HashSet<String> = sqlx::query_scalar(
            "SELECT documents.path FROM folder_docs \
             JOIN documents ON documents.id = folder_docs.doc_id \
             JOIN folders ON folders.id = folder_docs.folder_id \
             WHERE folders.path = ?",
        )
        .bind(&directory_path)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();

        for path in expected {
            checked += 1;
            if checked.is_multiple_of(SYNC_CANCEL_CHECK_EVERY) && sync_cancelled() {
                return Ok(pending);
            }
            let metadata = match std::fs::metadata(path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    eprintln!("[library] No se pudo comprobar {path}: {error}");
                    continue;
                }
            };
            let size = metadata.len() as i64;
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or(0);
            let (device_id, inode) = file_identity(&metadata);
            let state: Option<(i64, i64, Option<String>, Option<i64>, Option<i64>)> =
                sqlx::query_as(
                    "SELECT size_bytes, modified_at, error, device_id, inode \
                 FROM synced_file_state WHERE path = ?",
                )
                .bind(path)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;
            let up_to_date = state.as_ref().is_some_and(
                |(old_size, old_modified, error, old_device, old_inode)| {
                    *old_size == size
                        && *old_modified == modified
                        && *old_device == device_id
                        && *old_inode == inode
                        && (error.is_some() || linked.contains(path))
                },
            );
            if up_to_date {
                continue;
            }
            let pages = match resolve_pages(pool, path).await {
                Ok(pages) => pages,
                // El archivo desapareció entre el escaneo y la lectura: el
                // siguiente sync lo confirmará.
                Err(error) => {
                    eprintln!("[library] No se pudo comprobar {path}: {error}");
                    continue;
                }
            };
            pending
                .entry(directory_path.clone())
                .or_default()
                .push(PendingFile {
                    path: path.clone(),
                    size,
                    modified,
                    pages,
                    device_id,
                    inode,
                });
        }
    }
    Ok(pending)
}

/// Escribe en la base el resultado de sincronizar un archivo.
async fn apply_pending_file(
    conn: &mut sqlx::SqliteConnection,
    folder_id: &str,
    file: &PendingFile,
) -> Result<bool, String> {
    rebind_moved_file(conn, &file.path, file.device_id, file.inode).await?;
    add_file_inner(conn, &file.path, Some(folder_id), file.pages).await?;
    sqlx::query(
        "INSERT INTO synced_file_state \
         (path, size_bytes, modified_at, error, device_id, inode) \
         VALUES (?, ?, ?, NULL, ?, ?) ON CONFLICT(path) DO UPDATE SET \
         size_bytes = excluded.size_bytes, modified_at = excluded.modified_at, \
         error = NULL, device_id = excluded.device_id, inode = excluded.inode",
    )
    .bind(&file.path)
    .bind(file.size)
    .bind(file.modified)
    .bind(file.device_id)
    .bind(file.inode)
    .execute(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    Ok(true)
}

async fn rebind_moved_file(
    conn: &mut sqlx::SqliteConnection,
    new_path: &str,
    device_id: Option<i64>,
    inode: Option<i64>,
) -> Result<(), String> {
    let (Some(device_id), Some(inode)) = (device_id, inode) else {
        return Ok(());
    };
    let old_path: Option<String> = sqlx::query_scalar(
        "SELECT path FROM synced_file_state \
         WHERE device_id = ? AND inode = ? AND path != ? LIMIT 1",
    )
    .bind(device_id)
    .bind(inode)
    .bind(new_path)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    let Some(old_path) = old_path else {
        return Ok(());
    };

    let old_doc: Option<String> = sqlx::query_scalar("SELECT id FROM documents WHERE path = ?")
        .bind(&old_path)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(doc_id) = old_doc {
        let title = std::path::Path::new(new_path)
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("Documento.pdf");
        sqlx::query("UPDATE documents SET path = ?, title = ? WHERE id = ?")
            .bind(new_path)
            .bind(title)
            .bind(doc_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    sqlx::query("UPDATE synced_file_state SET path = ? WHERE path = ?")
        .bind(new_path)
        .bind(old_path)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Páginas de un PDF: reutiliza el recuento si ya hay un documento con el mismo
/// tamaño y, si no, parsea el archivo. Un PDF corrupto o demasiado grande no
/// impide añadirlo: se indexa con 0 páginas y pdf.js corrige el total al abrirlo
/// (`update_document_pages`). Nunca se llama dentro de una transacción.
async fn resolve_pages(pool: &sqlx::SqlitePool, path: &str) -> Result<i64, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len() as i64;
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT pages FROM documents WHERE path = ? AND size_bytes = ?")
            .bind(path)
            .bind(size)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    if let Some(pages) = existing {
        // `0` significa «aún no se sabe»: no lo cacheamos como definitivo.
        if pages > 0 {
            return Ok(pages);
        }
    }
    let count_path = path.to_string();
    match tokio::task::spawn_blocking(move || crate::commands::documents::count_pages(&count_path))
        .await
    {
        Ok(Ok(pages)) => Ok(pages),
        Ok(Err(error)) => {
            eprintln!("[library] No se pudieron contar las páginas de {path}: {error}");
            Ok(0)
        }
        Err(error) => {
            eprintln!("[library] No se pudieron contar las páginas de {path}: {error}");
            Ok(0)
        }
    }
}

async fn detach_synced_document(
    conn: &mut sqlx::SqliteConnection,
    folder_id: &str,
    doc_id: &str,
) -> Result<(), String> {
    let old_position: Option<i64> =
        sqlx::query_scalar("SELECT position FROM folder_docs WHERE folder_id = ? AND doc_id = ?")
            .bind(folder_id)
            .bind(doc_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM folder_docs WHERE folder_id = ? AND doc_id = ?")
        .bind(folder_id)
        .bind(doc_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(position) = old_position {
        sqlx::query(
            "UPDATE folder_docs SET position = position - 1 \
             WHERE folder_id = ? AND position > ?",
        )
        .bind(folder_id)
        .bind(position)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }
    sqlx::query(
        "UPDATE documents SET in_library = 0 WHERE id = ? \
         AND NOT EXISTS (SELECT 1 FROM folder_docs WHERE doc_id = ?)",
    )
    .bind(doc_id)
    .bind(doc_id)
    .execute(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Listado y comandos básicos
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_library() -> Result<LibrarySnapshot, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    list_library_inner(pool).await
}

pub async fn list_library_inner(pool: &sqlx::SqlitePool) -> Result<LibrarySnapshot, String> {
    let folders: Vec<FolderRecord> = sqlx::query_as(
        "SELECT id, parent_id, name, kind, path, expanded, position, created_at \
         FROM folders ORDER BY parent_id IS NULL DESC, position ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(LibrarySnapshot {
        folders,
        root_doc_ids: root_doc_ids(pool).await?,
        folder_docs: folder_docs_map(pool).await?,
        folder_child_counts: folder_child_counts(pool).await?,
    })
}

/// Solo las carpetas raíz, pero con el mapa `carpeta → documentos` **completo**:
/// así el índice `docId → carpeta` del frontend sigue siendo válido y los hijos
/// de cada carpeta se piden al expandirla (`list_folder_children`).
#[tauri::command]
pub async fn list_library_root() -> Result<LibrarySnapshot, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    list_library_root_inner(pool).await
}

pub async fn list_library_root_inner(pool: &sqlx::SqlitePool) -> Result<LibrarySnapshot, String> {
    let folders: Vec<FolderRecord> = sqlx::query_as(
        "SELECT id, parent_id, name, kind, path, expanded, position, created_at \
         FROM folders WHERE parent_id IS NULL ORDER BY position ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(LibrarySnapshot {
        folders,
        root_doc_ids: root_doc_ids(pool).await?,
        folder_docs: folder_docs_map(pool).await?,
        folder_child_counts: folder_child_counts(pool).await?,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderChildren {
    pub folders: Vec<FolderRecord>,
}

/// Hijos directos de una carpeta. Los documentos de la carpeta ya vienen en
/// `folderDocs`, así que aquí solo hacen falta las subcarpetas.
#[tauri::command]
pub async fn list_folder_children(folder_id: String) -> Result<FolderChildren, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    list_folder_children_inner(pool, &folder_id).await
}

pub async fn list_folder_children_inner(
    pool: &sqlx::SqlitePool,
    folder_id: &str,
) -> Result<FolderChildren, String> {
    let folders: Vec<FolderRecord> = sqlx::query_as(
        "SELECT id, parent_id, name, kind, path, expanded, position, created_at \
         FROM folders WHERE parent_id = ? ORDER BY position ASC",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(FolderChildren { folders })
}

/// Número de subcarpetas de cada carpeta: con el árbol bajo demanda el frontend
/// no las conoce hasta expandir, pero el contador de cada fila debe ser exacto.
async fn folder_child_counts(
    pool: &sqlx::SqlitePool,
) -> Result<std::collections::HashMap<String, i64>, String> {
    let rows = sqlx::query(
        "SELECT parent_id AS folder_id, COUNT(*) AS children FROM folders \
         WHERE parent_id IS NOT NULL GROUP BY parent_id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in rows {
        let folder_id: String = row.try_get("folder_id").map_err(|e| e.to_string())?;
        let children: i64 = row.try_get("children").map_err(|e| e.to_string())?;
        counts.insert(folder_id, children);
    }
    Ok(counts)
}

async fn root_doc_ids(pool: &sqlx::SqlitePool) -> Result<Vec<String>, String> {
    sqlx::query_scalar(
        "SELECT id FROM documents \
         WHERE in_library = 1 AND id NOT IN (SELECT doc_id FROM folder_docs) \
         ORDER BY position ASC, created_at ASC, rowid ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())
}

async fn next_root_position(conn: &mut sqlx::SqliteConnection) -> Result<i64, String> {
    sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM documents \
     WHERE in_library = 1 AND id NOT IN (SELECT doc_id FROM folder_docs)",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| e.to_string())
}

async fn unlink_document_from_folder(
    conn: &mut sqlx::SqliteConnection,
    doc_id: &str,
) -> Result<Option<(String, i64)>, String> {
    let current: Option<(String, i64)> =
        sqlx::query_as("SELECT folder_id, position FROM folder_docs WHERE doc_id = ?")
            .bind(doc_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    unlink_document_from_folder_at(conn, doc_id, current).await
}

async fn unlink_document_from_folder_at(
    conn: &mut sqlx::SqliteConnection,
    doc_id: &str,
    current: Option<(String, i64)>,
) -> Result<Option<(String, i64)>, String> {
    let Some((folder_id, position)) = current else {
        return Ok(None);
    };

    sqlx::query("DELETE FROM folder_docs WHERE doc_id = ?")
        .bind(doc_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE folder_docs SET position = position - 1 \
     WHERE folder_id = ? AND position > ?",
    )
    .bind(&folder_id)
    .bind(position)
    .execute(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    Ok(Some((folder_id, position)))
}

async fn folder_docs_map(
    pool: &sqlx::SqlitePool,
) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let folder_docs_rows = sqlx::query(
        "SELECT folder_docs.folder_id, folder_docs.doc_id \
         FROM folder_docs JOIN documents ON documents.id = folder_docs.doc_id \
         WHERE documents.in_library = 1 ORDER BY folder_docs.folder_id, folder_docs.position ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut folder_docs: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in folder_docs_rows {
        let fid: String = row.try_get("folder_id").map_err(|e| e.to_string())?;
        let did: String = row.try_get("doc_id").map_err(|e| e.to_string())?;
        folder_docs.entry(fid).or_default().push(did);
    }
    Ok(folder_docs)
}

async fn merge_folder_links(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    from_doc_id: &str,
    to_doc_id: &str,
) -> Result<(), String> {
    let links: Vec<(String, i64)> =
        sqlx::query_as("SELECT folder_id, position FROM folder_docs WHERE doc_id = ?")
            .bind(from_doc_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;

    for (folder_id, _) in &links {
        let target_exists: Option<String> =
            sqlx::query_scalar("SELECT folder_id FROM folder_docs WHERE doc_id = ?")
                .bind(to_doc_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
        if target_exists.is_some() {
            sqlx::query("DELETE FROM folder_docs WHERE folder_id = ? AND doc_id = ?")
                .bind(folder_id)
                .bind(from_doc_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            sqlx::query("UPDATE folder_docs SET doc_id = ? WHERE folder_id = ? AND doc_id = ?")
                .bind(to_doc_id)
                .bind(folder_id)
                .bind(from_doc_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        reindex_folder_docs(tx, folder_id).await?;
    }
    Ok(())
}

async fn reindex_folder_docs(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    folder_id: &str,
) -> Result<(), String> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT doc_id FROM folder_docs WHERE folder_id = ? ORDER BY position, doc_id",
    )
    .bind(folder_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    for (position, doc_id) in ids.iter().enumerate() {
        sqlx::query("UPDATE folder_docs SET position = ? WHERE folder_id = ? AND doc_id = ?")
            .bind(position as i64)
            .bind(folder_id)
            .bind(doc_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn add_file(
    path: String,
    folder_id: Option<String>,
) -> Result<crate::db::models::DocumentRecord, String> {
    validate_imported_pdf_path(&path)?;
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    // Contar páginas parsea el PDF entero: fuera de la transacción para no
    // retener el bloqueo de escritura de SQLite.
    let pages = resolve_pages(pool, &path).await?;
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let record = add_file_inner(&mut transaction, &path, folder_id.as_deref(), pages).await?;
    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(record)
}

fn validate_imported_pdf_path(path: &str) -> Result<(), String> {
    let file_path = std::path::Path::new(path);
    let extension_is_pdf = file_path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
    if !extension_is_pdf {
        return Err("Solo se pueden añadir archivos PDF".into());
    }
    let metadata = std::fs::metadata(file_path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("La ruta no es un archivo".into());
    }
    let mut file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut header = [0u8; 5];
    use std::io::Read;
    let bytes_read = file.read(&mut header).map_err(|e| e.to_string())?;
    if bytes_read < header.len() || &header != b"%PDF-" {
        return Err("El archivo no parece ser un PDF válido".into());
    }
    Ok(())
}

/// Añade (o refresca) un documento y lo vincula a `folder_id`. Trabaja sobre una
/// conexión, así que el sync puede meter cientos de archivos en la misma
/// transacción. `pages` se resuelve fuera (`resolve_pages`).
async fn add_file_inner(
    conn: &mut sqlx::SqliteConnection,
    path: &str,
    folder_id: Option<&str>,
    pages: i64,
) -> Result<crate::db::models::DocumentRecord, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let size = meta.len() as i64;

    let title = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Documento.pdf")
        .to_string();

    let doc_id = new_id("doc");
    let now = now_ms();

    let next_position = next_root_position(conn).await?;
    let existing_folder: Option<String> =
        sqlx::query_scalar("SELECT folder_id FROM folder_docs WHERE doc_id = (SELECT id FROM documents WHERE path = ?)")
            .bind(path)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;

    let insert_result = sqlx::query(
        "INSERT OR IGNORE INTO documents \
         (id, path, title, kind, pages, size_bytes, hash, category_id, last_page, last_zoom, last_opened, in_recent, created_at, view_json, view_state, position) \
         VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, 1, 100, NULL, 0, ?, '{}', '{}', ?)",
    )
    .bind(&doc_id)
    .bind(path)
    .bind(&title)
    .bind(kind_from_extension(path))
    .bind(pages)
    .bind(size)
    .bind(now)
    .bind(next_position)
    .execute(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE documents SET title = ?, kind = ?, pages = ?, size_bytes = ?, \
         last_page = CASE WHEN ? > 0 THEN MIN(MAX(last_page, 1), ?) ELSE last_page END, \
         in_library = 1 WHERE path = ?",
    )
    .bind(&title)
    .bind(kind_from_extension(path))
    .bind(pages)
    .bind(size)
    .bind(pages)
    .bind(pages)
    .bind(path)
    .execute(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    let stored_id: String = sqlx::query_scalar("SELECT id FROM documents WHERE path = ?")
        .bind(path)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;

    if folder_id.is_none() {
        unlink_document_from_folder(conn, &stored_id).await?;
        if insert_result.rows_affected() > 0 || existing_folder.is_some() {
            sqlx::query("UPDATE documents SET position = ? WHERE path = ? AND in_library = 1")
                .bind(next_position)
                .bind(path)
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(fid) = folder_id {
        let current_folder: Option<String> =
            sqlx::query_scalar("SELECT folder_id FROM folder_docs WHERE doc_id = ?")
                .bind(&stored_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;

        if current_folder.as_deref() != Some(fid) {
            unlink_document_from_folder(conn, &stored_id).await?;
            let pos: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM folder_docs WHERE folder_id = ?",
            )
            .bind(fid)
            .fetch_one(&mut *conn)
            .await
            .unwrap_or(0);
            sqlx::query(
                "INSERT OR IGNORE INTO folder_docs (folder_id, doc_id, position) \
                 SELECT ?, id, ? FROM documents WHERE path = ?",
            )
            .bind(fid)
            .bind(pos)
            .bind(path)
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                format!("No se pudo vincular el documento {stored_id} a la carpeta {fid}: {e}")
            })?;
        }
    }

    let record: crate::db::models::DocumentRecord = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {} FROM documents WHERE path = ?",
        crate::db::models::DOCUMENT_COLUMNS
    )))
    .bind(path)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    clear_ignored_paths_for_file(conn, std::path::Path::new(path)).await?;
    Ok(record)
}

/// Borra la entrada exacta del archivo en `ignored_synced_paths` y, si acaso,
/// las entradas ancestro que lo contengan. NO borra hermanos: añadir
/// `/libro/cap1.pdf` no debe des-ignorar `/libro/cap2.pdf`.
///
/// Se llama una vez por archivo dentro del sync, así que la consulta prefiltra
/// en SQL en lugar de leer la tabla entera: el `LIKE` es un **superconjunto** de
/// `starts_with` (no distingue mayúsculas y los comodines del patrón pueden
/// traer de más) y el descarte exacto se sigue haciendo en Rust.
async fn clear_ignored_paths_for_file(
    conn: &mut sqlx::SqliteConnection,
    file_path: &std::path::Path,
) -> Result<(), String> {
    let Some(file_path_str) = file_path.to_str() else {
        return Ok(());
    };
    let ignored: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM ignored_synced_paths WHERE ?1 LIKE \
             replace(replace(replace(path, '\\', '\\\\'), '%', '\\%'), '_', '\\_') || '%' \
             ESCAPE '\\'",
    )
    .bind(file_path_str)
    .fetch_all(&mut *conn)
    .await
    .map_err(|error| error.to_string())?;

    let mut to_clear: Vec<String> = Vec::new();
    let file_path_buf = file_path.to_path_buf();
    for path in ignored {
        let ignored_path = std::path::Path::new(&path);
        if ignored_path == file_path {
            to_clear.push(path);
            continue;
        }
        // Solo los ancestros de file_path (directorios que lo contienen).
        if ignored_path.is_dir() && file_path_buf.starts_with(ignored_path) {
            to_clear.push(path);
        }
    }

    for path in to_clear {
        sqlx::query("DELETE FROM ignored_synced_paths WHERE path = ?")
            .bind(path)
            .execute(&mut *conn)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn add_folder(
    name: String,
    kind: String,
    parent_id: Option<String>,
    path: Option<String>,
) -> Result<FolderRecord, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let record = add_folder_inner(&mut transaction, name, kind, parent_id, path, true).await?;
    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(record)
}

/// `expanded`: el sync crea cientos de subcarpetas de golpe, así que nacen
/// colapsadas para que el primer pintado del árbol no las abra todas. Las
/// carpetas que crea el usuario siguen naciendo expandidas.
async fn add_folder_inner(
    conn: &mut sqlx::SqliteConnection,
    name: String,
    kind: String,
    parent_id: Option<String>,
    mut path: Option<String>,
    expanded: bool,
) -> Result<FolderRecord, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("El nombre de la carpeta no puede estar vacío".into());
    }
    if kind != "temp" && kind != "synced" {
        return Err("Tipo de carpeta no válido".into());
    }
    if kind == "synced" {
        let folder_path = path
            .as_deref()
            .ok_or("La carpeta sincronizada necesita una ruta")?;
        let canonical_path = std::fs::canonicalize(folder_path)
            .map_err(|_| "La ruta seleccionada no es una carpeta válida".to_string())?;
        if !canonical_path.is_dir() {
            return Err("La ruta seleccionada no es una carpeta válida".into());
        }
        path = Some(canonical_path.to_string_lossy().into_owned());
        let linked_paths: Vec<String> = sqlx::query_scalar(
            "SELECT path FROM folders WHERE kind = 'synced' AND path IS NOT NULL",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
        if linked_paths.into_iter().any(|linked| {
            std::fs::canonicalize(&linked)
                .map(|canonical| canonical == canonical_path)
                .unwrap_or_else(|_| std::path::Path::new(&linked) == canonical_path)
        }) {
            return Err("Esa carpeta ya está vinculada a la biblioteca".into());
        }
    }
    let id = new_id("folder");
    let now = now_ms();

    let pos: i64 = match &parent_id {
        Some(p) => sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM folders WHERE parent_id = ?",
        )
        .bind(p)
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(0),
        None => sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM folders WHERE parent_id IS NULL",
        )
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(0),
    };

    sqlx::query(
        "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id).bind(&parent_id).bind(&name).bind(&kind).bind(&path).bind(expanded).bind(pos).bind(now)
    .execute(&mut *conn).await.map_err(|e| e.to_string())?;

    if kind == "synced"
        && parent_id.is_none()
        && let Some(root_path) = path.as_deref()
    {
        clear_ignored_paths_under(conn, std::path::Path::new(root_path)).await?;
    }

    Ok(FolderRecord {
        id,
        parent_id,
        name,
        kind,
        path,
        expanded,
        position: pos,
        created_at: now,
    })
}

async fn clear_ignored_paths_under(
    conn: &mut sqlx::SqliteConnection,
    root_path: &std::path::Path,
) -> Result<(), String> {
    // Prefiltro en SQL: como en el renombrado, es un superconjunto de
    // `starts_with` (`LIKE` no distingue mayúsculas y `%`/`_` son comodines), así
    // que puede traer de más pero nunca de menos. `starts_with` decide.
    let prefix = root_path
        .to_string_lossy()
        .trim_end_matches(['/', '\\'])
        .to_string();
    let ignored: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM ignored_synced_paths WHERE path LIKE ? || '%' ESCAPE '\\'",
    )
    .bind(like_prefix(&prefix))
    .fetch_all(&mut *conn)
    .await
    .map_err(|error| error.to_string())?;
    for path in ignored {
        if std::path::Path::new(&path).starts_with(root_path) {
            sqlx::query("DELETE FROM ignored_synced_paths WHERE path = ?")
                .bind(path)
                .execute(&mut *conn)
                .await
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn restore_document_to_library(doc_id: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    restore_document_to_library_inner(pool, doc_id).await
}

pub(crate) async fn restore_document_to_library_inner(
    pool: &sqlx::SqlitePool,
    doc_id: String,
) -> Result<(), String> {
    if doc_id.trim().is_empty() {
        return Err("El id del documento no puede estar vacío".into());
    }
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let next_position = next_root_position(&mut transaction).await?;
    let result = sqlx::query("UPDATE documents SET in_library = 1, position = ? WHERE id = ?")
        .bind(next_position)
        .bind(doc_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("No se encontró el documento".into());
    }
    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn rename_folder(id: String, name: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE folders SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn set_folder_expanded(id: String, expanded: bool) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE folders SET expanded = ? WHERE id = ?")
        .bind(if expanded { 1 } else { 0 })
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Borrado de carpeta con snapshot en papelera
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn delete_folder(id: String, delete_contents: bool) -> Result<Vec<String>, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    delete_folder_inner(pool, id, delete_contents).await
}

pub(crate) async fn delete_folder_inner(
    pool: &sqlx::SqlitePool,
    id: String,
    delete_contents: bool,
) -> Result<Vec<String>, String> {
    if id.trim().is_empty() {
        return Err("El id de la carpeta no puede estar vacío".into());
    }
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let mut deleted_document_ids = Vec::new();

    // Recogemos siempre el subárbol, independientemente de `delete_contents`.
    // Los paths de carpetas sincronizadas se marcan como ignorados para que
    // el sync no las recree en el siguiente ciclo.
    let folders: Vec<(String, String)> = sqlx::query_as(
        "WITH RECURSIVE folder_tree(id, path, kind) AS ( \
           SELECT id, path, kind FROM folders WHERE id = ? \
           UNION ALL \
           SELECT child.id, child.path, child.kind FROM folders child \
           JOIN folder_tree parent ON child.parent_id = parent.id \
         ) \
         SELECT path, kind FROM folder_tree WHERE path IS NOT NULL",
    )
    .bind(&id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?;

    let synced_paths: Vec<String> = folders
        .iter()
        .filter(|(_, kind)| kind == "synced")
        .map(|(path, _)| path.clone())
        .collect();
    for path in &synced_paths {
        sqlx::query("INSERT OR IGNORE INTO ignored_synced_paths (path) VALUES (?)")
            .bind(path)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Snapshot para la papelera. Se toma antes de los DELETE, dentro de la
    // misma transacción: si algo falla, se revierte también el snapshot.
    let folder_row: Option<FolderRecord> = sqlx::query_as(
        "SELECT id, parent_id, name, kind, path, expanded, position, created_at \
         FROM folders WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(folder_row) = folder_row {
        // Subárbol completo de carpetas, ordenado por profundidad para poder
        // restaurarlo insertando padres antes que hijos. Sin esto, restaurar una
        // carpeta con subcarpetas dejaba fuera las hijas y sus documentos
        // acababan colgando de la raíz (o perdidos).
        let folders_snapshot: Vec<serde_json::Value> = {
            let rows: Vec<FolderSnapshotRow> = sqlx::query_as(
                "WITH RECURSIVE folder_tree(id, depth) AS ( \
                   SELECT id, 0 FROM folders WHERE id = ? \
                   UNION ALL \
                   SELECT child.id, parent.depth + 1 FROM folders child \
                   JOIN folder_tree parent ON child.parent_id = parent.id \
                 ) \
                 SELECT folders.id, folders.parent_id, folders.name, folders.kind, folders.path, \
                        folders.expanded, folders.position, folders.created_at \
                 FROM folders JOIN folder_tree ON folder_tree.id = folders.id \
                 ORDER BY folder_tree.depth ASC",
            )
            .bind(&id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;

            rows.into_iter()
                .map(
                    |(id, parent_id, name, kind, path, expanded, position, created_at)| {
                        serde_json::json!({
                            "id": id, "parentId": parent_id, "name": name, "kind": kind,
                            "path": path, "expanded": expanded, "position": position,
                            "createdAt": created_at,
                        })
                    },
                )
                .collect()
        };

        let folder_docs_snapshot: Vec<(String, String, i64)> = sqlx::query_as(
            "WITH RECURSIVE folder_tree(id) AS ( \
               SELECT id FROM folders WHERE id = ? \
               UNION ALL \
               SELECT child.id FROM folders child \
               JOIN folder_tree parent ON child.parent_id = parent.id \
             ) \
             SELECT folder_id, doc_id, position FROM folder_docs \
             WHERE folder_id IN (SELECT id FROM folder_tree)",
        )
        .bind(&id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;

        let documents_snapshot: Vec<serde_json::Value> = {
            let rows: Vec<DocumentSnapshotRow> = sqlx::query_as(
                "WITH RECURSIVE folder_tree(id) AS ( \
                   SELECT id FROM folders WHERE id = ? \
                   UNION ALL \
                   SELECT child.id FROM folders child \
                   JOIN folder_tree parent ON child.parent_id = parent.id \
                 ) \
                 SELECT DISTINCT documents.id, documents.path, documents.title, documents.kind, documents.pages, \
                        documents.size_bytes, documents.hash, documents.category_id, \
                        COALESCE((SELECT json_group_array(category_id) FROM document_categories \
                                  WHERE doc_id = documents.id), '[]') AS category_ids, documents.last_page, \
                        documents.last_zoom, documents.last_opened, documents.in_recent, \
                        documents.in_library, documents.created_at, documents.view_json, documents.view_state, documents.position \
                 FROM documents JOIN folder_docs ON folder_docs.doc_id = documents.id \
                 WHERE folder_docs.folder_id IN (SELECT id FROM folder_tree)",
            )
            .bind(&id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;

            rows.into_iter().map(|row| {
                        serde_json::json!({
                            "id": row.id, "path": row.path, "title": row.title, "kind": row.kind, "pages": row.pages,
                            "sizeBytes": row.size_bytes, "hash": row.hash, "categoryId": row.category_id,
                            "categoryIds": serde_json::from_str::<Vec<String>>(
                                row.category_ids.as_deref().unwrap_or("[]"),
                            )
                                .unwrap_or_default(),
                            "lastPage": row.last_page, "lastZoom": row.last_zoom, "lastOpened": row.last_opened,
                            "inRecent": row.in_recent, "inLibrary": row.in_library, "createdAt": row.created_at,
                            "view": row.view_json, "viewState": row.view_state, "position": row.position,
                        })
                    }).collect()
        };

        crate::commands::trash::push(
            &mut transaction,
            "folder",
            &format!("Eliminar carpeta «{}»", folder_row.name),
            &serde_json::json!({
                "folder": folder_row,
                "folders": folders_snapshot,
                "folderDocs": folder_docs_snapshot.into_iter().map(|(f, d, p)| serde_json::json!({
                    "folderId": f, "docId": d, "position": p
                })).collect::<Vec<_>>(),
                "documents": documents_snapshot,
            }),
        )
        .await?;
    }

    let orphaned_document_ids = if delete_contents {
        Vec::new()
    } else {
        documents_in_subtree(&mut *transaction, &id)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(doc_id, _)| doc_id)
            .collect()
    };
    let mut orphan_position = if orphaned_document_ids.is_empty() {
        None
    } else {
        Some(next_root_position(&mut transaction).await?)
    };

    if delete_contents {
        let documents: Vec<(String, String)> = documents_in_subtree(&mut *transaction, &id)
            .await
            .map_err(|e| e.to_string())?;

        for (doc_id, path) in documents {
            let under_synced = synced_paths
                .iter()
                .any(|root| std::path::Path::new(&path).starts_with(root));
            if under_synced {
                sqlx::query("INSERT OR IGNORE INTO ignored_synced_paths (path) VALUES (?)")
                    .bind(&path)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            sqlx::query("DELETE FROM documents WHERE id = ?")
                .bind(&doc_id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            deleted_document_ids.push(doc_id);
        }
    }

    sqlx::query("DELETE FROM folders WHERE id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;

    if let Some(mut position) = orphan_position.take() {
        for doc_id in orphaned_document_ids {
            sqlx::query("UPDATE documents SET position = ? WHERE id = ? AND in_library = 1")
                .bind(position)
                .bind(doc_id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            position += 1;
        }
    }

    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(deleted_document_ids)
}

/// Documentos que cuelgan exclusivamente del subárbol de `root_id`.
/// Excluye documentos que también pertenecen a carpetas fuera del subárbol.
async fn documents_in_subtree<'e, E>(
    executor: E,
    root_id: &str,
) -> Result<Vec<(String, String)>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_as(
        "WITH RECURSIVE folder_tree(id) AS ( \
           SELECT id FROM folders WHERE id = ? \
           UNION ALL \
           SELECT child.id FROM folders child \
           JOIN folder_tree parent ON child.parent_id = parent.id \
         ) \
         SELECT DISTINCT documents.id, documents.path FROM documents \
         JOIN folder_docs selected ON selected.doc_id = documents.id \
         WHERE selected.folder_id IN (SELECT id FROM folder_tree) \
         AND NOT EXISTS ( \
           SELECT 1 FROM folder_docs other \
           WHERE other.doc_id = documents.id \
           AND other.folder_id NOT IN (SELECT id FROM folder_tree) \
         )",
    )
    .bind(root_id)
    .fetch_all(executor)
    .await
}

// ─────────────────────────────────────────────────────────────────────────────
// Reordenado
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn move_doc(
    doc_id: String,
    folder_id: Option<String>,
    position: i64,
) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    move_doc_inner(pool, doc_id, folder_id, position).await
}

pub(crate) async fn move_doc_inner(
    pool: &sqlx::SqlitePool,
    doc_id: String,
    folder_id: Option<String>,
    position: i64,
) -> Result<(), String> {
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;

    let current = unlink_document_from_folder(&mut transaction, &doc_id).await?;

    match folder_id {
        Some(folder_id) => {
            let target_position = match &current {
                Some((old_folder_id, old_position))
                    if old_folder_id == &folder_id && *old_position < position =>
                {
                    position - 1
                }
                _ => position,
            }
            .max(0);
            let item_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM folder_docs WHERE folder_id = ?")
                    .bind(&folder_id)
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(|e| e.to_string())?;
            let target_position = target_position.min(item_count);

            sqlx::query(
                "UPDATE folder_docs SET position = position + 1 \
                 WHERE folder_id = ? AND position >= ?",
            )
            .bind(&folder_id)
            .bind(target_position)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query("INSERT INTO folder_docs (folder_id, doc_id, position) VALUES (?, ?, ?)")
                .bind(&folder_id)
                .bind(&doc_id)
                .bind(target_position)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            reindex_document_children(&mut transaction, &folder_id).await?;
        }
        None => {
            // Documento a la raíz: actualizar `position` en `documents`.
            // Antes se ignoraba `position` y era un no-op silencioso.
            let current_position: i64 =
                sqlx::query_scalar("SELECT position FROM documents WHERE id = ?")
                    .bind(&doc_id)
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(|e| e.to_string())?;

            let max_index: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(position), -1) FROM documents \
                 WHERE in_library = 1 AND id != ? \
                 AND id NOT IN (SELECT doc_id FROM folder_docs)",
            )
            .bind(&doc_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;

            // El ajuste `-1` solo vale cuando el documento **ya estaba en la
            // raíz** y se mueve hacia abajo: al quitarlo de la lista, los
            // siguientes se desplazan un puesto y el índice recibido (calculado
            // sobre la lista con el documento todavía dentro) queda uno de más.
            // Viniendo de una carpeta no estaba en esa lista, así que el índice es
            // el destino tal cual.
            let target = if current.is_none() && current_position < position {
                position - 1
            } else {
                position
            }
            .clamp(0, max_index + 1);

            if current.is_some() {
                // Venimos de una carpeta: abrir hueco en la raíz.
                sqlx::query(
                    "UPDATE documents SET position = position + 1 \
                     WHERE in_library = 1 AND position >= ? \
                     AND id NOT IN (SELECT doc_id FROM folder_docs)",
                )
                .bind(target)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
                sqlx::query("UPDATE documents SET position = ? WHERE id = ?")
                    .bind(target)
                    .bind(&doc_id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|e| e.to_string())?;
            } else if target != current_position {
                // Ya estaba en la raíz: reordenar.
                if target > current_position {
                    sqlx::query(
                        "UPDATE documents SET position = position - 1 \
                         WHERE in_library = 1 AND position > ? AND position <= ? \
                         AND id != ? AND id NOT IN (SELECT doc_id FROM folder_docs)",
                    )
                    .bind(current_position)
                    .bind(target)
                    .bind(&doc_id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|e| e.to_string())?;
                } else {
                    sqlx::query(
                        "UPDATE documents SET position = position + 1 \
                         WHERE in_library = 1 AND position >= ? AND position < ? \
                         AND id != ? AND id NOT IN (SELECT doc_id FROM folder_docs)",
                    )
                    .bind(target)
                    .bind(current_position)
                    .bind(&doc_id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|e| e.to_string())?;
                }
                sqlx::query("UPDATE documents SET position = ? WHERE id = ?")
                    .bind(target)
                    .bind(&doc_id)
                    .execute(&mut *transaction)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    // La carpeta de origen ya quedó compactada arriba (justo después de sacar el
    // documento): repetirlo aquí era un `SELECT` + N `UPDATE` de más por cada
    // movimiento, porque nada de lo que hay en medio toca sus filas.
    transaction.commit().await.map_err(|e| e.to_string())
}

async fn reindex_document_children(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    folder_id: &str,
) -> Result<(), String> {
    let doc_ids: Vec<String> = sqlx::query_scalar(
        "SELECT doc_id FROM folder_docs WHERE folder_id = ? \
         ORDER BY position ASC, doc_id ASC",
    )
    .bind(folder_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|e| e.to_string())?;

    for (position, doc_id) in doc_ids.iter().enumerate() {
        sqlx::query("UPDATE folder_docs SET position = ? WHERE folder_id = ? AND doc_id = ?")
            .bind(position as i64)
            .bind(folder_id)
            .bind(doc_id)
            .execute(&mut **transaction)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn reindex_folder_children(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    parent_id: &Option<String>,
) -> Result<(), String> {
    let folder_ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM folders \
         WHERE ((? IS NULL AND parent_id IS NULL) OR parent_id = ?) \
         ORDER BY position ASC, created_at ASC, id ASC",
    )
    .bind(parent_id)
    .bind(parent_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|e| e.to_string())?;

    for (position, folder_id) in folder_ids.iter().enumerate() {
        sqlx::query("UPDATE folders SET position = ? WHERE id = ?")
            .bind(position as i64)
            .bind(folder_id)
            .execute(&mut **transaction)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn move_folder(
    folder_id: String,
    parent_id: Option<String>,
    position: i64,
    allow_reparent_synced: Option<bool>,
) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    move_folder_inner(pool, folder_id, parent_id, position, allow_reparent_synced).await
}

pub async fn move_folder_inner(
    pool: &sqlx::SqlitePool,
    folder_id: String,
    parent_id: Option<String>,
    position: i64,
    allow_reparent_synced: Option<bool>,
) -> Result<(), String> {
    // Evitar ciclo: parent_id no debe ser descendiente del folder
    if let Some(p) = &parent_id {
        let mut cur = Some(p.clone());
        let mut visited = std::collections::HashSet::new();
        while let Some(cid) = cur {
            if !visited.insert(cid.clone()) {
                return Err("No se puede mover una carpeta: la jerarquía contiene un ciclo".into());
            }
            if cid == folder_id {
                return Err("No se puede mover una carpeta dentro de sí misma".into());
            }
            cur = sqlx::query_scalar::<_, Option<String>>(
                "SELECT parent_id FROM folders WHERE id = ?",
            )
            .bind(&cid)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .flatten();
        }
    }

    // Una carpeta sincronizada raíz se sincroniza solo si sigue siendo raíz.
    // Moverla bajo otra carpeta la desactiva, así que exige confirmación.
    if parent_id.is_some() && !allow_reparent_synced.unwrap_or(false) {
        let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM folders WHERE id = ?")
            .bind(&folder_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
        if kind.as_deref() == Some("synced") {
            let is_root: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM folders WHERE id = ? AND parent_id IS NULL")
                    .bind(&folder_id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| e.to_string())?;
            if is_root.is_some() {
                return Err("Mover una carpeta sincronizada raíz la desactiva. \
                     Activa «Permitir mover raíces sincronizadas» en Configuración \
                     para continuar."
                    .into());
            }
        }
    }

    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let current: Option<(Option<String>, i64)> =
        sqlx::query_as("SELECT parent_id, position FROM folders WHERE id = ?")
            .bind(&folder_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    let Some((old_parent, old_position)) = current else {
        return Err("No se encontró la carpeta".into());
    };
    let target_position = if old_parent == parent_id && old_position < position {
        position - 1
    } else {
        position
    }
    .max(0);

    sqlx::query("UPDATE folders SET parent_id = ?, position = 999999 WHERE id = ?")
        .bind(&parent_id)
        .bind(&folder_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    if old_parent != parent_id {
        reindex_folder_children(&mut transaction, &old_parent).await?;
    }

    let mut siblings: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM folders \
         WHERE ((? IS NULL AND parent_id IS NULL) OR parent_id = ?) AND id != ? \
         ORDER BY position ASC, created_at ASC, id ASC",
    )
    .bind(&parent_id)
    .bind(&parent_id)
    .bind(&folder_id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?;
    siblings.insert(
        (target_position as usize).min(siblings.len()),
        folder_id.clone(),
    );

    for (position, sibling_id) in siblings.iter().enumerate() {
        sqlx::query("UPDATE folders SET position = ? WHERE id = ?")
            .bind(position as i64)
            .bind(sibling_id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }
    transaction.commit().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_doc(doc_id: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM documents WHERE id = ?")
        .bind(&doc_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn remove_from_library(doc_id: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    remove_from_library_inner(pool, doc_id).await
}

pub(crate) async fn remove_from_library_inner(
    pool: &sqlx::SqlitePool,
    doc_id: String,
) -> Result<(), String> {
    if doc_id.trim().is_empty() {
        return Err("El id del documento no puede estar vacío".into());
    }
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;

    // Snapshot antes del cambio.
    let doc: Option<crate::db::models::DocumentRecord> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {} FROM documents WHERE id = ?",
            crate::db::models::DOCUMENT_COLUMNS
        )))
        .bind(&doc_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    let Some(doc) = doc else {
        return Err("No se encontró el documento".into());
    };

    let folder_doc: Option<(String, String, i64)> =
        sqlx::query_as("SELECT folder_id, doc_id, position FROM folder_docs WHERE doc_id = ?")
            .bind(&doc_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;

    crate::commands::trash::push(
        &mut transaction,
        "document",
        &format!("Quitar «{}» de Archivos", doc.title),
        &serde_json::json!({
            "document": doc,
            "folderDoc": folder_doc.as_ref().map(|(f, d, p)| serde_json::json!({
                "folderId": f, "docId": d, "position": p
            })),
        }),
    )
    .await?;

    let in_synced_folder: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM folder_docs \
         JOIN folders ON folders.id = folder_docs.folder_id \
         WHERE folder_docs.doc_id = ? AND folders.kind = 'synced' LIMIT 1",
    )
    .bind(&doc_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?;

    let result = sqlx::query("UPDATE documents SET in_library = 0 WHERE id = ?")
        .bind(&doc_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("No se encontró el documento".into());
    }

    unlink_document_from_folder_at(
        &mut transaction,
        &doc_id,
        folder_doc
            .as_ref()
            .map(|(folder_id, _, position)| (folder_id.clone(), *position)),
    )
    .await?;
    if in_synced_folder.is_some() {
        sqlx::query("INSERT OR IGNORE INTO ignored_synced_paths (path) VALUES (?)")
            .bind(&doc.path)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }
    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn assign_category(doc_id: String, category_id: Option<String>) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let result = sqlx::query("DELETE FROM document_categories WHERE doc_id = ?")
        .bind(&doc_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0
        && sqlx::query_scalar::<_, i64>("SELECT 1 FROM documents WHERE id = ?")
            .bind(&doc_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?
            .is_none()
    {
        return Err("No se encontró el documento".into());
    }
    if let Some(category_id) = category_id {
        sqlx::query(
            "INSERT OR IGNORE INTO document_categories (doc_id, category_id) VALUES (?, ?)",
        )
        .bind(&doc_id)
        .bind(category_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    }
    sqlx::query(
        "UPDATE documents SET category_id = ( \
           SELECT category_id FROM document_categories \
           WHERE doc_id = ? ORDER BY rowid LIMIT 1 \
         ) WHERE id = ?",
    )
    .bind(&doc_id)
    .bind(&doc_id)
    .execute(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?;
    transaction.commit().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_recent_docs() -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE documents SET in_recent = 0, last_opened = NULL")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn remove_recent_doc(doc_id: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let result = sqlx::query("UPDATE documents SET in_recent = 0 WHERE id = ?")
        .bind(doc_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("No se encontró el documento".into());
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Renombrado externo con detección de colisiones
// ─────────────────────────────────────────────────────────────────────────────

pub async fn update_paths_after_rename(
    old_path: &std::path::Path,
    new_path: &std::path::Path,
) -> Result<bool, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let normalized_new = std::fs::canonicalize(new_path).unwrap_or_else(|_| new_path.to_path_buf());
    update_paths_after_rename_inner(pool, old_path, &normalized_new).await
}

pub async fn update_paths_after_rename_inner(
    pool: &sqlx::SqlitePool,
    old_path: &std::path::Path,
    new_path: &std::path::Path,
) -> Result<bool, String> {
    if old_path == new_path || !new_path.exists() {
        return Ok(false);
    }

    // El filtro SQL es un **superconjunto** de `strip_prefix`: `LIKE` no
    // distingue mayúsculas y trata `%` y `_` del patrón como comodines, así que
    // puede traer de más (`/libros` para `/lib`) pero nunca de menos. El
    // `rebase_path` de abajo decide. El separador final se recorta porque
    // `Path::strip_prefix("/lib/")` también acepta `/lib`.
    let prefix = old_path
        .to_string_lossy()
        .trim_end_matches(['/', '\\'])
        .to_string();

    let documents: Vec<(String, String)> =
        sqlx::query_as("SELECT id, path FROM documents WHERE path LIKE ? || '%' ESCAPE '\\'")
            .bind(like_prefix(&prefix))
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    let folders: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, path FROM folders WHERE path IS NOT NULL AND path LIKE ? || '%' ESCAPE '\\'",
    )
    .bind(like_prefix(&prefix))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let file_states: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM synced_file_state WHERE path LIKE ? || '%' ESCAPE '\\'",
    )
    .bind(like_prefix(&prefix))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let ignored_paths: Vec<String> = sqlx::query_scalar(
        "SELECT path FROM ignored_synced_paths WHERE path LIKE ? || '%' ESCAPE '\\'",
    )
    .bind(like_prefix(&prefix))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let rebased_documents: Vec<(String, String)> = documents
        .into_iter()
        .filter_map(|(id, path)| {
            rebase_path(std::path::Path::new(&path), old_path, new_path)
                .map(|new| (id, new.to_string_lossy().into_owned()))
        })
        .collect();
    let rebased_folders: Vec<(String, String)> = folders
        .into_iter()
        .filter_map(|(id, path)| {
            rebase_path(std::path::Path::new(&path), old_path, new_path)
                .map(|new| (id, new.to_string_lossy().into_owned()))
        })
        .collect();
    let rebased_states: Vec<(String, String)> = file_states
        .into_iter()
        .filter_map(|path| {
            rebase_path(std::path::Path::new(&path), old_path, new_path)
                .map(|new| (path, new.to_string_lossy().into_owned()))
        })
        .collect();
    let rebased_ignored_paths: Vec<(String, String)> = ignored_paths
        .into_iter()
        .filter_map(|path| {
            rebase_path(std::path::Path::new(&path), old_path, new_path)
                .map(|new| (path, new.to_string_lossy().into_owned()))
        })
        .collect();

    if rebased_documents.is_empty()
        && rebased_folders.is_empty()
        && rebased_states.is_empty()
        && rebased_ignored_paths.is_empty()
    {
        return Ok(false);
    }

    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;

    for (id, path) in &rebased_documents {
        let title = std::path::Path::new(path)
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("Documento.pdf");

        // Colisión: la nueva ruta ya está registrada bajo otro id. Fusionamos
        // anotaciones y categoría, y eliminamos el registro duplicado. Antes
        // se dejaba con `UPDATE OR IGNORE` y el siguiente sync podía duplicar.
        let collision: Option<String> =
            sqlx::query_scalar("SELECT id FROM documents WHERE path = ? AND id != ?")
                .bind(path)
                .bind(id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;

        if let Some(other_id) = collision {
            sqlx::query("UPDATE bookmarks SET doc_id = ? WHERE doc_id = ?")
                .bind(&other_id)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query("UPDATE comments SET doc_id = ? WHERE doc_id = ?")
                .bind(&other_id)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            merge_folder_links(&mut transaction, id, &other_id).await?;
            sqlx::query(
                "INSERT OR IGNORE INTO document_categories (doc_id, category_id) \
                 SELECT ?, category_id FROM document_categories WHERE doc_id = ?",
            )
            .bind(&other_id)
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query(
                "UPDATE documents SET category_id = ( \
                   SELECT category_id FROM document_categories \
                   WHERE doc_id = ? ORDER BY rowid LIMIT 1 \
                 ) WHERE id = ?",
            )
            .bind(&other_id)
            .bind(&other_id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query("UPDATE documents SET in_recent = MAX(in_recent, (SELECT in_recent FROM documents WHERE id = ?)) WHERE id = ?")
                .bind(id).bind(&other_id)
                .execute(&mut *transaction).await.map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM documents WHERE id = ?")
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            eprintln!(
                "[rename] Documento {id} fusionado con {other_id} por colisión de ruta {path}"
            );
            continue;
        }

        let result = sqlx::query("UPDATE documents SET path = ?, title = ? WHERE id = ?")
            .bind(path)
            .bind(title)
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
        if result.rows_affected() == 0 {
            eprintln!("[rename] Documento {id} no actualizado");
        }
    }

    for (id, path) in &rebased_folders {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("Carpeta");
        let collision: Option<String> =
            sqlx::query_scalar("SELECT id FROM folders WHERE path = ? AND id != ?")
                .bind(path)
                .bind(id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        if collision.is_some() {
            eprintln!("[rename] Carpeta {id} no actualizada (colisión con {path})");
            continue;
        }
        sqlx::query("UPDATE folders SET path = ?, name = ? WHERE id = ?")
            .bind(path)
            .bind(name)
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }

    for (old, new) in &rebased_states {
        let collision: Option<String> =
            sqlx::query_scalar("SELECT path FROM synced_file_state WHERE path = ?")
                .bind(new)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        if collision.is_some() {
            sqlx::query("DELETE FROM synced_file_state WHERE path = ?")
                .bind(old)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            continue;
        }
        sqlx::query("UPDATE synced_file_state SET path = ? WHERE path = ?")
            .bind(new)
            .bind(old)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }

    for (old, new) in &rebased_ignored_paths {
        let collision: Option<String> =
            sqlx::query_scalar("SELECT path FROM ignored_synced_paths WHERE path = ?")
                .bind(new)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        if collision.is_some() {
            sqlx::query("DELETE FROM ignored_synced_paths WHERE path = ?")
                .bind(old)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            continue;
        }
        sqlx::query("UPDATE ignored_synced_paths SET path = ? WHERE path = ?")
            .bind(new)
            .bind(old)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }

    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(true)
}

fn rebase_path(
    path: &std::path::Path,
    old_path: &std::path::Path,
    new_path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let suffix = path.strip_prefix(old_path).ok()?;
    // `join("")` añadiría un separador final; la ruta renombrada es la ruta.
    if suffix.as_os_str().is_empty() {
        return Some(new_path.to_path_buf());
    }
    Some(new_path.join(suffix))
}

fn is_ignored_path(path: &std::path::Path, ignored_paths: &[std::path::PathBuf]) -> bool {
    ignored_paths
        .iter()
        .any(|ignored| path.starts_with(ignored))
}

/// Contenido de una carpeta: sus subcarpetas y, por directorio, sus PDF.
type FolderContents = (
    Vec<std::path::PathBuf>,
    std::collections::HashMap<String, Vec<String>>,
);

fn collect_folder_contents(folder: &std::path::Path) -> std::io::Result<FolderContents> {
    fn visit(
        dir: &std::path::Path,
        directories: &mut Vec<std::path::PathBuf>,
        files: &mut std::collections::HashMap<String, Vec<String>>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let path = entry.path();
            if file_type.is_dir() {
                directories.push(path.clone());
                visit(&path, directories, files)?;
            } else if file_type.is_file()
                && path
                    .extension()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
            {
                files
                    .entry(dir.to_string_lossy().into_owned())
                    .or_default()
                    .push(path.to_string_lossy().into_owned());
            }
        }
        Ok(())
    }

    if !folder.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "la ruta vinculada ya no existe",
        ));
    }
    let mut directories = vec![folder.to_path_buf()];
    let mut files = std::collections::HashMap::new();
    visit(folder, &mut directories, &mut files)?;
    Ok((directories, files))
}

// ─────────────────────────────────────────────────────────────────────────────
// Export / Import
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPayload {
    pub version: u32,
    pub exported_at: i64,
    pub categories: Vec<ExportCategory>,
    pub folders: Vec<ExportFolder>,
    pub documents: Vec<ExportDocument>,
    pub folder_docs: Vec<ExportFolderDoc>,
    pub bookmarks: Vec<ExportBookmark>,
    pub comments: Vec<ExportComment>,
    #[serde(default)]
    pub ignored_synced_paths: Vec<String>,
    #[serde(default)]
    pub synced_file_state: Vec<ExportSyncedFileState>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExportSyncedFileState {
    pub path: String,
    pub size_bytes: i64,
    pub modified_at: i64,
    pub error: Option<String>,
    #[serde(default)]
    pub device_id: Option<i64>,
    #[serde(default)]
    pub inode: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExportCategory {
    pub id: String,
    pub name: String,
    pub color: String,
    pub position: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExportFolder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub path: Option<String>,
    pub expanded: bool,
    pub position: i64,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExportDocument {
    pub id: String,
    pub path: String,
    pub title: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub pages: i64,
    pub size_bytes: Option<i64>,
    #[serde(default, skip_serializing)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub category_ids: sqlx::types::Json<Vec<String>>,
    pub last_page: i64,
    pub last_zoom: f64,
    pub in_library: bool,
    pub in_recent: bool,
    pub created_at: i64,
    /// Campos añadidos en la versión 2 del formato. `serde(default)` mantiene la
    /// lectura de los archivos de la versión 1, que no los traían: antes se
    /// perdían al exportar (huella, última apertura, vista y orden de la raíz).
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub last_opened: Option<i64>,
    #[serde(default)]
    pub view: Option<serde_json::Value>,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub view_state: Option<serde_json::Value>,
}

fn default_kind() -> String {
    "pdf".to_string()
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExportFolderDoc {
    pub folder_id: String,
    pub doc_id: String,
    pub position: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExportBookmark {
    pub id: String,
    pub doc_id: String,
    pub page: i64,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExportComment {
    pub id: String,
    pub doc_id: String,
    pub page: Option<i64>,
    pub text: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub inserted: usize,
    pub skipped: usize,
}

/// Versión del formato de exportación. La 4 añade el tipo de documento y el
/// estado de vista específico del módulo, manteniendo compatibilidad con los
/// formatos anteriores.
const EXPORT_VERSION: u32 = 4;

#[tauri::command]
pub async fn export_library() -> Result<String, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    export_library_inner(pool).await
}

async fn export_library_inner(pool: &sqlx::SqlitePool) -> Result<String, String> {
    let categories: Vec<ExportCategory> =
        sqlx::query_as("SELECT id, name, color, position FROM categories ORDER BY position ASC")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

    let folders: Vec<ExportFolder> = sqlx::query_as(
        "SELECT id, parent_id, name, kind, path, expanded, position, created_at \
         FROM folders ORDER BY parent_id IS NULL DESC, position ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let documents: Vec<ExportDocument> = sqlx::query_as(
        "SELECT id, path, title, kind, pages, size_bytes, category_id, \
                COALESCE((SELECT json_group_array(category_id) \
                          FROM document_categories WHERE doc_id = documents.id), '[]') \
                    AS category_ids, \
                last_page, last_zoom, in_library, in_recent, created_at, hash, last_opened, \
                view_json AS view, position, view_state \
         FROM documents ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let folder_docs: Vec<ExportFolderDoc> = sqlx::query_as(
        "SELECT folder_id, doc_id, position FROM folder_docs \
         ORDER BY folder_id, position ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let bookmarks: Vec<ExportBookmark> =
        sqlx::query_as("SELECT id, doc_id, page, name, created_at FROM bookmarks")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

    let comments: Vec<ExportComment> =
        sqlx::query_as("SELECT id, doc_id, page, text, created_at FROM comments")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    let ignored_synced_paths: Vec<String> =
        sqlx::query_scalar("SELECT path FROM ignored_synced_paths ORDER BY path")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    let synced_file_state: Vec<ExportSyncedFileState> = sqlx::query_as(
        "SELECT path, size_bytes, modified_at, error, device_id, inode \
         FROM synced_file_state ORDER BY path",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let payload = ExportPayload {
        version: EXPORT_VERSION,
        exported_at: now_ms(),
        categories,
        folders,
        documents,
        folder_docs,
        bookmarks,
        comments,
        ignored_synced_paths,
        synced_file_state,
    };

    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

/// Diagnóstico de un conjunto de carpetas que no se pudieron ordenar.
/// Separa las causas para que el mensaje al usuario sea accionable.
struct TopologyError {
    missing_parents: Vec<(String, String, String)>,
    cycles: Vec<(String, String)>,
}

impl TopologyError {
    fn is_empty(&self) -> bool {
        self.missing_parents.is_empty() && self.cycles.is_empty()
    }

    fn render(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if !self.missing_parents.is_empty() {
            let items: Vec<String> = self
                .missing_parents
                .iter()
                .map(|(id, name, parent)| format!("«{name}» ({id}) → padre {parent} no presente"))
                .collect();
            parts.push(format!(
                "Carpetas con padre inexistente: {}",
                items.join("; ")
            ));
        }

        if !self.cycles.is_empty() {
            let items: Vec<String> = self
                .cycles
                .iter()
                .map(|(id, name)| format!("«{name}» ({id})"))
                .collect();
            parts.push(format!("Carpetas en un ciclo: {}", items.join("; ")));
        }

        format!("No se puede importar. {}", parts.join(" "))
    }
}

fn analyze_topology(folders: &[ExportFolder]) -> TopologyError {
    use std::collections::{HashMap, HashSet};

    // Índice id → &ExportFolder para evitar O(N²) en las búsquedas.
    let by_id: HashMap<&str, &ExportFolder> = folders.iter().map(|f| (f.id.as_str(), f)).collect();
    let ids: HashSet<&str> = by_id.keys().copied().collect();

    let mut missing_parents: Vec<(String, String, String)> = Vec::new();
    let mut folder_ids_with_missing_parent: HashSet<&str> = HashSet::new();
    for folder in folders {
        if let Some(parent) = folder.parent_id.as_deref()
            && !ids.contains(parent)
        {
            missing_parents.push((folder.id.clone(), folder.name.clone(), parent.to_string()));
            folder_ids_with_missing_parent.insert(folder.id.as_str());
        }
    }

    // Detección de ciclos: seguir la cadena de padres desde cada carpeta.
    // Excluimos las que ya tienen padre inexistente.
    let mut cycles: Vec<(String, String)> = Vec::new();
    for folder in folders {
        if folder_ids_with_missing_parent.contains(folder.id.as_str()) {
            continue;
        }
        let mut visited: HashSet<&str> = HashSet::new();
        let mut current = Some(folder.id.as_str());
        let mut in_cycle = false;
        while let Some(id) = current {
            if !visited.insert(id) {
                in_cycle = true;
                break;
            }
            let Some(f) = by_id.get(id) else {
                break;
            };
            current = f.parent_id.as_deref();
        }
        if in_cycle {
            cycles.push((folder.id.clone(), folder.name.clone()));
        }
    }
    cycles.sort();
    cycles.dedup();

    TopologyError {
        missing_parents,
        cycles,
    }
}

/// Ordena carpetas por profundidad para que un padre siempre se inserte antes
/// que sus hijos. Devuelve error si alguna carpeta tiene un padre inexistente
/// o si el grafo forma un ciclo.
fn topological_sort_folders(folders: &[ExportFolder]) -> Result<Vec<&ExportFolder>, String> {
    let topology = analyze_topology(folders);
    if !topology.is_empty() {
        return Err(topology.render());
    }

    use std::collections::{HashMap, VecDeque};

    // Kahn: las raíces entran en la cola y cada carpeta se emite cuando ya lo ha
    // hecho su padre. Antes se recalculaban las profundidades por pasadas hasta
    // que no hubiera progreso, lo que es O(N²) con una cadena larga.
    let mut children: HashMap<&str, Vec<&ExportFolder>> = HashMap::new();
    let mut queue: VecDeque<&ExportFolder> = VecDeque::new();
    for folder in folders {
        match folder.parent_id.as_deref() {
            None => queue.push_back(folder),
            Some(parent) => children.entry(parent).or_default().push(folder),
        }
    }

    let mut ordered: Vec<&ExportFolder> = Vec::with_capacity(folders.len());
    while let Some(folder) = queue.pop_front() {
        ordered.push(folder);
        if let Some(kids) = children.remove(folder.id.as_str()) {
            queue.extend(kids);
        }
    }

    if ordered.len() != folders.len() {
        let missing: Vec<&str> = folders
            .iter()
            .filter(|f| !ordered.iter().any(|seen| seen.id == f.id))
            .map(|f| f.id.as_str())
            .collect();
        return Err(format!(
            "Error interno: {} carpetas sin orden asignado tras la validación: {}",
            missing.len(),
            missing.join(", "),
        ));
    }

    Ok(ordered)
}

#[tauri::command]
pub async fn import_library(json: String, mode: String) -> Result<ImportSummary, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    import_library_inner(pool, json, mode).await
}

async fn import_library_inner(
    pool: &sqlx::SqlitePool,
    json: String,
    mode: String,
) -> Result<ImportSummary, String> {
    let payload: ExportPayload =
        serde_json::from_str(&json).map_err(|e| format!("JSON inválido: {e}"))?;
    if payload.version > EXPORT_VERSION {
        return Err(format!(
            "El archivo es de una versión más nueva ({}) que esta app ({})",
            payload.version, EXPORT_VERSION,
        ));
    }

    // Validar la topología antes de abrir la transacción para no dejar la BD
    // a medias si el JSON tiene un ciclo o un padre inexistente.
    let ordered_folders = topological_sort_folders(&payload.folders)?;

    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let mut inserted = 0usize;
    let mut skipped = 0usize;
    let mut document_ids = std::collections::HashMap::<String, String>::new();
    let mut folder_ids = std::collections::HashMap::<String, String>::new();

    if mode == "replace" {
        for table in [
            "bookmarks",
            "comments",
            "folder_docs",
            "folders",
            "documents",
            "categories",
            // Rutas que ya no existen tras el reemplazo. Limpiarlas evita que
            // el sync ignore archivos nuevos por rutas antiguas.
            "ignored_synced_paths",
            "synced_file_state",
        ] {
            // Los nombres vienen de la lista literal de arriba, no del usuario.
            sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {table}")))
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        }
        if payload.categories.is_empty() {
            sqlx::query(
                "INSERT INTO categories (id, name, color, position) \
                 VALUES ('category-default', 'General', '#888888', 0)",
            )
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
            inserted += 1;
        }
    }

    for category in &payload.categories {
        let result = sqlx::query(
            "INSERT INTO categories (id, name, color, position) VALUES (?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, \
             color = excluded.color, position = excluded.position",
        )
        .bind(&category.id)
        .bind(&category.name)
        .bind(&category.color)
        .bind(category.position)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
        if result.rows_affected() > 0 {
            inserted += 1;
        } else {
            skipped += 1;
        }
    }

    for folder in ordered_folders {
        let stored_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM folders WHERE id = ? OR (path IS NOT NULL AND path = ?)",
        )
        .bind(&folder.id)
        .bind(&folder.path)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
        let stored_id = if let Some(id) = stored_id {
            sqlx::query(
                "UPDATE folders SET parent_id = ?, name = ?, kind = ?, path = ?, \
                 expanded = ?, position = ?, created_at = ? WHERE id = ?",
            )
            .bind(
                folder
                    .parent_id
                    .as_ref()
                    .and_then(|id| folder_ids.get(id))
                    .or(folder.parent_id.as_ref()),
            )
            .bind(&folder.name)
            .bind(&folder.kind)
            .bind(&folder.path)
            .bind(folder.expanded)
            .bind(folder.position)
            .bind(folder.created_at)
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
            skipped += 1;
            id
        } else {
            let parent_id = folder
                .parent_id
                .as_ref()
                .and_then(|id| folder_ids.get(id))
                .or(folder.parent_id.as_ref());
            sqlx::query(
                "INSERT INTO folders \
                 (id, parent_id, name, kind, path, expanded, position, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&folder.id)
            .bind(parent_id)
            .bind(&folder.name)
            .bind(&folder.kind)
            .bind(&folder.path)
            .bind(folder.expanded)
            .bind(folder.position)
            .bind(folder.created_at)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
            inserted += 1;
            folder.id.clone()
        };
        folder_ids.insert(folder.id.clone(), stored_id);
    }

    for doc in &payload.documents {
        let stored_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM documents WHERE id = ? OR path = ?")
                .bind(&doc.id)
                .bind(&doc.path)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        let stored_id = if let Some(id) = stored_id {
            sqlx::query(
                "UPDATE documents SET path = ?, title = ?, kind = ?, pages = ?, size_bytes = ?, \
                 hash = ?, last_page = ?, last_zoom = ?, last_opened = ?, in_recent = ?, \
                 in_library = ?, created_at = ?, view_json = ?, view_state = ?, position = ? \
                 WHERE id = ?",
            )
            .bind(&doc.path)
            .bind(&doc.title)
            .bind(&doc.kind)
            .bind(doc.pages)
            .bind(doc.size_bytes)
            .bind(&doc.hash)
            .bind(doc.last_page)
            .bind(doc.last_zoom)
            .bind(doc.last_opened)
            .bind(doc.in_recent)
            .bind(doc.in_library)
            .bind(doc.created_at)
            .bind(
                doc.view
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "{}".into()),
            )
            .bind(
                doc.view_state
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "{}".into()),
            )
            .bind(doc.position)
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query("DELETE FROM document_categories WHERE doc_id = ?")
                .bind(&id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            skipped += 1;
            id
        } else {
            sqlx::query(
                "INSERT INTO documents \
             (id, path, title, kind, pages, size_bytes, hash, category_id, last_page, \
              last_zoom, last_opened, in_recent, in_library, created_at, view_json, view_state, position) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&doc.id)
            .bind(&doc.path)
            .bind(&doc.title)
            .bind(&doc.kind)
            .bind(doc.pages)
            .bind(doc.size_bytes)
            .bind(&doc.hash)
            .bind(doc.category_ids.0.first().or(doc.category_id.as_ref()))
            .bind(doc.last_page)
            .bind(doc.last_zoom)
            .bind(doc.last_opened)
            .bind(doc.in_recent)
            .bind(doc.in_library)
            .bind(doc.created_at)
            .bind(
                doc.view
                    .as_ref()
                    .map(|view| view.to_string())
                    .unwrap_or_else(|| "{}".to_string()),
            )
            .bind(
                doc.view_state
                    .as_ref()
                    .map(|state| state.to_string())
                    .unwrap_or_else(|| "{}".to_string()),
            )
            .bind(doc.position)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
            inserted += 1;
            doc.id.clone()
        };
        document_ids.insert(doc.id.clone(), stored_id.clone());

        {
            let category_ids = if doc.category_ids.0.is_empty() {
                doc.category_id
                    .as_ref()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                doc.category_ids.0.clone()
            };
            for category_id in category_ids {
                let category_exists: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM categories WHERE id = ?")
                        .bind(&category_id)
                        .fetch_optional(&mut *transaction)
                        .await
                        .map_err(|e| e.to_string())?;
                if category_exists.is_none() {
                    // Older exports can contain a document-category relation
                    // without the category itself. Ignore that relation rather
                    // than letting SQLite abort the whole import.
                    skipped += 1;
                    continue;
                }
                sqlx::query(
                    "INSERT OR IGNORE INTO document_categories (doc_id, category_id) \
                     VALUES (?, ?)",
                )
                .bind(&stored_id)
                .bind(category_id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
            }
            sqlx::query(
                "UPDATE documents SET category_id = ( \
                   SELECT category_id FROM document_categories \
                   WHERE doc_id = ? ORDER BY rowid LIMIT 1 \
                 ) WHERE id = ?",
            )
            .bind(&stored_id)
            .bind(&stored_id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    for fd in &payload.folder_docs {
        let folder_id = folder_ids
            .get(&fd.folder_id)
            .cloned()
            .unwrap_or_else(|| fd.folder_id.clone());
        let Some(doc_id) = document_ids.get(&fd.doc_id) else {
            skipped += 1;
            continue;
        };
        let result = sqlx::query(
            "INSERT OR IGNORE INTO folder_docs (folder_id, doc_id, position) \
             VALUES (?, ?, ?)",
        )
        .bind(&folder_id)
        .bind(doc_id)
        .bind(fd.position)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
        if result.rows_affected() > 0 {
            inserted += 1;
        } else {
            skipped += 1;
        }
    }

    for bm in &payload.bookmarks {
        let Some(doc_id) = document_ids.get(&bm.doc_id) else {
            skipped += 1;
            continue;
        };
        let result = sqlx::query(
            "INSERT OR IGNORE INTO bookmarks (id, doc_id, page, name, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&bm.id)
        .bind(doc_id)
        .bind(bm.page)
        .bind(&bm.name)
        .bind(bm.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
        if result.rows_affected() > 0 {
            inserted += 1;
        } else {
            skipped += 1;
        }
    }

    for cm in &payload.comments {
        let Some(doc_id) = document_ids.get(&cm.doc_id) else {
            skipped += 1;
            continue;
        };
        let result = sqlx::query(
            "INSERT OR IGNORE INTO comments (id, doc_id, page, text, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&cm.id)
        .bind(doc_id)
        .bind(cm.page)
        .bind(&cm.text)
        .bind(cm.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
        if result.rows_affected() > 0 {
            inserted += 1;
        } else {
            skipped += 1;
        }
    }

    for path in &payload.ignored_synced_paths {
        sqlx::query("INSERT OR IGNORE INTO ignored_synced_paths (path) VALUES (?)")
            .bind(path)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }
    for state in &payload.synced_file_state {
        sqlx::query(
            "INSERT INTO synced_file_state \
             (path, size_bytes, modified_at, error, device_id, inode) \
             VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(path) DO UPDATE SET \
             size_bytes = excluded.size_bytes, modified_at = excluded.modified_at, \
             error = excluded.error, device_id = excluded.device_id, inode = excluded.inode",
        )
        .bind(&state.path)
        .bind(state.size_bytes)
        .bind(state.modified_at)
        .bind(&state.error)
        .bind(state.device_id)
        .bind(state.inode)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    }

    // Imported root positions may contain gaps, duplicate values, or positions
    // from a different library. Normalize them after all folder links exist.
    let root_ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM documents \
         WHERE in_library = 1 AND id NOT IN (SELECT doc_id FROM folder_docs) \
         ORDER BY position ASC, created_at ASC, id ASC",
    )
    .fetch_all(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?;
    for (position, id) in root_ids.iter().enumerate() {
        sqlx::query("UPDATE documents SET position = ? WHERE id = ?")
            .bind(position as i64)
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }

    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(ImportSummary { inserted, skipped })
}

// ─────────────────────────────────────────────────────────────────────────────
// Export / Import a archivo
// ─────────────────────────────────────────────────────────────────────────────

/// Exporta la biblioteca a un archivo elegido por el usuario.
///
/// Devuelve la ruta escrita, o `None` si el usuario cancela el diálogo.
/// El acceso a disco se queda en Rust: la webview no recibe permisos de `fs`.
#[tauri::command]
pub async fn export_library_to_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let json = export_library().await?;
    let choice = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Exportar biblioteca")
            .set_file_name("ioun-library.json")
            .add_filter("JSON", &["json"])
            .blocking_save_file()
    })
    .await
    .map_err(|error| format!("El diálogo no se pudo abrir: {error}"))?;

    let Some(path) = choice else { return Ok(None) };
    let path = path.into_path().map_err(|e| e.to_string())?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("No se pudo escribir el archivo: {e}"))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Importa una biblioteca desde un archivo elegido por el usuario.
///
/// `mode` es `"merge"` o `"replace"`. Devuelve `None` si el usuario cancela.
#[tauri::command]
pub async fn import_library_from_file(
    app: tauri::AppHandle,
    mode: String,
) -> Result<Option<ImportSummary>, String> {
    let choice = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Importar biblioteca")
            .add_filter("JSON", &["json"])
            .blocking_pick_file()
    })
    .await
    .map_err(|error| format!("El diálogo no se pudo abrir: {error}"))?;

    let Some(path) = choice else { return Ok(None) };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let json = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    import_library(json, mode).await.map(Some)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod path_rename_tests {
    use super::rebase_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn rebases_file_paths_under_renamed_directory() {
        assert_eq!(
            rebase_path(
                Path::new("/library/old/report.pdf"),
                Path::new("/library/old"),
                Path::new("/library/new")
            ),
            Some(PathBuf::from("/library/new/report.pdf"))
        );
    }

    #[test]
    fn does_not_rebase_a_similarly_prefixed_sibling() {
        assert_eq!(
            rebase_path(
                Path::new("/library/old-docs/report.pdf"),
                Path::new("/library/old"),
                Path::new("/library/new")
            ),
            None
        );
    }

    #[test]
    fn ignores_descendants_but_not_similarly_prefixed_siblings() {
        let ignored_paths = vec![PathBuf::from("/library/hidden")];

        assert!(super::is_ignored_path(
            Path::new("/library/hidden/book.pdf"),
            &ignored_paths
        ));
        assert!(!super::is_ignored_path(
            Path::new("/library/hidden-copy/book.pdf"),
            &ignored_paths
        ));
    }

    #[tokio::test]
    async fn folder_contents_query_keeps_documents_linked_outside_the_subtree() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for statement in [
            "CREATE TABLE folders (id TEXT PRIMARY KEY, parent_id TEXT)",
            "CREATE TABLE documents (id TEXT PRIMARY KEY, path TEXT NOT NULL)",
            "CREATE TABLE folder_docs (folder_id TEXT NOT NULL, doc_id TEXT NOT NULL)",
            "INSERT INTO folders VALUES ('root', NULL), ('child', 'root'), ('other', NULL)",
            "INSERT INTO documents VALUES ('root-doc', '/root/a.pdf'), ('child-doc', '/root/child/b.pdf'), ('shared-doc', '/root/child/c.pdf')",
            "INSERT INTO folder_docs VALUES ('root', 'root-doc'), ('child', 'child-doc'), ('child', 'shared-doc'), ('other', 'shared-doc')",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }

        let selected = super::documents_in_subtree(&pool, "root").await.unwrap();
        let selected: std::collections::HashSet<String> =
            selected.into_iter().map(|(id, _)| id).collect();
        assert_eq!(
            selected,
            std::collections::HashSet::from(["root-doc".into(), "child-doc".into()])
        );
    }
}

#[cfg(test)]
mod add_folder_tests {
    use super::*;
    use crate::db::test_pool;

    async fn seed_folder(pool: &sqlx::SqlitePool, id: &str, parent: Option<&str>, kind: &str) {
        sqlx::query(
            "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) \
             VALUES (?, ?, ?, ?, NULL, 1, 0, 0)",
        )
        .bind(id)
        .bind(parent)
        .bind(id)
        .bind(kind)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn synced_subfolders_are_created_collapsed() {
        let pool = test_pool().await;
        seed_folder(&pool, "root", None, "synced").await;

        let dir = std::env::temp_dir().to_string_lossy().into_owned();
        let mut conn = pool.acquire().await.unwrap();
        let record = add_folder_inner(
            &mut conn,
            "sub".into(),
            "synced".into(),
            Some("root".into()),
            Some(dir),
            false,
        )
        .await
        .unwrap();
        drop(conn);

        assert!(!record.expanded);
        let stored: bool = sqlx::query_scalar("SELECT expanded FROM folders WHERE id = ?")
            .bind(&record.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!stored);
    }

    /// El prefiltro SQL (`LIKE 'prefijo%'`) trae de más: una carpeta ignorada con
    /// el mismo texto inicial pero fuera de la raíz no se debe des-ignorar.
    #[tokio::test]
    async fn linking_a_root_only_clears_the_ignored_paths_under_it() {
        let pool = test_pool().await;

        let tmp = std::env::temp_dir().join(format!(
            "ioun-ignore-scope-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let root = tmp.join("lib");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::create_dir_all(tmp.join("library")).unwrap();
        for path in [
            root.join("sub/interno.pdf"),
            tmp.join("library/externo.pdf"),
        ] {
            sqlx::query("INSERT INTO ignored_synced_paths (path) VALUES (?)")
                .bind(path.to_str().unwrap())
                .execute(&pool)
                .await
                .unwrap();
        }

        let mut conn = pool.acquire().await.unwrap();
        add_folder_inner(
            &mut conn,
            "Vinculada".into(),
            "synced".into(),
            None,
            Some(root.to_str().unwrap().into()),
            false,
        )
        .await
        .unwrap();
        drop(conn);

        let remaining: Vec<String> = sqlx::query_scalar("SELECT path FROM ignored_synced_paths")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(
            remaining,
            vec![
                tmp.join("library/externo.pdf")
                    .to_string_lossy()
                    .into_owned()
            ]
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[tokio::test]
    async fn folders_created_by_the_user_still_start_expanded() {
        let pool = test_pool().await;

        let record = {
            let mut conn = pool.acquire().await.unwrap();
            add_folder_inner(&mut conn, "Nueva".into(), "temp".into(), None, None, true)
                .await
                .unwrap()
        };

        assert!(record.expanded);
    }
}

#[cfg(test)]
mod add_file_tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn refreshing_a_zero_page_document_preserves_the_last_page() {
        let pool = test_pool().await;
        let path = std::env::temp_dir().join(format!(
            "ioun-add-file-{}.pdf",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, b"%PDF-1.4 test").unwrap();
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, last_page, last_zoom, \
                                    in_recent, in_library, created_at, view_json, position) \
             VALUES ('doc-1', ?, 'test.pdf', 0, 1, 57, 100, 0, 1, 0, '{}', 0)",
        )
        .bind(path.to_string_lossy().as_ref())
        .execute(&pool)
        .await
        .unwrap();

        let mut conn = pool.acquire().await.unwrap();
        add_file_inner(&mut conn, path.to_string_lossy().as_ref(), None, 0)
            .await
            .unwrap();
        drop(conn);
        let last_page: i64 =
            sqlx::query_scalar("SELECT last_page FROM documents WHERE id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(last_page, 57);
        std::fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod root_position_tests {
    use super::*;
    use crate::db::test_pool;

    async fn insert_document(pool: &sqlx::SqlitePool, id: &str, in_library: i64, position: i64) {
        sqlx::query(
            "INSERT INTO documents \
             (id, path, title, pages, size_bytes, last_page, last_zoom, in_recent, in_library, \
              created_at, view_json, position) \
             VALUES (?, ?, ?, 1, 1, 1, 100, 0, ?, 0, '{}', ?)",
        )
        .bind(id)
        .bind(format!("/{id}.pdf"))
        .bind(id)
        .bind(in_library)
        .bind(position)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn deleting_a_folder_without_contents_appends_orphans_to_the_root() {
        let pool = test_pool().await;
        let folder_id = {
            let mut conn = pool.acquire().await.unwrap();
            add_folder_inner(&mut conn, "Carpeta".into(), "temp".into(), None, None, true)
                .await
                .unwrap()
                .id
        };
        insert_document(&pool, "root-doc", 1, 0).await;
        insert_document(&pool, "orphan-doc", 1, 42).await;
        sqlx::query("INSERT INTO folder_docs (folder_id, doc_id, position) VALUES (?, ?, 0)")
            .bind(&folder_id)
            .bind("orphan-doc")
            .execute(&pool)
            .await
            .unwrap();

        delete_folder_inner(&pool, folder_id, false).await.unwrap();

        let positions: Vec<(String, i64)> = sqlx::query_as(
            "SELECT id, position FROM documents WHERE in_library = 1 \
             AND id NOT IN (SELECT doc_id FROM folder_docs) ORDER BY position",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            positions,
            vec![("root-doc".to_string(), 0), ("orphan-doc".to_string(), 1)]
        );
    }

    #[tokio::test]
    async fn restoring_a_removed_document_appends_it_to_the_root() {
        let pool = test_pool().await;
        insert_document(&pool, "root-doc", 1, 0).await;
        insert_document(&pool, "removed-doc", 0, 99).await;

        restore_document_to_library_inner(&pool, "removed-doc".into())
            .await
            .unwrap();

        let positions: Vec<(String, i64)> = sqlx::query_as(
            "SELECT id, position FROM documents WHERE in_library = 1 \
             AND id NOT IN (SELECT doc_id FROM folder_docs) ORDER BY position",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            positions,
            vec![("root-doc".to_string(), 0), ("removed-doc".to_string(), 1)]
        );
    }
}

#[cfg(test)]
mod lazy_tree_tests {
    use super::*;
    use crate::db::test_pool;

    async fn seed(pool: &sqlx::SqlitePool) {
        for statement in [
            "INSERT INTO folders (id, parent_id, name, kind, expanded, position, created_at) \
             VALUES ('root', NULL, 'Root', 'synced', 1, 0, 0), \
                    ('sub-a', 'root', 'A', 'synced', 0, 0, 0), \
                    ('sub-b', 'root', 'B', 'synced', 0, 1, 0), \
                    ('deep', 'sub-a', 'Deep', 'synced', 0, 0, 0)",
            "INSERT INTO documents (id, path, title, pages, created_at, in_library, position) \
             VALUES ('root-doc', '/r.pdf', 'r', 1, 0, 1, 0), \
                    ('deep-doc', '/d.pdf', 'd', 1, 0, 1, 0)",
            "INSERT INTO folder_docs (folder_id, doc_id, position) VALUES ('deep', 'deep-doc', 0)",
        ] {
            sqlx::query(statement).execute(pool).await.unwrap();
        }
    }

    #[tokio::test]
    async fn root_snapshot_brings_roots_and_the_complete_document_map() {
        let pool = test_pool().await;
        seed(&pool).await;

        let snapshot = list_library_root_inner(&pool).await.unwrap();

        let ids: Vec<&str> = snapshot.folders.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["root"]);
        // El mapa de documentos sigue completo: el índice docId → carpeta del
        // frontend depende de él.
        assert_eq!(
            snapshot.folder_docs.get("deep"),
            Some(&vec!["deep-doc".to_string()])
        );
        assert_eq!(snapshot.root_doc_ids, vec!["root-doc".to_string()]);
    }

    #[tokio::test]
    async fn children_are_loaded_one_level_at_a_time() {
        let pool = test_pool().await;
        seed(&pool).await;

        let children = list_folder_children_inner(&pool, "root").await.unwrap();
        let ids: Vec<&str> = children.folders.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["sub-a", "sub-b"]);

        let deep = list_folder_children_inner(&pool, "sub-a").await.unwrap();
        let ids: Vec<&str> = deep.folders.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["deep"]);
    }
}

#[cfg(test)]
mod move_folder_tests {
    use super::*;
    use crate::db::test_pool;

    async fn seed_folder(pool: &sqlx::SqlitePool, id: &str, parent: Option<&str>, kind: &str) {
        sqlx::query(
            "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) \
             VALUES (?, ?, ?, ?, NULL, 1, 0, 0)",
        )
        .bind(id)
        .bind(parent)
        .bind(id)
        .bind(kind)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn moving_synced_root_without_flag_is_rejected() {
        let pool = test_pool().await;
        seed_folder(&pool, "root", None, "synced").await;
        seed_folder(&pool, "dest", None, "temp").await;

        let result = move_folder_inner(&pool, "root".into(), Some("dest".into()), 0, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("sincronizada raíz"), "error inesperado: {err}");

        let parent: Option<String> =
            sqlx::query_scalar("SELECT parent_id FROM folders WHERE id = 'root'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(parent.is_none());
    }

    #[tokio::test]
    async fn moving_synced_root_with_flag_is_allowed() {
        let pool = test_pool().await;
        seed_folder(&pool, "root", None, "synced").await;
        seed_folder(&pool, "dest", None, "temp").await;

        move_folder_inner(&pool, "root".into(), Some("dest".into()), 0, Some(true))
            .await
            .expect("move permitido");

        let parent: Option<String> =
            sqlx::query_scalar("SELECT parent_id FROM folders WHERE id = 'root'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(parent.as_deref(), Some("dest"));

        let kind: String = sqlx::query_scalar("SELECT kind FROM folders WHERE id = 'root'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(kind, "synced");
    }

    #[tokio::test]
    async fn moving_synced_subfolder_is_allowed_without_flag() {
        let pool = test_pool().await;
        seed_folder(&pool, "root", None, "synced").await;
        seed_folder(&pool, "child", Some("root"), "synced").await;
        seed_folder(&pool, "dest", None, "temp").await;

        move_folder_inner(&pool, "child".into(), Some("dest".into()), 0, None)
            .await
            .expect("subcarpeta sincronizada movible");

        let parent: Option<String> =
            sqlx::query_scalar("SELECT parent_id FROM folders WHERE id = 'child'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(parent.as_deref(), Some("dest"));
    }

    #[tokio::test]
    async fn moving_folder_into_itself_is_rejected() {
        let pool = test_pool().await;
        seed_folder(&pool, "a", None, "temp").await;

        let result = move_folder_inner(&pool, "a".into(), Some("a".into()), 0, Some(true)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn moving_folder_into_descendant_is_rejected() {
        let pool = test_pool().await;
        seed_folder(&pool, "a", None, "temp").await;
        seed_folder(&pool, "b", Some("a"), "temp").await;
        seed_folder(&pool, "c", Some("b"), "temp").await;

        let result = move_folder_inner(&pool, "a".into(), Some("c".into()), 0, Some(true)).await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod import_topology_tests {
    use super::{ExportFolder, topological_sort_folders};

    fn folder(id: &str, parent: Option<&str>) -> ExportFolder {
        ExportFolder {
            id: id.into(),
            parent_id: parent.map(|s| s.into()),
            name: id.into(),
            kind: "temp".into(),
            path: None,
            expanded: true,
            position: 0,
            created_at: 0,
        }
    }

    #[test]
    fn orders_parents_before_children() {
        let folders = vec![
            folder("c", Some("b")),
            folder("b", Some("a")),
            folder("a", None),
        ];
        let ordered = topological_sort_folders(&folders).unwrap();
        let ids: Vec<&str> = ordered.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn orders_a_deep_chain_parent_before_child() {
        // Cadena larga: el algoritmo anterior asignaba profundidades por pasadas
        // hasta que no hubiera progreso, así que con N eslabones hacía N pasadas.
        let depth = 500;
        let folders: Vec<ExportFolder> = (0..depth)
            .map(|index| {
                folder(
                    &format!("f{index}"),
                    (index > 0).then(|| format!("f{}", index - 1)).as_deref(),
                )
            })
            .collect();

        let ordered = topological_sort_folders(&folders).unwrap();
        let ids: Vec<String> = ordered.iter().map(|f| f.id.clone()).collect();
        let expected: Vec<String> = (0..depth).map(|index| format!("f{index}")).collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn handles_multiple_roots_and_branches() {
        let folders = vec![
            folder("child1", Some("root1")),
            folder("root2", None),
            folder("child2", Some("root2")),
            folder("root1", None),
        ];
        let ordered = topological_sort_folders(&folders).unwrap();
        let pos = |id: &str| ordered.iter().position(|f| f.id == id).unwrap();
        assert!(pos("root1") < pos("child1"));
        assert!(pos("root2") < pos("child2"));
    }

    #[test]
    fn rejects_missing_parent() {
        let folders = vec![folder("a", None), folder("orphan", Some("nonexistent"))];
        let error = topological_sort_folders(&folders).unwrap_err();
        assert!(error.contains("padre inexistente"), "mensaje: {error}");
        assert!(error.contains("orphan"));
    }

    #[test]
    fn rejects_cycle() {
        let folders = vec![folder("a", Some("b")), folder("b", Some("a"))];
        let error = topological_sort_folders(&folders).unwrap_err();
        assert!(error.contains("ciclo"), "mensaje: {error}");
    }

    #[test]
    fn rejects_self_reference() {
        let folders = vec![folder("a", Some("a"))];
        let error = topological_sort_folders(&folders).unwrap_err();
        assert!(error.contains("ciclo"));
        assert!(!error.contains("padre inexistente"), "mensaje: {error}");
    }

    #[test]
    fn reports_both_missing_parent_and_cycle() {
        let folders = vec![
            folder("a", Some("b")),
            folder("b", Some("a")),
            folder("c", Some("ghost")),
            folder("root", None),
        ];
        let error = topological_sort_folders(&folders).unwrap_err();
        assert!(error.contains("padre inexistente"), "mensaje: {error}");
        assert!(error.contains("ciclo"), "mensaje: {error}");
        assert!(error.contains("ghost"), "mensaje: {error}");
    }

    #[test]
    fn empty_input_is_ok() {
        let ordered = topological_sort_folders(&[]).unwrap();
        assert!(ordered.is_empty());
    }

    #[test]
    fn deep_chain() {
        let mut folders = vec![folder("f0", None)];
        for i in 1..100 {
            let parent = format!("f{}", i - 1);
            let child = format!("f{}", i);
            folders.push(folder(&child, Some(&parent)));
        }
        let ordered = topological_sort_folders(&folders).unwrap();
        assert_eq!(ordered.len(), 100);
        for (i, f) in ordered.iter().enumerate() {
            assert_eq!(f.id, format!("f{i}"));
        }
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;
    use crate::db::test_pool;

    async fn insert_doc(pool: &sqlx::SqlitePool, id: &str, path: &str) {
        sqlx::query(
            "INSERT INTO documents \
             (id, path, title, pages, size_bytes, hash, category_id, last_page, last_zoom, \
              last_opened, in_recent, in_library, created_at, view_json, position) \
             VALUES (?, ?, ?, 1, 0, NULL, NULL, 1, 100, NULL, 0, 1, 0, '{}', 0)",
        )
        .bind(id)
        .bind(path)
        .bind(path)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn rename_merges_documents_when_target_path_already_exists() {
        let pool = test_pool().await;

        let tmp = std::env::temp_dir().join(format!(
            "ioun-rename-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let old_path = tmp.join("old.pdf");
        let new_path = tmp.join("new.pdf");
        std::fs::write(&old_path, b"%PDF-1.4 old").unwrap();
        std::fs::write(&new_path, b"%PDF-1.4 new").unwrap();

        insert_doc(&pool, "doc-old", old_path.to_str().unwrap()).await;
        insert_doc(&pool, "doc-new", new_path.to_str().unwrap()).await;

        sqlx::query(
            "INSERT INTO bookmarks (id, doc_id, page, name, created_at) \
             VALUES ('bm-1', 'doc-old', 3, '', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO comments (id, doc_id, page, text, created_at) \
             VALUES ('cm-1', 'doc-old', 2, 'nota', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        update_paths_after_rename_inner(&pool, &old_path, &new_path)
            .await
            .unwrap();

        let doc_old_exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM documents WHERE id = 'doc-old'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(doc_old_exists.is_none());

        let bm_doc: String = sqlx::query_scalar("SELECT doc_id FROM bookmarks WHERE id = 'bm-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(bm_doc, "doc-new");

        let cm_doc: String = sqlx::query_scalar("SELECT doc_id FROM comments WHERE id = 'cm-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(cm_doc, "doc-new");

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// El prefiltro SQL (`LIKE 'prefijo%'`) trae de más: una carpeta hermana que
    /// empieza por el mismo texto no se debe tocar.
    #[tokio::test]
    async fn rename_only_rebases_the_subtree() {
        let pool = test_pool().await;

        let tmp = std::env::temp_dir().join(format!(
            "ioun-rename-subtree-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let old_path = tmp.join("lib");
        let new_path = tmp.join("lib-renombrada");
        std::fs::create_dir_all(old_path.join("sub")).unwrap();
        std::fs::create_dir_all(tmp.join("library")).unwrap();
        std::fs::create_dir_all(&new_path).unwrap();

        insert_doc(
            &pool,
            "doc-inside",
            old_path.join("a.pdf").to_str().unwrap(),
        )
        .await;
        insert_doc(
            &pool,
            "doc-nested",
            old_path.join("sub/b.pdf").to_str().unwrap(),
        )
        .await;
        insert_doc(
            &pool,
            "doc-sibling",
            tmp.join("library/x.pdf").to_str().unwrap(),
        )
        .await;
        sqlx::query(
            "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) \
             VALUES ('f-root', NULL, 'lib', 'synced', ?, 1, 0, 0)",
        )
        .bind(old_path.to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) \
             VALUES ('f-sub', 'f-root', 'sub', 'synced', ?, 1, 0, 0)",
        )
        .bind(old_path.join("sub").to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO synced_file_state (path, size_bytes, modified_at, error) VALUES (?, 1, 1, NULL)",
        )
        .bind(old_path.join("sub/b.pdf").to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO ignored_synced_paths (path) VALUES (?)")
            .bind(old_path.join("ignorado.pdf").to_str().unwrap())
            .execute(&pool)
            .await
            .unwrap();

        assert!(
            update_paths_after_rename_inner(&pool, &old_path, &new_path)
                .await
                .unwrap()
        );

        let docs: Vec<(String, String)> =
            sqlx::query_as("SELECT id, path FROM documents ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            docs,
            vec![
                (
                    "doc-inside".to_string(),
                    new_path.join("a.pdf").to_string_lossy().into_owned()
                ),
                (
                    "doc-nested".to_string(),
                    new_path.join("sub/b.pdf").to_string_lossy().into_owned()
                ),
                (
                    "doc-sibling".to_string(),
                    tmp.join("library/x.pdf").to_string_lossy().into_owned()
                ),
            ]
        );

        let folders: Vec<(String, String)> =
            sqlx::query_as("SELECT id, path FROM folders ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            folders,
            vec![
                (
                    "f-root".to_string(),
                    new_path.to_string_lossy().into_owned()
                ),
                (
                    "f-sub".to_string(),
                    new_path.join("sub").to_string_lossy().into_owned()
                ),
            ]
        );

        let state: String = sqlx::query_scalar("SELECT path FROM synced_file_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(state, new_path.join("sub/b.pdf").to_string_lossy());
        let ignored: String = sqlx::query_scalar("SELECT path FROM ignored_synced_paths")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(ignored, new_path.join("ignorado.pdf").to_string_lossy());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[tokio::test]
    async fn rename_updates_path_without_collision() {
        let pool = test_pool().await;

        let tmp = std::env::temp_dir().join(format!(
            "ioun-rename-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let old_path = tmp.join("old.pdf");
        let new_path = tmp.join("new.pdf");
        std::fs::write(&old_path, b"%PDF-1.4").unwrap();
        std::fs::write(&new_path, b"%PDF-1.4").unwrap();

        insert_doc(&pool, "doc-1", old_path.to_str().unwrap()).await;

        update_paths_after_rename_inner(&pool, &old_path, &new_path)
            .await
            .unwrap();

        let stored: String = sqlx::query_scalar("SELECT path FROM documents WHERE id = 'doc-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, new_path.to_str().unwrap());

        let title: String = sqlx::query_scalar("SELECT title FROM documents WHERE id = 'doc-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(title, "new.pdf");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[tokio::test]
    async fn rename_collision_merges_folder_links_without_unique_violation() {
        let pool = test_pool().await;
        let tmp = std::env::temp_dir().join(format!(
            "ioun-rename-links-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let old_path = tmp.join("old.pdf");
        let new_path = tmp.join("new.pdf");
        std::fs::write(&old_path, b"%PDF-1.4 old").unwrap();
        std::fs::write(&new_path, b"%PDF-1.4 new").unwrap();

        insert_doc(&pool, "doc-old", old_path.to_str().unwrap()).await;
        insert_doc(&pool, "doc-new", new_path.to_str().unwrap()).await;
        for (id, name) in [("folder-old", "Old"), ("folder-new", "New")] {
            sqlx::query(
                "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) \
                 VALUES (?, NULL, ?, 'temp', NULL, 1, 0, 0)",
            )
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO folder_docs (folder_id, doc_id, position) \
             VALUES ('folder-old', 'doc-old', 0), ('folder-new', 'doc-new', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        update_paths_after_rename_inner(&pool, &old_path, &new_path)
            .await
            .unwrap();

        let links: Vec<(String, String)> =
            sqlx::query_as("SELECT folder_id, doc_id FROM folder_docs ORDER BY folder_id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            links,
            vec![("folder-new".to_string(), "doc-new".to_string())]
        );
        std::fs::remove_dir_all(&tmp).ok();
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;
    use crate::db::test_pool;

    async fn seed(pool: &sqlx::SqlitePool) {
        sqlx::query(
            "INSERT INTO documents (id, path, title, kind, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, last_opened, in_recent, in_library, \
                                    created_at, view_json, view_state, position) \
             VALUES ('doc-1', '/a.pdf', 'a.pdf', 'pdf', 10, 2048, 'huella-1', NULL, \
                     5, 175.0, 999, 1, 1, 7, '{\"pagesPerRow\":3}', \
                     '{\"page\":5,\"zoom\":175,\"continuousScroll\":true}', 4)",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    /// La exportación dejaba fuera la huella, la última apertura, la vista por
    /// documento y la posición en la raíz: al importar en otro equipo se perdían
    /// en silencio.
    #[tokio::test]
    async fn export_and_import_round_trip_keeps_every_document_field() {
        let source = test_pool().await;
        seed(&source).await;
        let json = export_library_inner(&source).await.unwrap();

        let destination = test_pool().await;
        import_library_inner(&destination, json, "merge".to_string())
            .await
            .unwrap();

        let (kind, hash, last_opened, view, view_state, position): (
            String,
            Option<String>,
            Option<i64>,
            String,
            String,
            i64,
        ) = sqlx::query_as(
            "SELECT kind, hash, last_opened, view_json, view_state, position \
                 FROM documents WHERE id = 'doc-1'",
        )
        .fetch_one(&destination)
        .await
        .unwrap();

        assert_eq!(kind, "pdf");
        assert_eq!(hash.as_deref(), Some("huella-1"));
        assert_eq!(last_opened, Some(999));
        assert_eq!(view, "{\"pagesPerRow\":3}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&view_state).unwrap(),
            serde_json::json!({"page": 5, "zoom": 175, "continuousScroll": true})
        );
        assert_eq!(position, 0);
    }

    /// Un archivo de la versión 1 no trae esos campos: se sigue importando, con
    /// los valores por defecto.
    #[tokio::test]
    async fn import_accepts_a_version_1_file() {
        let pool = test_pool().await;
        let json = serde_json::json!({
            "version": 1,
            "exportedAt": 0,
            "categories": [],
            "folders": [],
            "documents": [{
                "id": "doc-1", "path": "/a.pdf", "title": "a.pdf", "pages": 10,
                "sizeBytes": 2048, "categoryId": null, "lastPage": 5, "lastZoom": 175.0,
                "inLibrary": true, "inRecent": true, "createdAt": 7
            }],
            "folderDocs": [],
            "bookmarks": [],
            "comments": []
        })
        .to_string();

        let summary = import_library_inner(&pool, json, "merge".to_string())
            .await
            .unwrap();
        assert_eq!(summary.inserted, 1);

        let (hash, view, position): (Option<String>, String, i64) =
            sqlx::query_as("SELECT hash, view_json, position FROM documents WHERE id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(hash, None);
        assert_eq!(view, "{}");
        assert_eq!(position, 0);
    }

    #[tokio::test]
    async fn merge_import_remaps_relations_when_document_path_collides() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO categories (id, name, color, position) VALUES ('cat-1', 'Trabajo', '#fff', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) \
             VALUES ('folder-1', NULL, 'Importada', 'temp', NULL, 1, 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, in_recent, in_library, created_at, \
                                    view_json, position) \
             VALUES ('stored-doc', '/shared.pdf', 'shared.pdf', 1, 1, NULL, NULL, 1, 100, 0, 1, 0, '{}', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let json = serde_json::json!({
            "version": 3,
            "exportedAt": 0,
            "categories": [],
            "folders": [],
            "documents": [{
                "id": "imported-doc", "path": "/shared.pdf", "title": "shared.pdf",
                "pages": 1, "sizeBytes": 1, "categoryId": "cat-1", "categoryIds": ["cat-1"],
                "lastPage": 1, "lastZoom": 100, "inLibrary": true, "inRecent": false,
                "createdAt": 0, "hash": null, "lastOpened": null, "view": {}, "position": 0
            }],
            "folderDocs": [{"folderId": "folder-1", "docId": "imported-doc", "position": 0}],
            "bookmarks": [{"id": "bm-imported", "docId": "imported-doc", "page": 1, "name": "B", "createdAt": 0}],
            "comments": [{"id": "cm-imported", "docId": "imported-doc", "page": 1, "text": "C", "createdAt": 0}]
        }).to_string();

        import_library_inner(&pool, json, "merge".to_string())
            .await
            .unwrap();

        let bookmark_doc: String =
            sqlx::query_scalar("SELECT doc_id FROM bookmarks WHERE id = 'bm-imported'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let comment_doc: String =
            sqlx::query_scalar("SELECT doc_id FROM comments WHERE id = 'cm-imported'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let folder_doc: String =
            sqlx::query_scalar("SELECT doc_id FROM folder_docs WHERE folder_id = 'folder-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(bookmark_doc, "stored-doc");
        assert_eq!(comment_doc, "stored-doc");
        assert_eq!(folder_doc, "stored-doc");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM document_categories WHERE doc_id = 'stored-doc' AND category_id = 'cat-1'",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;
    use crate::db::test_pool;

    /// Árbol temporal con una raíz vinculada ya creada en la base.
    struct SyncedTree {
        dir: std::path::PathBuf,
        pool: sqlx::SqlitePool,
        root_id: String,
    }

    impl SyncedTree {
        async fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "ioun-sync-{name}-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let pool = test_pool().await;
            let root_id = {
                let mut conn = pool.acquire().await.unwrap();
                // La raíz se guarda ya canonizada para que las rutas del árbol
                // coincidan con las que escribe el sync (`canonicalize`).
                let canonical = std::fs::canonicalize(&dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                add_folder_inner(
                    &mut conn,
                    "Vinculada".into(),
                    "synced".into(),
                    None,
                    Some(canonical),
                    false,
                )
                .await
                .unwrap()
                .id
            };
            Self { dir, pool, root_id }
        }

        /// Ruta canónica, que es la que guarda el sync (`canonicalize`).
        fn canonical(&self, relative: &str) -> String {
            std::fs::canonicalize(self.dir.join(relative))
                .unwrap()
                .to_string_lossy()
                .into_owned()
        }

        /// El contenido no es un PDF válido, así que se indexa con 0 páginas; el
        /// recuento lo corrige pdf.js al abrirlo (`update_document_pages`).
        fn write(&self, relative: &str) {
            self.write_bytes(relative, b"%PDF-1.4 test");
        }

        fn write_bytes(&self, relative: &str, contents: &[u8]) {
            let path = self.dir.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, contents).unwrap();
        }

        fn remove(&self, relative: &str) {
            let path = self.dir.join(relative);
            if path.is_dir() {
                std::fs::remove_dir_all(&path).unwrap();
            } else {
                std::fs::remove_file(&path).unwrap();
            }
        }

        async fn sync(&self) -> Result<bool, String> {
            sync_all_roots(&self.pool, None).await
        }

        /// Documentos en la biblioteca, por título.
        async fn library_docs(&self) -> Vec<(String, String)> {
            sqlx::query_as(
                "SELECT documents.title, folders.path FROM documents \
                 JOIN folder_docs ON folder_docs.doc_id = documents.id \
                 JOIN folders ON folders.id = folder_docs.folder_id \
                 WHERE documents.in_library = 1 ORDER BY folders.path, documents.title",
            )
            .fetch_all(&self.pool)
            .await
            .unwrap()
        }

        async fn folder_paths(&self) -> Vec<String> {
            let mut paths: Vec<String> =
                sqlx::query_scalar("SELECT path FROM folders WHERE path IS NOT NULL")
                    .fetch_all(&self.pool)
                    .await
                    .unwrap();
            paths.sort();
            paths
        }
    }

    impl Drop for SyncedTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[tokio::test]
    async fn imports_the_whole_tree_and_then_has_nothing_to_do() {
        let tree = SyncedTree::new("import").await;
        tree.write("a.pdf");
        tree.write("sub/b.pdf");
        tree.write("sub/nested/c.pdf");

        assert!(tree.sync().await.unwrap());

        assert_eq!(
            tree.folder_paths().await,
            vec![
                tree.canonical(""),
                tree.canonical("sub"),
                tree.canonical("sub/nested"),
            ]
        );
        let docs = tree.library_docs().await;
        assert_eq!(docs.len(), 3);
        assert_eq!(
            docs.iter()
                .map(|(title, _)| title.as_str())
                .collect::<Vec<_>>(),
            vec!["a.pdf", "b.pdf", "c.pdf"]
        );
        // Cada documento queda en su carpeta.
        assert_eq!(docs[0].1, tree.canonical(""));
        assert_eq!(docs[1].1, tree.canonical("sub"));
        assert_eq!(docs[2].1, tree.canonical("sub/nested"));

        // Las posiciones dentro de cada carpeta no se repiten.
        let positions: Vec<i64> =
            sqlx::query_scalar("SELECT position FROM folder_docs ORDER BY folder_id, position")
                .fetch_all(&tree.pool)
                .await
                .unwrap();
        assert_eq!(positions, vec![0, 0, 0]);

        // Segundo pase: nada cambió, así que no hay escrituras.
        assert!(!tree.sync().await.unwrap());
        assert_eq!(tree.library_docs().await.len(), 3);
        let state_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM synced_file_state")
            .fetch_one(&tree.pool)
            .await
            .unwrap();
        assert_eq!(state_rows, 3);
    }

    #[tokio::test]
    async fn scoped_sync_only_rescans_roots_that_contain_the_changed_path() {
        let tree = SyncedTree::new("scoped").await;
        tree.write("a.pdf");
        assert!(tree.sync().await.unwrap());
        assert_eq!(tree.library_docs().await.len(), 1);

        // Un cambio nuevo dentro de la raíz…
        tree.write("b.pdf");

        // …no se indexa si el pase acotado recibe solo rutas ajenas a la raíz.
        let unrelated =
            std::env::temp_dir().join(format!("ioun-unrelated-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&unrelated).unwrap();
        assert!(
            !sync_roots_containing(&tree.pool, None, std::slice::from_ref(&unrelated))
                .await
                .unwrap()
        );
        assert_eq!(tree.library_docs().await.len(), 1);

        // Con la ruta cambiada dentro de la raíz, el pase acotado sí la recoge.
        assert!(
            sync_roots_containing(
                &tree.pool,
                None,
                &[std::path::PathBuf::from(tree.canonical("b.pdf"))],
            )
            .await
            .unwrap()
        );
        assert_eq!(tree.library_docs().await.len(), 2);

        std::fs::remove_dir_all(&unrelated).ok();
    }

    /// Eliminar una carpeta sincronizada deja sus rutas en `ignored_synced_paths`
    /// (para que el sync no las recree). Restaurarla desde la papelera tiene que
    /// deshacer ese "ignorar": si no, el siguiente sync filtra el subárbol entero
    /// y borra de la biblioteca lo que se acaba de restaurar.
    /// Un documento que viene de una carpeta **no** estaba en la lista de la raíz,
    /// así que el índice que manda la interfaz ya es el destino: restarle uno lo
    /// dejaba una posición antes de donde el usuario lo soltó. El ajuste `-1` solo
    /// vale cuando el documento ya estaba en esa misma lista y se mueve hacia
    /// abajo (porque al quitarlo, los siguientes se desplazan).
    #[tokio::test]
    async fn moving_a_document_from_a_folder_to_the_root_respects_the_drop_index() {
        let pool = test_pool().await;
        for (id, position) in [("doc-a", 0), ("doc-b", 1), ("doc-c", 2)] {
            sqlx::query(
                "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                        last_page, last_zoom, last_opened, in_recent, in_library, \
                                        created_at, view_json, position) \
                 VALUES (?, ?, ?, 1, 1, NULL, NULL, 1, 100.0, NULL, 0, 1, 0, '{}', ?)",
            )
            .bind(id)
            .bind(format!("/{id}.pdf"))
            .bind(id)
            .bind(position)
            .execute(&pool)
            .await
            .unwrap();
        }
        let folder_id = {
            let mut conn = pool.acquire().await.unwrap();
            add_folder_inner(
                &mut conn,
                "Carpeta".into(),
                "temp".into(),
                None,
                None,
                false,
            )
            .await
            .unwrap()
            .id
        };
        // `doc-d` vive en la carpeta; su `position` de la raíz es un resto viejo.
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, last_opened, in_recent, in_library, \
                                    created_at, view_json, position) \
             VALUES ('doc-d', '/doc-d.pdf', 'doc-d', 1, 1, NULL, NULL, 1, 100.0, NULL, 0, 1, 0, '{}', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO folder_docs (folder_id, doc_id, position) VALUES (?, 'doc-d', 0)")
            .bind(&folder_id)
            .execute(&pool)
            .await
            .unwrap();

        // La interfaz manda el índice entre `doc-a` y `doc-b`: el 1.
        move_doc_inner(&pool, "doc-d".into(), None, 1)
            .await
            .unwrap();

        let root: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM documents WHERE in_library = 1 \
             AND id NOT IN (SELECT doc_id FROM folder_docs) ORDER BY position",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            root,
            vec!["doc-a", "doc-d", "doc-b", "doc-c"],
            "el documento aterrizó en un índice distinto del que se soltó"
        );
    }

    #[tokio::test]
    async fn restoring_a_deleted_synced_folder_survives_the_next_sync() {
        let tree = SyncedTree::new("restore-folder").await;
        tree.write("sub/b.pdf");
        assert!(tree.sync().await.unwrap());
        assert_eq!(tree.library_docs().await.len(), 1);

        delete_folder_inner(&tree.pool, tree.root_id.clone(), true)
            .await
            .unwrap();
        assert!(tree.library_docs().await.is_empty());

        let trash_id: String = sqlx::query_scalar("SELECT id FROM trash LIMIT 1")
            .fetch_one(&tree.pool)
            .await
            .unwrap();
        crate::commands::trash::restore_from_trash_inner(&tree.pool, &trash_id)
            .await
            .unwrap();

        // El siguiente sync no debe llevarse por delante lo restaurado.
        tree.sync().await.unwrap();

        assert_eq!(
            tree.library_docs().await.len(),
            1,
            "el sync borró el documento restaurado"
        );
        assert!(
            tree.folder_paths().await.contains(&tree.canonical("sub")),
            "el sync borró la subcarpeta restaurada"
        );
    }

    #[tokio::test]
    async fn restoring_a_removed_synced_document_clears_its_ignore_marker() {
        let tree = SyncedTree::new("restore-document").await;
        tree.write("a.pdf");
        assert!(tree.sync().await.unwrap());
        let doc_id: String = sqlx::query_scalar("SELECT id FROM documents WHERE path = ?")
            .bind(tree.canonical("a.pdf"))
            .fetch_one(&tree.pool)
            .await
            .unwrap();

        remove_from_library_inner(&tree.pool, doc_id.clone())
            .await
            .unwrap();
        let trash_id: String = sqlx::query_scalar("SELECT id FROM trash LIMIT 1")
            .fetch_one(&tree.pool)
            .await
            .unwrap();
        crate::commands::trash::restore_from_trash_inner(&tree.pool, &trash_id)
            .await
            .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM ignored_synced_paths WHERE path = ?",
            )
            .bind(tree.canonical("a.pdf"))
            .fetch_one(&tree.pool)
            .await
            .unwrap(),
            0
        );
        let _ = tree.sync().await.unwrap();
        let (in_library, links): (i64, i64) = sqlx::query_as(
            "SELECT documents.in_library, COUNT(folder_docs.doc_id) \
             FROM documents LEFT JOIN folder_docs ON folder_docs.doc_id = documents.id \
             WHERE documents.id = ? GROUP BY documents.id",
        )
        .bind(doc_id)
        .fetch_one(&tree.pool)
        .await
        .unwrap();
        assert_eq!(in_library, 1);
        assert_eq!(links, 1);
    }

    #[tokio::test]
    async fn a_paused_sync_does_not_scan_until_resumed() {
        struct PauseGuard;

        impl Drop for PauseGuard {
            fn drop(&mut self) {
                SYNC_PAUSED.store(false, std::sync::atomic::Ordering::SeqCst);
                SYNC_CANCEL.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let tree = SyncedTree::new("paused").await;
        tree.write("a.pdf");

        SYNC_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
        let _guard = PauseGuard;
        assert!(
            !sync_synced_folders_with_pool(&tree.pool, None, None)
                .await
                .unwrap()
        );
        assert!(tree.library_docs().await.is_empty());

        SYNC_PAUSED.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(
            sync_synced_folders_with_pool(&tree.pool, None, None)
                .await
                .unwrap()
        );
        assert_eq!(tree.library_docs().await.len(), 1);
    }

    #[tokio::test]
    async fn a_modified_file_is_refreshed_instead_of_duplicated() {
        let tree = SyncedTree::new("modified").await;
        tree.write("a.pdf");
        tree.sync().await.unwrap();
        let doc_id: String = sqlx::query_scalar("SELECT id FROM documents")
            .fetch_one(&tree.pool)
            .await
            .unwrap();

        let grown = b"%PDF-1.4 test con mas bytes que antes";
        tree.write_bytes("a.pdf", grown);
        assert!(tree.sync().await.unwrap());

        assert_eq!(tree.library_docs().await.len(), 1);
        let stored: (String, i64) = sqlx::query_as(
            "SELECT documents.id, synced_file_state.size_bytes FROM documents \
             JOIN synced_file_state ON synced_file_state.path = documents.path",
        )
        .fetch_one(&tree.pool)
        .await
        .unwrap();
        assert_eq!(stored.0, doc_id);
        assert_eq!(stored.1, grown.len() as i64);
    }

    #[tokio::test]
    async fn updating_a_file_inside_a_folder_keeps_its_position() {
        let tree = SyncedTree::new("keep-position").await;
        tree.write("a.pdf");
        tree.write("b.pdf");
        tree.write("c.pdf");
        tree.sync().await.unwrap();

        let positions_before: Vec<(String, i64)> = sqlx::query_as(
            "SELECT d.title, fd.position FROM folder_docs fd \
             JOIN documents d ON d.id = fd.doc_id ORDER BY fd.position",
        )
        .fetch_all(&tree.pool)
        .await
        .unwrap();

        tree.write_bytes("b.pdf", b"%PDF-1.4 actualizado");
        tree.sync().await.unwrap();

        let positions_after: Vec<(String, i64)> = sqlx::query_as(
            "SELECT d.title, fd.position FROM folder_docs fd \
             JOIN documents d ON d.id = fd.doc_id ORDER BY fd.position",
        )
        .fetch_all(&tree.pool)
        .await
        .unwrap();
        assert_eq!(
            positions_before, positions_after,
            "cambiar el contenido no debe mover el documento al final"
        );
    }

    #[tokio::test]
    async fn deleting_a_file_detaches_it_without_touching_the_rest() {
        let tree = SyncedTree::new("delete-file").await;
        tree.write("a.pdf");
        tree.write("sub/b.pdf");
        tree.sync().await.unwrap();

        tree.remove("a.pdf");
        assert!(tree.sync().await.unwrap());

        assert_eq!(tree.library_docs().await.len(), 1);
        // El documento no se borra: deja de estar en la biblioteca.
        let orphaned: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM documents WHERE in_library = 0")
                .fetch_one(&tree.pool)
                .await
                .unwrap();
        assert_eq!(orphaned, 1);
        let links: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folder_docs")
            .fetch_one(&tree.pool)
            .await
            .unwrap();
        assert_eq!(links, 1);
    }

    #[tokio::test]
    async fn deleting_a_folder_removes_it_from_the_tree() {
        let tree = SyncedTree::new("delete-folder").await;
        tree.write("gone/x.pdf");
        tree.write("kept/y.pdf");
        tree.sync().await.unwrap();
        assert_eq!(tree.folder_paths().await.len(), 3);

        tree.remove("gone");
        assert!(tree.sync().await.unwrap());

        assert_eq!(
            tree.folder_paths().await,
            vec![tree.canonical(""), tree.canonical("kept")]
        );
        assert_eq!(tree.library_docs().await.len(), 1);
    }

    #[tokio::test]
    async fn a_root_nested_in_another_root_is_reused_instead_of_duplicated() {
        let tree = SyncedTree::new("nested-roots").await;
        tree.write("inner/a.pdf");
        // La carpeta interna se vincula también como raíz propia: la ruta ya
        // existe como carpeta `synced`, así que el sync externo debe reutilizar
        // esa fila en vez de chocar con el índice único de `folders(path)`.
        let _inner_root_id = {
            let mut conn = tree.pool.acquire().await.unwrap();
            add_folder_inner(
                &mut conn,
                "Interna".into(),
                "synced".into(),
                None,
                Some(tree.canonical("inner")),
                false,
            )
            .await
            .unwrap()
            .id
        };

        assert!(tree.sync().await.unwrap());

        let duplicates: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders WHERE path = ?")
            .bind(tree.canonical("inner"))
            .fetch_one(&tree.pool)
            .await
            .unwrap();
        assert_eq!(duplicates, 1);
        let adopted_children: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM folders WHERE parent_id = ?")
                .bind(&tree.root_id)
                .fetch_one(&tree.pool)
                .await
                .unwrap();
        assert_eq!(adopted_children, 1, "la raíz externa adopta la interna");
        assert_eq!(tree.library_docs().await.len(), 1);
    }

    #[tokio::test]
    async fn a_root_that_cannot_be_written_rolls_back_whole() {
        let tree = SyncedTree::new("atomic").await;
        tree.write("sub/deep/x.pdf");
        // Una carpeta con la misma ruta pero de otro tipo: el INSERT del
        // subárbol falla y, al ser una única transacción, no debe quedar ni la
        // carpeta intermedia ni el documento.
        sqlx::query(
            "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) \
             VALUES ('temp-shadow', NULL, 'deep', 'temp', ?, 1, 0, 0)",
        )
        .bind(tree.canonical("sub/deep"))
        .execute(&tree.pool)
        .await
        .unwrap();

        assert!(tree.sync().await.is_err());

        let created: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders WHERE path = ?")
            .bind(tree.canonical("sub"))
            .fetch_one(&tree.pool)
            .await
            .unwrap();
        assert_eq!(
            created, 0,
            "la transacción de la raíz debería deshacerse entera"
        );
        let docs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM documents")
            .fetch_one(&tree.pool)
            .await
            .unwrap();
        assert_eq!(docs, 0);
    }

    #[tokio::test]
    async fn a_vanished_root_is_unlinked() {
        let tree = SyncedTree::new("vanished").await;
        tree.write("a.pdf");
        tree.sync().await.unwrap();

        std::fs::remove_dir_all(&tree.dir).unwrap();
        assert!(tree.sync().await.unwrap());

        let folders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
            .fetch_one(&tree.pool)
            .await
            .unwrap();
        assert_eq!(folders, 0);
        let in_library: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM documents WHERE in_library = 1")
                .fetch_one(&tree.pool)
                .await
                .unwrap();
        assert_eq!(in_library, 0);
    }
}
