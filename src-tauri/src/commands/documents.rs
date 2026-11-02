use crate::db;
use crate::db::models::{DOCUMENT_COLUMNS, DocumentRecord};
use serde::{Deserialize, Serialize};
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocumentView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pages_per_row: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    continuous_scroll: Option<bool>,
}

#[tauri::command]
pub async fn list_documents() -> Result<Vec<DocumentRecord>, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents ORDER BY created_at DESC");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_doc_view(doc_id: String, view: serde_json::Value) -> Result<(), String> {
    let view: DocumentView = serde_json::from_value(view).map_err(|e| e.to_string())?;
    if view
        .pages_per_row
        .is_some_and(|pages| !(1..=4).contains(&pages))
    {
        return Err("pagesPerRow debe estar entre 1 y 4".into());
    }

    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let view_json = serde_json::to_string(&view).map_err(|e| e.to_string())?;
    let result = sqlx::query("UPDATE documents SET view_json = ? WHERE id = ?")
        .bind(view_json)
        .bind(doc_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("No se encontró el documento".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn save_document_progress(doc_id: String, page: i64, zoom: f64) -> Result<(), String> {
    if page < 1 || !zoom.is_finite() || !(40.0..=999.0).contains(&zoom) {
        return Err("La página o el zoom del documento no son válidos".into());
    }

    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let last_opened = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64;
    let result = sqlx::query(
        "UPDATE documents \
         SET last_page = ?, last_zoom = ?, last_opened = ?, in_recent = 1 \
         WHERE id = ? AND (pages = 0 OR pages >= ?)",
    )
    .bind(page)
    .bind(zoom)
    .bind(last_opened)
    .bind(&doc_id)
    .bind(page)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        // Distinguir documento inexistente de página fuera de rango.
        let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM documents WHERE id = ?")
            .bind(&doc_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
        if exists.is_none() {
            return Err("No se encontró el documento".into());
        }
        return Err("La página está fuera del rango del documento".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn update_document_pages(doc_id: String, pages: i64) -> Result<(), String> {
    if pages < 0 {
        return Err("El número de páginas no puede ser negativo".into());
    }
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    update_document_pages_inner(pool, doc_id, pages).await
}

pub(crate) async fn update_document_pages_inner(
    pool: &sqlx::SqlitePool,
    doc_id: String,
    pages: i64,
) -> Result<(), String> {
    // Con `pages = 0` —el valor que usa la biblioteca para «aún no se sabe», que
    // pdf.js corrige al abrir— el recorte de `last_page` lo dejaba en 1: un PDF
    // que falla al analizar borraba la posición de lectura guardada.
    let result = sqlx::query(
        "UPDATE documents SET pages = ?, \
         last_page = CASE WHEN ? > 0 THEN MIN(MAX(last_page, 1), ?) ELSE last_page END \
         WHERE id = ?",
    )
    .bind(pages)
    .bind(pages)
    .bind(pages)
    .bind(&doc_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("No se encontró el documento".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn open_pdf_dialog(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let result = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("PDF", &["pdf"])
            .blocking_pick_files()
    })
    .await
    .map_err(|error| format!("El diálogo no se pudo abrir: {error}"))?;
    result
        .unwrap_or_default()
        .into_iter()
        .map(|path| {
            path.into_path()
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| error.to_string())
        })
        .collect()
}

#[tauri::command]
pub async fn open_folder_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let result = tokio::task::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
        .await
        .map_err(|error| format!("El diálogo no se pudo abrir: {error}"))?;
    Ok(result
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

/// Tamaño máximo que aceptamos parsear con `lopdf` para contar páginas.
///
/// `lopdf::Document::load` carga el archivo entero en memoria. Este límite no
/// evita el uso de RAM durante el conteo; solo acota cuánta memoria puede
/// consumir. Un PDF mayor se añade con `pages = 0` y pdf.js corrige el total
/// al abrirlo. Ver TECH_DEBT.md.
const MAX_PDF_BYTES_FOR_COUNT: u64 = 512 * 1024 * 1024;

pub fn count_pages(path: &str) -> Result<i64, String> {
    let metadata =
        std::fs::metadata(path).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    if metadata.len() > MAX_PDF_BYTES_FOR_COUNT {
        return Err(format!(
            "El PDF supera el límite de {} MB para indexado. \
             Puedes abrirlo igualmente; pdf.js corregirá el total al cargarlo.",
            MAX_PDF_BYTES_FOR_COUNT / (1024 * 1024),
        ));
    }
    let doc = lopdf::Document::load(path).map_err(|e| e.to_string())?;
    Ok(doc.get_pages().len() as i64)
}

// ─────────────────────────────────────────────────────────────────────────────
// Lectura por rangos para PDFs grandes
// ─────────────────────────────────────────────────────────────────────────────

/// Tamaño máximo de un rango servido por IPC. pdf.js pide bloques de 64 KiB, pero
/// un rango mayor (documento linealizado, xref grande) también es legítimo; el
/// tope evita que una llamada pida el archivo entero de una vez.
const MAX_PDF_RANGE_BYTES: u64 = 16 * 1024 * 1024;

fn validate_pdf_range(begin: u64, end: u64) -> Result<(), String> {
    if end <= begin {
        return Err("El rango pedido está vacío".into());
    }
    if end - begin > MAX_PDF_RANGE_BYTES {
        return Err(format!(
            "El rango pedido supera el máximo de {MAX_PDF_RANGE_BYTES} bytes"
        ));
    }
    Ok(())
}

/// Comprueba que el archivo está activo en la biblioteca: conservar una fila
/// histórica (por ejemplo, para recientes o papelera) no debe conservar acceso
/// de lectura al archivo.
async fn is_registered_document(pool: &sqlx::SqlitePool, path: &str) -> Result<bool, String> {
    #[cfg(windows)]
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM documents WHERE in_library = 1 \
         AND REPLACE(path, char(92), '/') = ? LIMIT 1",
    )
    .bind(path.replace('\\', "/"))
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM documents WHERE in_library = 1 AND path = ? LIMIT 1")
            .bind(path)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(found.is_some())
}

/// Lee `[begin, end)` del archivo. `end` es exclusivo, como lo pide pdf.js
/// (`Range: bytes=begin-(end-1)`). Se recorta al tamaño real y un `begin` más
/// allá del final devuelve un rango vacío: pdf.js lo tolera y devolver un error
/// rompería la carga de un documento que encogió desde el último sync.
async fn read_range_from_file(
    path: &std::path::Path,
    begin: u64,
    end: u64,
) -> Result<Vec<u8>, String> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("No se pudo abrir el archivo: {e}"))?;
    let length = file.metadata().await.map_err(|e| e.to_string())?.len();
    if begin >= length {
        return Ok(Vec::new());
    }
    let end = end.min(length);
    let mut buffer = vec![0u8; (end - begin) as usize];
    file.seek(std::io::SeekFrom::Start(begin))
        .await
        .map_err(|e| e.to_string())?;
    file.read_exact(&mut buffer)
        .await
        .map_err(|e| format!("No se pudo leer el rango: {e}"))?;
    Ok(buffer)
}

/// Longitud real del archivo en disco. pdf.js necesita el total antes de pedir
/// rangos, y tomarlo del disco evita depender del `size_bytes` guardado (que
/// puede estar desactualizado si el archivo cambió de tamaño).
#[tauri::command]
pub async fn pdf_file_length(path: String) -> Result<u64, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    if !is_registered_document(pool, &path).await? {
        return Err("El documento no está registrado en la biblioteca".into());
    }
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    if !metadata.is_file() {
        return Err("La ruta no es un archivo".into());
    }
    Ok(metadata.len())
}

/// Devuelve los bytes `[begin, end)` del documento registrado en la biblioteca.
#[tauri::command]
pub async fn read_pdf_range(
    path: String,
    begin: u64,
    end: u64,
) -> Result<tauri::ipc::Response, String> {
    validate_pdf_range(begin, end)?;
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    if !is_registered_document(pool, &path).await? {
        return Err("El documento no está registrado en la biblioteca".into());
    }
    let bytes = read_range_from_file(std::path::Path::new(&path), begin, end).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[cfg(test)]
mod range_tests {
    use super::*;
    use crate::db::test_pool;

    fn temp_file(name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ioun-range-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn rejects_empty_and_oversized_ranges() {
        assert!(validate_pdf_range(10, 10).is_err());
        assert!(validate_pdf_range(20, 10).is_err());
        assert!(validate_pdf_range(0, MAX_PDF_RANGE_BYTES + 1).is_err());
        assert!(validate_pdf_range(0, MAX_PDF_RANGE_BYTES).is_ok());
    }

    #[tokio::test]
    async fn reads_exactly_the_requested_range() {
        let content: Vec<u8> = (0..=255u8).collect();
        let path = temp_file("exact", &content);

        let chunk = read_range_from_file(&path, 10, 20).await.unwrap();

        assert_eq!(chunk, &content[10..20]);
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn clamps_the_end_to_the_file_length() {
        let path = temp_file("clamp", b"0123456789");

        let chunk = read_range_from_file(&path, 5, 10_000).await.unwrap();

        assert_eq!(chunk, b"56789");
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn a_range_beyond_the_end_is_empty() {
        let path = temp_file("past-end", b"0123456789");

        let chunk = read_range_from_file(&path, 50, 60).await.unwrap();

        assert!(chunk.is_empty());
        std::fs::remove_file(&path).ok();
    }

    /// El caso que importa de verdad: los rangos que pide pdf.js cubren el
    /// archivo entero sin huecos ni solapes, y el último se recorta al final.
    #[tokio::test]
    async fn consecutive_chunks_rebuild_the_whole_file() {
        let content: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let path = temp_file("rebuild", &content);

        let mut rebuilt = vec![0u8; content.len()];
        // Desordenados a propósito, como los pide pdf.js según va necesitando.
        for (begin, end) in [(4000u64, 6000u64), (0, 2000), (2000, 4000)] {
            let chunk = read_range_from_file(&path, begin, end).await.unwrap();
            rebuilt[begin as usize..begin as usize + chunk.len()].copy_from_slice(&chunk);
        }

        assert_eq!(rebuilt, content);
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn only_registered_documents_are_served() {
        let pool = test_pool().await;

        assert!(
            !is_registered_document(&pool, "/libros/manual.pdf")
                .await
                .unwrap()
        );

        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, last_opened, in_recent, in_library, \
                                    created_at, view_json, position) \
             VALUES ('doc-1', '/libros/manual.pdf', 'manual.pdf', 1, 1, NULL, NULL, \
                     1, 100.0, NULL, 0, 1, 0, '{}', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        assert!(
            is_registered_document(&pool, "/libros/manual.pdf")
                .await
                .unwrap()
        );
        sqlx::query("UPDATE documents SET in_library = 0 WHERE id = 'doc-1'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            !is_registered_document(&pool, "/libros/manual.pdf")
                .await
                .unwrap()
        );
        assert!(
            !is_registered_document(&pool, "/otros/ajeno.pdf")
                .await
                .unwrap()
        );
    }
    #[tokio::test]
    async fn zero_pages_does_not_reset_the_reading_position() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, last_opened, in_recent, in_library, \
                                    created_at, view_json, position) \
             VALUES ('doc-1', '/a.pdf', 'a.pdf', 0, 1, NULL, NULL, 42, 100.0, NULL, 0, 1, 0, '{}', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        update_document_pages_inner(&pool, "doc-1".into(), 0)
            .await
            .unwrap();

        let last_page: i64 =
            sqlx::query_scalar("SELECT last_page FROM documents WHERE id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            last_page, 42,
            "el recorte con pages = 0 puso la posición a 1"
        );

        // Con un total real sí se recorta a lo que existe.
        update_document_pages_inner(&pool, "doc-1".into(), 10)
            .await
            .unwrap();
        let last_page: i64 =
            sqlx::query_scalar("SELECT last_page FROM documents WHERE id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(last_page, 10);
    }
}
