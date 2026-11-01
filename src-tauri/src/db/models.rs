use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRecord {
    pub id: String,
    pub path: String,
    pub title: String,
    pub kind: String,
    pub pages: i64,
    pub size_bytes: Option<i64>,
    pub hash: Option<String>,
    pub category_ids: sqlx::types::Json<Vec<String>>,
    pub last_page: i64,
    pub last_zoom: f64,
    pub last_opened: Option<i64>,
    pub in_recent: bool,
    pub in_library: bool,
    pub created_at: i64,
    pub view: serde_json::Value,
    pub view_state: serde_json::Value,
    pub position: i64,
}

/// Columns used by queries that hydrate `DocumentRecord`.
pub const DOCUMENT_COLUMNS: &str = "\
    documents.id, documents.path, documents.title, documents.kind, documents.pages, \
    documents.size_bytes, documents.hash, documents.category_id, \
    COALESCE((SELECT json_group_array(category_id) FROM document_categories \
              WHERE doc_id = documents.id), '[]') AS category_ids, \
    documents.last_page, documents.last_zoom, documents.last_opened, \
    documents.in_recent, documents.in_library, documents.created_at, \
    documents.view_json AS view, documents.view_state, documents.position";

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FolderRecord {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub path: Option<String>,
    pub expanded: bool,
    pub position: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub doc_id: String,
    pub page: i64,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub doc_id: String,
    pub page: Option<i64>,
    pub text: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub color: String,
    pub position: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub folders: Vec<FolderRecord>,
    pub root_doc_ids: Vec<String>,
    pub folder_docs: std::collections::HashMap<String, Vec<String>>,
    /// Subcarpetas de cada carpeta, para poder mostrar el contador sin cargarlas.
    pub folder_child_counts: std::collections::HashMap<String, i64>,
}
