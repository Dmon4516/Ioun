use crate::db;
use crate::ids::new_id;
use serde::{Deserialize, Serialize};

pub mod error_codes {
    pub const TRASH_PATH_COLLISION: &str = "TRASH_PATH_COLLISION";
    pub const TRASH_ID_COLLISION: &str = "TRASH_ID_COLLISION";
    pub const TRASH_MISSING_PARENT: &str = "TRASH_MISSING_PARENT";
    pub const TRASH_MISSING_RELATION: &str = "TRASH_MISSING_RELATION";
    pub const TRASH_NOT_FOUND: &str = "TRASH_NOT_FOUND";
    pub const TRASH_UNKNOWN_KIND: &str = "TRASH_UNKNOWN_KIND";
}

fn coded_error(code: &str, message: String) -> String {
    format!("{code}:{message}")
}

fn required_id(value: &serde_json::Value, field: &str) -> Result<String, String> {
    let id = value
        .get(field)
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if id.trim().is_empty() {
        return Err(coded_error(
            error_codes::TRASH_MISSING_RELATION,
            format!("El identificador de {field} no puede estar vacío"),
        ));
    }
    Ok(id.to_string())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub created_at: i64,
}

/// Inserta un snapshot en la papelera.
pub async fn push(
    executor: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    kind: &str,
    label: &str,
    payload: &serde_json::Value,
) -> Result<String, String> {
    let id = new_id("trash");
    let payload_str = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO trash (id, kind, label, payload, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(kind)
        .bind(label)
        .bind(&payload_str)
        .bind(now_ms())
        .execute(&mut **executor)
        .await
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn list_trash() -> Result<Vec<TrashItem>, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    sqlx::query_as("SELECT id, kind, label, created_at FROM trash ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_from_trash(id: String) -> Result<Vec<String>, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    restore_from_trash_inner(pool, &id).await
}

pub async fn restore_from_trash_inner(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<Vec<String>, String> {
    if id.trim().is_empty() {
        return Err(coded_error(
            error_codes::TRASH_NOT_FOUND,
            "El id de la papelera no puede estar vacío".into(),
        ));
    }
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;

    let row: Option<(String, String)> =
        sqlx::query_as("SELECT kind, payload FROM trash WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    let (kind, payload_str) = row.ok_or_else(|| {
        coded_error(
            error_codes::TRASH_NOT_FOUND,
            "No se encontró el elemento en la papelera".into(),
        )
    })?;
    let payload: serde_json::Value =
        serde_json::from_str(&payload_str).map_err(|e| e.to_string())?;

    let mut warnings = Vec::new();
    match kind.as_str() {
        "folder" => restore_folder(&mut transaction, &payload, &mut warnings).await?,
        "document" => restore_document(&mut transaction, &payload, &mut warnings).await?,
        "category" => restore_category(&mut transaction, &payload, &mut warnings).await?,
        "bookmark" => restore_bookmark(&mut transaction, &payload, &mut warnings).await?,
        "comment" => restore_comment(&mut transaction, &payload, &mut warnings).await?,
        _ => {
            return Err(coded_error(
                error_codes::TRASH_UNKNOWN_KIND,
                format!("Tipo de papelera desconocido: {kind}"),
            ));
        }
    }

    sqlx::query("DELETE FROM trash WHERE id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;

    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(warnings)
}

async fn reindex_folder_siblings(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    parent_id: &Option<String>,
) -> Result<(), String> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM folders \
         WHERE ((? IS NULL AND parent_id IS NULL) OR parent_id = ?) \
         ORDER BY position ASC, created_at ASC, id ASC",
    )
    .bind(parent_id)
    .bind(parent_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    for (position, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE folders SET position = ? WHERE id = ?")
            .bind(position as i64)
            .bind(id)
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Hace hueco en la posición `target` para el documento que se va a restaurar:
/// los hermanos que ocupan esa posición o una posterior ceden su sitio. Así el
/// documento recupera su índice original y no comparte `position` con otro (con
/// posiciones repetidas el orden visible pasaría a depender de `created_at`).
async fn make_room_for_doc(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    folder_id: &Option<String>,
    target: i64,
) -> Result<(), String> {
    match folder_id {
        Some(fid) => {
            sqlx::query(
                "UPDATE folder_docs SET position = position + 1 \
                 WHERE folder_id = ? AND position >= ?",
            )
            .bind(fid)
            .bind(target)
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
        }
        None => {
            // Solo los documentos de la raíz (los que no viven en ninguna
            // carpeta) comparten este índice; el del documento en sí queda fuera
            // porque al quitarse de la biblioteca ya está `in_library = 0`.
            sqlx::query(
                "UPDATE documents SET position = position + 1 \
                 WHERE in_library = 1 \
                 AND id NOT IN (SELECT doc_id FROM folder_docs) \
                 AND position >= ?",
            )
            .bind(target)
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// La vista por documento (`pagesPerRow`, `continuousScroll`) tal como viene en
/// el snapshot.
///
/// Los snapshots de carpeta guardan `view` como la cadena cruda de `view_json`,
/// pero los de documento serializan `DocumentRecord`, cuyo campo `view` es un
/// `serde_json::Value`: ahí llega como **objeto**. Leer solo la cadena hacía que
/// ese caso escribiera `{}` y el usuario perdiera su vista al restaurar.
fn view_json_from_snapshot(doc: &serde_json::Value) -> String {
    match doc.get("view") {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(value) if !value.is_null() => value.to_string(),
        _ => "{}".to_string(),
    }
}

fn view_state_json_from_snapshot(doc: &serde_json::Value) -> String {
    match doc.get("viewState") {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(value) if !value.is_null() => value.to_string(),
        _ => "{}".to_string(),
    }
}

fn category_ids_from_snapshot(doc: &serde_json::Value) -> Vec<String> {
    if let Some(ids) = doc.get("categoryIds").and_then(|value| value.as_array()) {
        return ids
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect();
    }
    doc.get("categoryId")
        .and_then(|value| value.as_str())
        .into_iter()
        .map(str::to_owned)
        .collect()
}

async fn restore_document_categories(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    doc_id: &str,
    category_ids: &[String],
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let document_exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM documents WHERE id = ?")
        .bind(&doc_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    if document_exists.is_none() {
        warnings.push(format!(
            "{}:document:{}",
            error_codes::TRASH_MISSING_RELATION,
            doc_id
        ));
        return Ok(());
    }
    for category_id in category_ids {
        let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM categories WHERE id = ?")
            .bind(category_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
        if exists.is_none() {
            warnings.push(format!(
                "{}:category:{}",
                error_codes::TRASH_MISSING_RELATION,
                category_id
            ));
            continue;
        }
        sqlx::query(
            "INSERT OR IGNORE INTO document_categories (doc_id, category_id) VALUES (?, ?)",
        )
        .bind(&doc_id)
        .bind(category_id)
        .execute(&mut **tx)
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
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn reindex_docs_siblings(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    folder_id: &Option<String>,
) -> Result<(), String> {
    match folder_id {
        Some(fid) => {
            let ids: Vec<String> = sqlx::query_scalar(
                "SELECT doc_id FROM folder_docs WHERE folder_id = ? \
                 ORDER BY position ASC, doc_id ASC",
            )
            .bind(fid)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
            for (position, id) in ids.iter().enumerate() {
                sqlx::query(
                    "UPDATE folder_docs SET position = ? WHERE folder_id = ? AND doc_id = ?",
                )
                .bind(position as i64)
                .bind(fid)
                .bind(id)
                .execute(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
            }
        }
        None => {
            let ids: Vec<String> = sqlx::query_scalar(
                "SELECT id FROM documents WHERE in_library = 1 \
                 AND id NOT IN (SELECT doc_id FROM folder_docs) \
                 ORDER BY position ASC, created_at ASC, id ASC",
            )
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
            for (position, id) in ids.iter().enumerate() {
                sqlx::query("UPDATE documents SET position = ? WHERE id = ?")
                    .bind(position as i64)
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

async fn restore_folder(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    payload: &serde_json::Value,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let folder = payload.get("folder").ok_or("payload sin folder")?;
    let parent_id = folder["parentId"].as_str().map(|s| s.to_string());

    // El snapshot incluye **todo el subárbol** en `folders`, ordenado por
    // profundidad. Los snapshots anteriores a ese campo solo traían la carpeta
    // raíz, así que un payload sin `folders` se sigue restaurando como antes.
    let subfolders: Vec<&serde_json::Value> =
        match payload.get("folders").and_then(|v| v.as_array()) {
            Some(list) if !list.is_empty() => list.iter().collect(),
            _ => vec![folder],
        };

    // Si el sync recreó alguna carpeta con el mismo path pero distinto id, no
    // duplicamos: devolvemos un error estructurado para que la UI lo explique.
    // Se revisa el subárbol entero porque un `INSERT OR IGNORE` que choca se
    // descarta en silencio y dejaría sin carpeta a los documentos de dentro.
    for entry in &subfolders {
        let entry_id = entry["id"].as_str().unwrap_or("");
        let existing_by_id: Option<Option<String>> =
            sqlx::query_scalar("SELECT path FROM folders WHERE id = ?")
                .bind(entry_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
        if let Some(existing_path) = existing_by_id {
            return Err(coded_error(
                error_codes::TRASH_ID_COLLISION,
                format!(
                    "La carpeta «{}» ya existe con el id restaurado{}.",
                    entry["name"].as_str().unwrap_or(""),
                    existing_path
                        .as_deref()
                        .map(|path| format!(" en «{path}»"))
                        .unwrap_or_default(),
                ),
            ));
        }
        let Some(path) = entry["path"].as_str() else {
            continue;
        };
        let existing: Option<String> = sqlx::query_scalar("SELECT id FROM folders WHERE path = ?")
            .bind(path)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
        if let Some(existing_id) = existing.filter(|id| *id != entry["id"].as_str().unwrap_or("")) {
            return Err(coded_error(
                error_codes::TRASH_PATH_COLLISION,
                format!(
                    "La carpeta «{}» ya existe con otro id ({existing_id}). \
                     Puedes renombrar o mover la carpeta existente antes de restaurar, \
                     o vaciar la papelera si no la necesitas.",
                    entry["name"].as_str().unwrap_or(""),
                ),
            ));
        }
    }

    // Mismo motivo con los documentos: si otro documento ocupa el mismo `path`,
    // el `INSERT OR IGNORE` lo descartaría y con él su enlace a la carpeta.
    if let Some(docs) = payload.get("documents").and_then(|v| v.as_array()) {
        for doc in docs {
            let Some(path) = doc["path"].as_str() else {
                continue;
            };
            let existing: Option<String> =
                sqlx::query_scalar("SELECT id FROM documents WHERE path = ?")
                    .bind(path)
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(|e| e.to_string())?;
            if let Some(existing_id) = existing.filter(|id| *id != doc["id"].as_str().unwrap_or(""))
            {
                return Err(coded_error(
                    error_codes::TRASH_PATH_COLLISION,
                    format!(
                        "El documento «{}» ya está en la biblioteca con otro id ({existing_id}). \
                         Elimínalo o renómbralo antes de restaurar.",
                        doc["title"].as_str().unwrap_or(""),
                    ),
                ));
            }
        }
    }

    // Padre antes que hijo: el snapshot viene ordenado por profundidad. Si el
    // padre ya no existe, restaurar en silencio dejaría la carpeta fuera (la FK
    // lo impediría y `INSERT OR IGNORE` no avisa), así que se explica el motivo.
    for entry in &subfolders {
        let parent = entry["parentId"].as_str();
        if let Some(parent) = parent {
            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM folders WHERE id = ?")
                .bind(parent)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
            if exists.is_none() {
                return Err(coded_error(
                    error_codes::TRASH_MISSING_PARENT,
                    format!(
                        "No se puede restaurar «{}»: su carpeta superior ya no existe. \
                         Restaura primero esa carpeta o vuelve a añadir el archivo original.",
                        entry["name"].as_str().unwrap_or(""),
                    ),
                ));
            }
        }

        sqlx::query(
            "INSERT OR IGNORE INTO folders \
             (id, parent_id, name, kind, path, expanded, position, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(entry["id"].as_str().unwrap_or(""))
        .bind(parent)
        .bind(entry["name"].as_str().unwrap_or(""))
        .bind(entry["kind"].as_str().unwrap_or("temp"))
        .bind(entry["path"].as_str())
        .bind(entry["expanded"].as_bool().unwrap_or(true))
        .bind(entry["position"].as_i64().unwrap_or(0))
        .bind(entry["createdAt"].as_i64().unwrap_or(0))
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    let documents: Vec<&serde_json::Value> = payload
        .get("documents")
        .and_then(|v| v.as_array())
        .map(|list| list.iter().collect())
        .unwrap_or_default();

    if let Some(docs) = payload.get("documents").and_then(|v| v.as_array()) {
        for d in docs {
            let doc_id = required_id(d, "id")?;
            sqlx::query(
                "INSERT OR IGNORE INTO documents \
                 (id, path, title, kind, pages, size_bytes, hash, category_id, last_page, \
                   last_zoom, last_opened, in_recent, in_library, created_at, view_json, view_state, position) \
                  VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&doc_id)
            .bind(d["path"].as_str().unwrap_or(""))
            .bind(d["title"].as_str().unwrap_or(""))
            .bind(d["kind"].as_str().unwrap_or("pdf"))
            .bind(d["pages"].as_i64().unwrap_or(0))
            .bind(d["sizeBytes"].as_i64())
            .bind(d["hash"].as_str())
            .bind(Option::<&str>::None)
            .bind(d["lastPage"].as_i64().unwrap_or(1))
            .bind(d["lastZoom"].as_f64().unwrap_or(100.0))
            .bind(d["lastOpened"].as_i64())
            .bind(d["inRecent"].as_bool().unwrap_or(false))
            .bind(d["inLibrary"].as_bool().unwrap_or(true))
            .bind(d["createdAt"].as_i64().unwrap_or(0))
            .bind(view_json_from_snapshot(d))
            .bind(view_state_json_from_snapshot(d))
            .bind(d["position"].as_i64().unwrap_or(0))
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
            restore_document_categories(tx, &doc_id, &category_ids_from_snapshot(d), warnings)
                .await?;
        }
    }

    if let Some(fds) = payload.get("folderDocs").and_then(|v| v.as_array()) {
        for fd in fds {
            let folder_id = required_id(fd, "folderId")?;
            let doc_id = required_id(fd, "docId")?;
            sqlx::query(
                "INSERT OR IGNORE INTO folder_docs (folder_id, doc_id, position) \
                 VALUES (?, ?, ?)",
            )
            .bind(folder_id)
            .bind(&doc_id)
            .bind(fd["position"].as_i64().unwrap_or(0))
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
        }
        let restored_folder_ids: std::collections::HashSet<String> = subfolders
            .iter()
            .filter_map(|folder| folder["id"].as_str().map(str::to_owned))
            .collect();
        for folder_id in restored_folder_ids {
            reindex_docs_siblings(tx, &Some(folder_id)).await?;
        }
    }

    reindex_folder_siblings(tx, &parent_id).await?;
    clear_ignored_entries(tx, &subfolders, &documents).await?;
    Ok(())
}

/// Deshace el «ignorar» que dejó `delete_folder`.
///
/// Al eliminar una carpeta de una raíz sincronizada se apuntan en
/// `ignored_synced_paths` tanto sus carpetas como las rutas de sus documentos,
/// para que el sync no las vuelva a crear. Si el usuario la restaura desde la
/// papelera, esas marcas tienen que desaparecer: si no, el siguiente sync filtra
/// el subárbol entero (todas sus rutas empiezan por la de la raíz ignorada) y
/// borra de la biblioteca justo lo que se acaba de restaurar.
///
/// Se borran las entradas **exactas** de lo restaurado, no todo lo que cuelga de
/// la carpeta: así un documento que el usuario había quitado a propósito de una
/// carpeta sincronizada conserva su propia entrada y el sync no lo resucita.
async fn clear_ignored_entries(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    folders: &[&serde_json::Value],
    documents: &[&serde_json::Value],
) -> Result<(), String> {
    for path in folders
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .chain(documents.iter().filter_map(|entry| entry["path"].as_str()))
    {
        sqlx::query("DELETE FROM ignored_synced_paths WHERE path = ?")
            .bind(path)
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn restore_document(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    payload: &serde_json::Value,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let doc = payload.get("document").ok_or("payload sin document")?;
    let doc_id = required_id(doc, "id")?;
    let folder_doc = payload.get("folderDoc").filter(|fd| !fd.is_null());
    let folder_id = folder_doc
        .and_then(|fd| fd["folderId"].as_str())
        .map(|s| s.to_string());

    // Si el usuario volvió a añadir el mismo archivo después de quitarlo, hay
    // otra fila con el mismo `path`: el `INSERT OR IGNORE` la descartaría en
    // silencio y acto seguido borraríamos la entrada de la papelera, así que el
    // usuario perdía la restauración sin ver ningún error.
    if let Some(path) = doc["path"].as_str() {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM documents WHERE path = ?")
                .bind(path)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
        if let Some(existing_id) = existing.filter(|id| *id != doc_id) {
            return Err(coded_error(
                error_codes::TRASH_PATH_COLLISION,
                format!(
                    "El documento «{}» ya está en la biblioteca con otro id ({existing_id}). \
                     Elimínalo o renómbralo antes de restaurar, o vacía la papelera si no lo necesitas.",
                    doc["title"].as_str().unwrap_or(""),
                ),
            ));
        }
    }

    // Hacer hueco en la posición del snapshot en vez de compactar los hermanos
    // antes de insertar: compactar dejaba al documento restaurado compartiendo
    // `position` con otro, así que el orden visible pasaba a depender de
    // `created_at`. Es el mismo patrón que usa `move_doc` (desplazar y luego
    // insertar); al final se recompacta.
    let reindex_folder_id = match &folder_id {
        Some(fid) => {
            let folder_exists: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM folders WHERE id = ?")
                    .bind(fid)
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(|e| e.to_string())?;
            if folder_exists.is_some() {
                let folder_position = folder_doc
                    .and_then(|fd| fd["position"].as_i64())
                    .unwrap_or(0);
                make_room_for_doc(tx, &folder_id, folder_position).await?;
                Some(fid.clone())
            } else {
                make_room_for_doc(tx, &None, doc["position"].as_i64().unwrap_or(0)).await?;
                None
            }
        }
        None => {
            make_room_for_doc(tx, &None, doc["position"].as_i64().unwrap_or(0)).await?;
            None
        }
    };

    sqlx::query(
        "INSERT OR IGNORE INTO documents \
         (id, path, title, kind, pages, size_bytes, hash, category_id, last_page, \
           last_zoom, last_opened, in_recent, in_library, created_at, view_json, view_state, position) \
          VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&doc_id)
    .bind(doc["path"].as_str().unwrap_or(""))
    .bind(doc["title"].as_str().unwrap_or(""))
    .bind(doc["kind"].as_str().unwrap_or("pdf"))
    .bind(doc["pages"].as_i64().unwrap_or(0))
    .bind(doc["sizeBytes"].as_i64())
    .bind(doc["hash"].as_str())
    .bind(Option::<&str>::None)
    .bind(doc["lastPage"].as_i64().unwrap_or(1))
    .bind(doc["lastZoom"].as_f64().unwrap_or(100.0))
    .bind(doc["lastOpened"].as_i64())
    .bind(doc["inRecent"].as_bool().unwrap_or(false))
    .bind(doc["inLibrary"].as_bool().unwrap_or(true))
    .bind(doc["createdAt"].as_i64().unwrap_or(0))
    .bind(view_json_from_snapshot(doc))
    .bind(view_state_json_from_snapshot(doc))
    .bind(doc["position"].as_i64().unwrap_or(0))
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(fd) = folder_doc {
        let folder_id = fd["folderId"].as_str().unwrap_or("");
        if reindex_folder_id.as_deref() != Some(folder_id) {
            warnings.push(format!(
                "{}:folder:{}",
                error_codes::TRASH_MISSING_RELATION,
                folder_id
            ));
        } else {
            sqlx::query(
                "INSERT OR IGNORE INTO folder_docs (folder_id, doc_id, position) \
             VALUES (?, ?, ?)",
            )
            .bind(folder_id)
            .bind(required_id(fd, "docId")?)
            .bind(fd["position"].as_i64().unwrap_or(0))
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    restore_document_categories(tx, &doc_id, &category_ids_from_snapshot(doc), warnings).await?;

    if let Some(path) = doc["path"].as_str() {
        sqlx::query("DELETE FROM ignored_synced_paths WHERE path = ?")
            .bind(path)
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Quitar un documento de la biblioteca solo marca `in_library = 0` y deja la
    // fila: en ese caso el `INSERT OR IGNORE` no reactivaba nada y el documento
    // seguía sin aparecer tras restaurarlo.
    sqlx::query("UPDATE documents SET in_library = 1 WHERE id = ?")
        .bind(&doc_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    if reindex_folder_id.is_none() {
        sqlx::query("UPDATE documents SET position = ? WHERE id = ?")
            .bind(doc["position"].as_i64().unwrap_or(0))
            .bind(&doc_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Recompactar los hermanos: el hueco abierto antes de insertar deja las
    // posiciones contiguas y en el orden original.
    reindex_docs_siblings(tx, &reindex_folder_id).await?;
    Ok(())
}

async fn restore_category(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    payload: &serde_json::Value,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let c = payload.get("category").ok_or("payload sin category")?;
    sqlx::query("INSERT OR IGNORE INTO categories (id, name, color, position) VALUES (?, ?, ?, ?)")
        .bind(c["id"].as_str().unwrap_or(""))
        .bind(c["name"].as_str().unwrap_or(""))
        .bind(c["color"].as_str().unwrap_or("#888888"))
        .bind(c["position"].as_i64().unwrap_or(0))
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;

    if let Some(doc_ids) = payload.get("documentIds").and_then(|v| v.as_array()) {
        for id in doc_ids {
            if let Some(id) = id.as_str() {
                restore_document_categories(
                    tx,
                    id,
                    &[c["id"].as_str().unwrap_or("").to_string()],
                    warnings,
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn restore_bookmark(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    payload: &serde_json::Value,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let b = payload.get("bookmark").ok_or("payload sin bookmark")?;
    let doc_id = required_id(b, "docId")?;
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM documents WHERE id = ?")
        .bind(&doc_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        warnings.push(format!(
            "{}:document:{}",
            error_codes::TRASH_MISSING_RELATION,
            doc_id
        ));
        return Ok(());
    }
    sqlx::query(
        "INSERT OR IGNORE INTO bookmarks (id, doc_id, page, name, created_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(b["id"].as_str().unwrap_or(""))
    .bind(&doc_id)
    .bind(b["page"].as_i64().unwrap_or(1))
    .bind(b["name"].as_str().unwrap_or(""))
    .bind(b["createdAt"].as_i64().unwrap_or(0))
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    // Restoring an annotation is an explicit request to make its document
    // usable again; this mirrors document restoration without touching files
    // or folder placement.
    sqlx::query("UPDATE documents SET in_library = 1 WHERE id = ?")
        .bind(&doc_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn restore_comment(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    payload: &serde_json::Value,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let c = payload.get("comment").ok_or("payload sin comment")?;
    let doc_id = required_id(c, "docId")?;
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM documents WHERE id = ?")
        .bind(&doc_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        warnings.push(format!(
            "{}:document:{}",
            error_codes::TRASH_MISSING_RELATION,
            doc_id
        ));
        return Ok(());
    }
    sqlx::query(
        "INSERT OR IGNORE INTO comments (id, doc_id, page, text, created_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(c["id"].as_str().unwrap_or(""))
    .bind(&doc_id)
    .bind(c["page"].as_i64())
    .bind(c["text"].as_str().unwrap_or(""))
    .bind(c["createdAt"].as_i64().unwrap_or(0))
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    // See `restore_bookmark`: an annotation restored from trash reactivates
    // its existing parent document, but never creates a missing document.
    sqlx::query("UPDATE documents SET in_library = 1 WHERE id = ?")
        .bind(doc_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Vacía la papelera de forma irreversible.
///
/// No pide confirmación en el backend: la UI es la única entrada y ya
/// confirma antes de llamar. Si se expone a un script, añadir confirmación.
#[tauri::command]
pub async fn empty_trash() -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM trash")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod restore_tests {
    use super::*;
    use crate::db::test_pool;

    async fn seed_folder(
        pool: &sqlx::SqlitePool,
        id: &str,
        parent: Option<&str>,
        path: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO folders (id, parent_id, name, kind, path, expanded, position, created_at) \
             VALUES (?, ?, ?, 'temp', ?, 1, 0, 0)",
        )
        .bind(id)
        .bind(parent)
        .bind(id)
        .bind(path)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn push_trash(
        pool: &sqlx::SqlitePool,
        id: &str,
        kind: &str,
        label: &str,
        payload: serde_json::Value,
    ) {
        let payload_str = serde_json::to_string(&payload).unwrap();
        sqlx::query(
            "INSERT INTO trash (id, kind, label, payload, created_at) VALUES (?, ?, ?, ?, 0)",
        )
        .bind(id)
        .bind(kind)
        .bind(label)
        .bind(&payload_str)
        .execute(pool)
        .await
        .unwrap();
    }

    fn folder_json(id: &str, parent: Option<&str>, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id, "parentId": parent, "name": name, "kind": "temp",
            "path": null, "expanded": true, "position": 0, "createdAt": 0
        })
    }

    fn document_payload(id: &str) -> serde_json::Value {
        serde_json::json!({
            "document": {
                "id": id, "path": "/manual.pdf", "title": "manual.pdf",
                "pages": 42, "sizeBytes": 1024, "hash": null, "categoryId": null,
                "lastPage": 1, "lastZoom": 100.0, "lastOpened": null,
                "inRecent": false, "inLibrary": true, "createdAt": 0,
                "view": "{}", "position": 0
            },
            "folderDoc": null
        })
    }

    #[tokio::test]
    async fn restore_folder_with_path_collision_returns_coded_error() {
        let pool = test_pool().await;
        seed_folder(&pool, "recreated", None, Some("/libro")).await;

        push_trash(
            &pool,
            "trash-1",
            "folder",
            "Eliminar carpeta «libro»",
            serde_json::json!({
                "folder": {
                    "id": "original", "parentId": null, "name": "libro",
                    "kind": "synced", "path": "/libro", "expanded": true,
                    "position": 0, "createdAt": 0
                },
                "folderDocs": [], "documents": []
            }),
        )
        .await;

        let result = restore_from_trash_inner(&pool, "trash-1").await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.starts_with("TRASH_PATH_COLLISION:"), "error: {error}");

        // La papelera no debe haberse vaciado.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trash WHERE id = 'trash-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn restore_folder_without_collision_succeeds() {
        let pool = test_pool().await;
        push_trash(
            &pool,
            "trash-1",
            "folder",
            "Eliminar carpeta «libro»",
            serde_json::json!({
                "folder": {
                    "id": "original", "parentId": null, "name": "libro",
                    "kind": "synced", "path": "/libro", "expanded": true,
                    "position": 0, "createdAt": 0
                },
                "folderDocs": [], "documents": []
            }),
        )
        .await;

        restore_from_trash_inner(&pool, "trash-1").await.unwrap();

        let folder_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM folders WHERE id = 'original'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(folder_count, 1);

        let trash_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM trash WHERE id = 'trash-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(trash_count, 0);
    }

    /// El snapshot real no lo construye la UI: `remove_from_library` serializa
    /// `DocumentRecord`, cuyo campo `view` es un `serde_json::Value`, así que en
    /// el payload la vista llega como **objeto**. El test de abajo la pasa como
    /// cadena, que es la forma que usan los snapshots de carpeta.
    ///
    /// Y hay un detalle que hacía pasar la prueba sin probar nada: quitar de la
    /// biblioteca deja la fila (`INSERT OR IGNORE` no escribe), así que la vista
    /// solo se perdía cuando la fila ya no existía — p. ej. tras eliminar los
    /// datos del documento, que es justo cuando el usuario restaura.
    #[tokio::test]
    async fn restoring_after_removing_from_the_library_keeps_the_document_view() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, last_opened, in_recent, in_library, \
                                    created_at, view_json, position) \
             VALUES ('doc-1', '/manual.pdf', 'manual.pdf', 42, 1024, 'abc123', NULL, \
                     7, 150.0, 111, 1, 1, 0, '{\"pagesPerRow\":2}', 3)",
        )
        .execute(&pool)
        .await
        .unwrap();

        crate::commands::library::remove_from_library_inner(&pool, "doc-1".into())
            .await
            .unwrap();

        let payload: String = sqlx::query_scalar("SELECT payload FROM trash LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert!(
            !payload["document"]["view"].is_string(),
            "el snapshot de documento guarda la vista como objeto JSON, no como cadena"
        );

        // La fila desaparece antes de restaurar: ahora sí se inserta.
        sqlx::query("DELETE FROM documents WHERE id = 'doc-1'")
            .execute(&pool)
            .await
            .unwrap();

        let trash_id: String = sqlx::query_scalar("SELECT id FROM trash LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        restore_from_trash_inner(&pool, &trash_id).await.unwrap();

        let (view, position, hash, last_opened): (String, i64, Option<String>, Option<i64>) =
            sqlx::query_as(
                "SELECT view_json, position, hash, last_opened FROM documents WHERE id = 'doc-1'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(view, "{\"pagesPerRow\":2}");
        // Al restaurar se hace hueco y luego se recompactan los hermanos, así que
        // un documento solo en la raíz queda en el índice 0. Lo que importa del
        // orden (que recupere su sitio entre varios hermanos) se comprueba en
        // `restoring_a_root_document_keeps_the_positions_contiguous`.
        assert_eq!(position, 0);
        assert_eq!(hash.as_deref(), Some("abc123"));
        assert_eq!(last_opened, Some(111));
    }

    /// Restaurar no debe dejar dos documentos con la misma `position` en la raíz:
    /// con posiciones repetidas el orden visible depende de `created_at`, así que
    /// el documento restaurado podía aparecer en un sitio u otro según el id.
    #[tokio::test]
    async fn restoring_a_root_document_keeps_the_positions_contiguous() {
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

        crate::commands::library::remove_from_library_inner(&pool, "doc-b".into())
            .await
            .unwrap();
        sqlx::query("DELETE FROM documents WHERE id = 'doc-b'")
            .execute(&pool)
            .await
            .unwrap();
        let trash_id: String = sqlx::query_scalar("SELECT id FROM trash LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        restore_from_trash_inner(&pool, &trash_id).await.unwrap();

        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT id, position FROM documents WHERE in_library = 1 ORDER BY position",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![
                ("doc-a".to_string(), 0),
                ("doc-b".to_string(), 1),
                ("doc-c".to_string(), 2),
            ]
        );
    }

    /// Payload con la vista como **cadena**, que es la forma que usan los
    /// snapshots de carpeta (leen `view_json` crudo). La forma de los snapshots
    /// de documento (objeto) se cubre en
    /// `restoring_after_removing_from_the_library_keeps_the_document_view`.
    #[tokio::test]
    async fn restore_document_preserves_the_view_and_hash() {
        let pool = test_pool().await;
        push_trash(
            &pool,
            "trash-1",
            "document",
            "Quitar «manual»",
            serde_json::json!({
                "document": {
                    "id": "doc-1", "path": "/manual.pdf", "title": "manual.pdf",
                    "pages": 42, "sizeBytes": 1024, "hash": "abc123",
                    "categoryId": null, "lastPage": 7, "lastZoom": 150.0,
                    "lastOpened": 0, "inRecent": true, "inLibrary": true,
                    "createdAt": 0, "view": "{\"pagesPerRow\":2}", "position": 3
                },
                "folderDoc": null
            }),
        )
        .await;

        restore_from_trash_inner(&pool, "trash-1").await.unwrap();

        let (view, hash): (String, Option<String>) =
            sqlx::query_as("SELECT view_json, hash FROM documents WHERE id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(view, "{\"pagesPerRow\":2}");
        assert_eq!(hash.as_deref(), Some("abc123"));
    }

    async fn seed_document(pool: &sqlx::SqlitePool, id: &str, path: &str, in_library: bool) {
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, hash, category_id, \
                                    last_page, last_zoom, last_opened, in_recent, in_library, \
                                    created_at, view_json, position) \
             VALUES (?, ?, ?, 1, 1, NULL, NULL, 1, 100.0, NULL, 0, ?, 0, '{}', 0)",
        )
        .bind(id)
        .bind(path)
        .bind(id)
        .bind(in_library)
        .execute(pool)
        .await
        .unwrap();
    }

    /// El snapshot de una carpeta incluye todo el subárbol: si solo volviera la
    /// raíz, los `folder_docs` de las hijas chocarían con la FK y se
    /// descartarían en silencio, dejando los documentos sin carpeta.
    #[tokio::test]
    async fn restore_folder_restores_the_whole_subtree() {
        let pool = test_pool().await;
        push_trash(
            &pool,
            "trash-1",
            "folder",
            "Eliminar carpeta «libro»",
            serde_json::json!({
                "folder": folder_json("root", None, "libro"),
                "folders": [folder_json("root", None, "libro"), folder_json("child", Some("root"), "capitulos")],
                "folderDocs": [{ "folderId": "child", "docId": "doc-1", "position": 0 }],
                "documents": [{
                    "id": "doc-1", "path": "/manual.pdf", "title": "manual.pdf",
                    "pages": 3, "sizeBytes": 1, "hash": null, "categoryId": null,
                    "lastPage": 1, "lastZoom": 100.0, "lastOpened": null,
                    "inRecent": false, "inLibrary": true, "createdAt": 0,
                    "view": "{}", "position": 0
                }]
            }),
        )
        .await;

        restore_from_trash_inner(&pool, "trash-1").await.unwrap();

        let folders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(folders, 2, "la subcarpeta debe volver con la carpeta");

        let linked: String =
            sqlx::query_scalar("SELECT folder_id FROM folder_docs WHERE doc_id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, "child", "el documento debe volver a su subcarpeta");
    }

    /// Restaurar sin padre dejaría la carpeta fuera (la FK lo impide y
    /// `INSERT OR IGNORE` no avisa) y borraría la entrada de la papelera.
    #[tokio::test]
    async fn restore_folder_without_parent_keeps_the_trash_entry() {
        let pool = test_pool().await;
        push_trash(
            &pool,
            "trash-1",
            "folder",
            "Eliminar carpeta «hija»",
            serde_json::json!({
                "folder": folder_json("child", Some("padre-ausente"), "hija")
            }),
        )
        .await;

        let error = restore_from_trash_inner(&pool, "trash-1")
            .await
            .unwrap_err();
        assert!(error.starts_with("TRASH_MISSING_PARENT:"), "error: {error}");

        let folders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(folders, 0);

        let trash: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trash")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(trash, 1, "la entrada debe seguir en la papelera");
    }

    #[tokio::test]
    async fn restore_folder_with_id_collision_keeps_the_trash_entry() {
        let pool = test_pool().await;
        seed_folder(&pool, "same-id", None, Some("/actual")).await;
        push_trash(
            &pool,
            "trash-1",
            "folder",
            "Eliminar carpeta «anterior»",
            serde_json::json!({
                "folder": {
                    "id": "same-id", "parentId": null, "name": "anterior",
                    "kind": "temp", "path": "/anterior", "expanded": true,
                    "position": 0, "createdAt": 0
                }
            }),
        )
        .await;

        let error = restore_from_trash_inner(&pool, "trash-1")
            .await
            .unwrap_err();
        assert!(error.starts_with("TRASH_ID_COLLISION:"), "error: {error}");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM trash")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn restore_document_with_path_collision_keeps_the_trash_entry() {
        let pool = test_pool().await;
        seed_document(&pool, "otro", "/manual.pdf", true).await;
        push_trash(
            &pool,
            "trash-1",
            "document",
            "Quitar «manual»",
            document_payload("doc-1"),
        )
        .await;

        let error = restore_from_trash_inner(&pool, "trash-1")
            .await
            .unwrap_err();
        assert!(error.starts_with("TRASH_PATH_COLLISION:"), "error: {error}");

        let docs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM documents")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(docs, 1);
        let trash: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trash")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(trash, 1, "la entrada debe seguir en la papelera");
    }

    #[tokio::test]
    async fn restoring_missing_relations_keeps_the_restore_and_reports_warnings() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, size_bytes, category_id, last_page, \
             last_zoom, in_recent, in_library, created_at, view_json, position) \
             VALUES ('doc-1', '/doc.pdf', 'doc.pdf', 1, 1, NULL, 1, 100, 0, 1, 0, '{}', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        push_trash(
            &pool,
            "trash-document",
            "document",
            "Quitar documento",
            serde_json::json!({
                "document": {
                    "id": "doc-1", "path": "/doc.pdf", "title": "doc.pdf",
                    "pages": 1, "sizeBytes": 1, "categoryId": null,
                    "categoryIds": ["missing-category"],
                    "lastPage": 1, "lastZoom": 100, "inRecent": false, "inLibrary": true,
                    "createdAt": 0, "view": {}, "position": 0
                },
                "folderDoc": null
            }),
        )
        .await;
        push_trash(
            &pool,
            "trash-category",
            "category",
            "Eliminar categoría",
            serde_json::json!({
                "category": {
                    "id": "category-1", "name": "Categoría", "color": "#fff", "position": 0
                },
                "documentIds": ["missing-document"]
            }),
        )
        .await;
        push_trash(
            &pool,
            "trash-bookmark",
            "bookmark",
            "Eliminar marcador",
            serde_json::json!({
                "bookmark": {
                    "id": "bookmark-1", "docId": "missing-document", "page": 1,
                    "name": "Marcador", "createdAt": 0
                }
            }),
        )
        .await;
        push_trash(
            &pool,
            "trash-comment",
            "comment",
            "Eliminar comentario",
            serde_json::json!({
                "comment": {
                    "id": "comment-1", "docId": "missing-document", "page": 1,
                    "text": "Comentario", "createdAt": 0
                }
            }),
        )
        .await;

        let document_warnings = restore_from_trash_inner(&pool, "trash-document")
            .await
            .unwrap();
        let category_warnings = restore_from_trash_inner(&pool, "trash-category")
            .await
            .unwrap();
        let bookmark_warnings = restore_from_trash_inner(&pool, "trash-bookmark")
            .await
            .unwrap();
        let comment_warnings = restore_from_trash_inner(&pool, "trash-comment")
            .await
            .unwrap();

        assert!(
            document_warnings
                .iter()
                .any(|warning| warning.starts_with("TRASH_MISSING_RELATION:category"))
        );
        assert!(
            category_warnings
                .iter()
                .any(|warning| warning.starts_with("TRASH_MISSING_RELATION:document"))
        );
        assert!(
            bookmark_warnings
                .iter()
                .any(|warning| warning.starts_with("TRASH_MISSING_RELATION:document"))
        );
        assert!(
            comment_warnings
                .iter()
                .any(|warning| warning.starts_with("TRASH_MISSING_RELATION:document"))
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM trash")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    /// Quitar un documento de la biblioteca deja la fila con `in_library = 0`,
    /// así que el `INSERT OR IGNORE` solo no lo devolvía a la biblioteca.
    #[tokio::test]
    async fn restore_document_reactivates_one_removed_from_the_library() {
        let pool = test_pool().await;
        seed_document(&pool, "doc-1", "/manual.pdf", false).await;
        push_trash(
            &pool,
            "trash-1",
            "document",
            "Quitar «manual»",
            document_payload("doc-1"),
        )
        .await;

        restore_from_trash_inner(&pool, "trash-1").await.unwrap();

        let in_library: i64 =
            sqlx::query_scalar("SELECT in_library FROM documents WHERE id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(in_library, 1);
        let trash: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trash")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(trash, 0);
    }

    #[tokio::test]
    async fn restore_document_with_deleted_folder_lands_in_the_root() {
        let pool = test_pool().await;
        seed_document(&pool, "doc-1", "/manual.pdf", true).await;
        seed_document(&pool, "doc-2", "/other.pdf", true).await;
        push_trash(
            &pool,
            "trash-1",
            "document",
            "Eliminar «manual»",
            serde_json::json!({
                "document": {
                    "id": "doc-1", "path": "/manual.pdf", "title": "manual.pdf",
                    "pages": 1, "sizeBytes": 1, "hash": null, "categoryId": null,
                    "lastPage": 1, "lastZoom": 100.0, "lastOpened": null,
                    "inRecent": false, "inLibrary": true, "createdAt": 0,
                    "view": "{}", "position": 0
                },
                "folderDoc": { "folderId": "deleted-folder", "docId": "doc-1", "position": 0 }
            }),
        )
        .await;

        let warnings = restore_from_trash_inner(&pool, "trash-1").await.unwrap();

        assert!(
            warnings
                .iter()
                .any(|warning| { warning == "TRASH_MISSING_RELATION:folder:deleted-folder" })
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM folder_docs WHERE doc_id = 'doc-1'",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT in_library FROM documents WHERE id = 'doc-1'",)
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM documents WHERE in_library = 1 \
                 AND id NOT IN (SELECT doc_id FROM folder_docs) \
                 AND position = (SELECT position FROM documents WHERE id = 'doc-1')",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1,
            "el documento restaurado ocupa una posición única en la raíz"
        );
    }

    /// Ida y vuelta completa: el snapshot que toma `delete_folder` tiene que
    /// bastar para reconstruir el árbol con sus enlaces.
    #[tokio::test]
    async fn deleting_and_restoring_a_folder_keeps_its_subtree() {
        let pool = test_pool().await;
        seed_folder(&pool, "root", None, None).await;
        seed_folder(&pool, "child", Some("root"), None).await;
        seed_document(&pool, "doc-1", "/manual.pdf", true).await;
        sqlx::query(
            "INSERT INTO folder_docs (folder_id, doc_id, position) VALUES ('child', 'doc-1', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        crate::commands::library::delete_folder_inner(&pool, "root".into(), true)
            .await
            .unwrap();

        let folders_left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(folders_left, 0);

        let trash_id: String = sqlx::query_scalar("SELECT id FROM trash")
            .fetch_one(&pool)
            .await
            .unwrap();
        restore_from_trash_inner(&pool, &trash_id).await.unwrap();

        let folders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(folders, 2, "el subárbol debe volver entero");

        let linked: String =
            sqlx::query_scalar("SELECT folder_id FROM folder_docs WHERE doc_id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(linked, "child");
    }

    #[tokio::test]
    async fn restore_document_rejects_an_empty_id() {
        let pool = test_pool().await;
        let mut payload = document_payload("doc-1");
        payload["document"]["id"] = serde_json::Value::String(String::new());
        push_trash(&pool, "trash-empty-id", "document", "Documento", payload).await;

        let error = restore_from_trash_inner(&pool, "trash-empty-id")
            .await
            .unwrap_err();
        assert!(error.starts_with("TRASH_MISSING_RELATION:"));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM documents WHERE id = ''")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn restore_bookmark_reactivates_existing_document() {
        let pool = test_pool().await;
        seed_document(&pool, "doc-1", "/manual.pdf", false).await;
        push_trash(
            &pool,
            "trash-bookmark",
            "bookmark",
            "Marcador",
            serde_json::json!({
                "bookmark": {
                    "id": "bookmark-1", "docId": "doc-1", "page": 2,
                    "name": "Página 2", "createdAt": 0
                }
            }),
        )
        .await;

        restore_from_trash_inner(&pool, "trash-bookmark")
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT in_library FROM documents WHERE id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }
}
