use crate::db::{
    self,
    models::{Bookmark, Comment},
};
use crate::ids::new_id;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn list_bookmarks(doc_id: String) -> Result<Vec<Bookmark>, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let rows: Vec<Bookmark> = sqlx::query_as(
        "SELECT id, doc_id, page, name, created_at FROM bookmarks WHERE doc_id = ? ORDER BY page ASC",
    )
    .bind(&doc_id).fetch_all(pool).await.map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Valida lo que la interfaz ya comprueba, para que una llamada directa (o una
/// biblioteca importada a mano) no meta una página imposible en la base.
///
/// El nombre del marcador **no** se valida: vacío es su valor normal, porque la
/// interfaz etiqueta cada marcador con su número de página.
fn validate_page(page: Option<i64>, max_pages: Option<i64>) -> Result<(), String> {
    match page {
        Some(value) if value < 1 => Err("La página debe ser 1 o mayor".into()),
        Some(value) if max_pages.is_some_and(|max| max > 0 && value > max) => {
            Err("La página supera el total del documento".into())
        }
        _ => Ok(()),
    }
}

async fn document_page_count(pool: &sqlx::SqlitePool, doc_id: &str) -> Result<Option<i64>, String> {
    sqlx::query_scalar("SELECT pages FROM documents WHERE id = ?")
        .bind(doc_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())
}

async fn require_document(pool: &sqlx::SqlitePool, doc_id: &str) -> Result<Option<i64>, String> {
    let pages = document_page_count(pool, doc_id).await?;
    if pages.is_none() {
        return Err("No se encontró el documento".into());
    }
    Ok(pages)
}

/// El texto del comentario se guarda recortado y nunca vacío.
fn normalize_comment_text(text: String) -> Result<String, String> {
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err("El comentario no puede estar vacío".into());
    }
    Ok(trimmed)
}

#[tauri::command]
pub async fn add_bookmark(doc_id: String, page: i64, name: String) -> Result<Bookmark, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    validate_page(Some(page), require_document(pool, &doc_id).await?)?;
    let id = new_id("bm");
    let now = now_ms();
    sqlx::query(
        "INSERT INTO bookmarks (id, doc_id, page, name, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&doc_id)
    .bind(page)
    .bind(&name)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(Bookmark {
        id,
        doc_id,
        page,
        name,
        created_at: now,
    })
}

#[tauri::command]
pub async fn delete_bookmark(id: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;

    let bookmark: Option<Bookmark> =
        sqlx::query_as("SELECT id, doc_id, page, name, created_at FROM bookmarks WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    let Some(bookmark) = bookmark else {
        return Err("No se encontró el marcador".into());
    };

    crate::commands::trash::push(
        &mut transaction,
        "bookmark",
        &format!("Eliminar marcador de página {}", bookmark.page),
        &serde_json::json!({ "bookmark": bookmark }),
    )
    .await?;

    sqlx::query("DELETE FROM bookmarks WHERE id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_comments(doc_id: String) -> Result<Vec<Comment>, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let rows: Vec<Comment> = sqlx::query_as(
        "SELECT id, doc_id, page, text, created_at FROM comments WHERE doc_id = ? \
         ORDER BY page IS NULL, page ASC, created_at ASC",
    )
    .bind(&doc_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
pub async fn add_comment(
    doc_id: String,
    page: Option<i64>,
    text: String,
) -> Result<Comment, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    validate_page(page, require_document(pool, &doc_id).await?)?;
    let text = normalize_comment_text(text)?;
    let id = new_id("cm");
    let now = now_ms();
    sqlx::query("INSERT INTO comments (id, doc_id, page, text, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&doc_id)
        .bind(page)
        .bind(&text)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Comment {
        id,
        doc_id,
        page,
        text,
        created_at: now,
    })
}

#[tauri::command]
pub async fn delete_comment(id: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;

    let comment: Option<Comment> =
        sqlx::query_as("SELECT id, doc_id, page, text, created_at FROM comments WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    let Some(comment) = comment else {
        return Err("No se encontró el comentario".into());
    };

    crate::commands::trash::push(
        &mut transaction,
        "comment",
        "Eliminar comentario",
        &serde_json::json!({ "comment": comment }),
    )
    .await?;

    sqlx::query("DELETE FROM comments WHERE id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn rejects_pages_below_one() {
        assert!(validate_page(Some(0), Some(120)).is_err());
        assert!(validate_page(Some(-3), Some(120)).is_err());
        assert!(validate_page(Some(1), Some(120)).is_ok());
        assert!(validate_page(Some(120), Some(120)).is_ok());
        assert!(validate_page(Some(121), Some(120)).is_err());
        assert!(validate_page(Some(120), Some(0)).is_ok());
        // Sin página el comentario es válido: cuelga del documento, no de una hoja.
        assert!(validate_page(None, Some(120)).is_ok());
    }

    #[test]
    fn trims_comment_text_and_rejects_blank() {
        assert_eq!(normalize_comment_text("  hola  ".into()).unwrap(), "hola");
        assert!(normalize_comment_text("   ".into()).is_err());
        assert!(normalize_comment_text("\n\t".into()).is_err());
        assert!(normalize_comment_text(String::new()).is_err());
    }
}
