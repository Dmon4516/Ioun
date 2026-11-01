mod commands;
mod db;
mod fs_watcher;
mod ids;
mod pdf_protocol;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};

struct ExternalFiles {
    buffer: Mutex<Vec<String>>,
    ready: AtomicBool,
}

impl ExternalFiles {
    fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::new()),
            ready: AtomicBool::new(false),
        }
    }

    fn on_open(&self, app: &tauri::AppHandle, path: String) {
        let send_now = {
            let mut buffer = self.buffer.lock().unwrap();
            if self.ready.load(Ordering::SeqCst) {
                true
            } else {
                buffer.push(path.clone());
                false
            }
        };
        if send_now {
            let _ = app.emit("open-file-external", path);
        }
    }

    fn drain_and_ready(&self) -> Vec<String> {
        let mut buffer = self.buffer.lock().unwrap();
        self.ready.store(true, Ordering::SeqCst);
        std::mem::take(&mut *buffer)
    }
}

fn is_supported_document(path: &str) -> bool {
    matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("pdf"),
    )
}

#[tauri::command]
async fn drain_pending_external_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    Ok(app.state::<ExternalFiles>().drain_and_ready())
}

pub fn run() {
    tauri::Builder::default()
        .manage(ExternalFiles::new())
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
            for arg in argv.iter().skip(1) {
                if arg.starts_with('-') {
                    continue;
                }
                if is_supported_document(arg) {
                    app.state::<ExternalFiles>().on_open(app, arg.clone());
                }
            }
        }))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .register_asynchronous_uri_scheme_protocol("lyra-pdf", |_app, request, responder| {
            tauri::async_runtime::spawn(async move {
                responder.respond(pdf_protocol::handle(request).await);
            });
        })
        .setup(|app| {
            let args_handle = app.handle();
            for arg in std::env::args().skip(1) {
                if arg.starts_with('-') {
                    continue;
                }
                if is_supported_document(&arg) {
                    app.state::<ExternalFiles>().on_open(args_handle, arg);
                }
            }

            let handle = app.handle().clone();
            // Inicialización: base de datos y watcher. El watcher reacciona a
            // cambios en disco; el poll de 30 s es un respaldo por si el
            // watcher pierde eventos y además sincroniza carpetas vinculadas
            // periódicamente.
            tauri::async_runtime::block_on(async move {
                match db::init(&handle).await {
                    Ok(()) => {
                        if let Err(error) = fs_watcher::start(handle.clone()).await {
                            eprintln!("[lyra] No se pudo iniciar la vigilancia de archivos: {error}");
                            let _ = handle.emit(
                                "watcher-init-failed",
                                format!("La vigilancia de archivos no pudo iniciarse: {error}"),
                            );
                        }
                        let sync_handle = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let mut interval =
                                tokio::time::interval(std::time::Duration::from_secs(30));
                            interval.tick().await;
                            loop {
                                interval.tick().await;
                                if let Err(error) = fs_watcher::refresh().await {
                                    eprintln!(
                                        "[lyra] No se pudieron actualizar las rutas vigiladas: {error}"
                                    );
                                }
                                match commands::library::sync_synced_folders_throttled(&sync_handle).await
                                {
                                    Ok(true) => {
                                        if let Err(error) = sync_handle.emit("library-changed", ()) {
                                            eprintln!(
                                                "[lyra] No se notificó la biblioteca sincronizada: {error}"
                                            );
                                        }
                                    }
                                    Ok(false) => {}
                                    Err(error) => {
                                        eprintln!("[lyra] Error al sincronizar carpetas: {error}")
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("[lyra] db init error: {e}");
                        // Notificar al frontend para que muestre un estado degradado.
                        // No arrancamos el watcher ni el poll: sin BD todo fallaría.
                        let _ = handle.emit("db-init-failed", e.to_string());
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::library::list_library,
            commands::library::list_library_root,
            commands::library::list_folder_children,
            commands::library::add_file,
            commands::library::add_folder,
            commands::library::restore_document_to_library,
            commands::library::sync_synced_folders,
            commands::library::cancel_sync,
            commands::library::is_sync_paused,
            commands::library::rename_folder,
            commands::library::set_folder_expanded,
            commands::library::delete_folder,
            commands::library::move_doc,
            commands::library::move_folder,
            commands::library::remove_doc,
            commands::library::remove_from_library,
            commands::library::export_library,
            commands::library::import_library,
            commands::library::export_library_to_file,
            commands::library::import_library_from_file,
            commands::documents::list_documents,
            commands::documents::set_doc_view,
            commands::documents::save_document_progress,
            commands::documents::update_document_pages,
            commands::documents::open_pdf_dialog,
            commands::documents::open_folder_dialog,
            commands::documents::pdf_file_length,
            commands::documents::read_pdf_range,
            commands::library::assign_category,
            commands::library::clear_recent_docs,
            commands::library::remove_recent_doc,
            commands::annotations::list_bookmarks,
            commands::annotations::add_bookmark,
            commands::annotations::delete_bookmark,
            commands::annotations::list_comments,
            commands::annotations::add_comment,
            commands::annotations::delete_comment,
            commands::categories::list_categories,
            commands::categories::upsert_category,
            commands::categories::delete_category,
            commands::categories::set_document_category,
            commands::categories::clear_document_categories,
            commands::trash::list_trash,
            commands::trash::restore_from_trash,
            commands::trash::empty_trash,
            #[cfg(debug_assertions)]
            commands::diagnostics::get_process_rss,
            drain_pending_external_files,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                if let tauri::RunEvent::Opened { urls } = event {
                    for url in urls {
                        if let Ok(path) = url.to_file_path() {
                            let path = path.to_string_lossy().into_owned();
                            if is_supported_document(&path) {
                                app_handle
                                    .state::<ExternalFiles>()
                                    .on_open(app_handle, path);
                            }
                        }
                    }
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            {
                let _ = (app_handle, event);
            }
        });
}

#[cfg(test)]
mod external_files_tests {
    use super::is_supported_document;

    #[test]
    fn accepts_pdf_case_insensitively() {
        assert!(is_supported_document("/libros/manual.pdf"));
        assert!(is_supported_document("/libros/MANUAL.PDF"));
        assert!(is_supported_document("/libros/Manual.Pdf"));
    }

    #[test]
    fn rejects_other_extensions_and_bare_paths() {
        assert!(!is_supported_document("/libros/guia.md"));
        assert!(!is_supported_document("/libros/notas.txt"));
        assert!(!is_supported_document("/libros/README"));
        assert!(!is_supported_document(""));
    }
}
