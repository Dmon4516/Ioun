use crate::db::{self, models::Category};

#[tauri::command]
pub async fn list_categories() -> Result<Vec<Category>, String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let rows: Vec<Category> =
        sqlx::query_as("SELECT id, name, color, position FROM categories ORDER BY position ASC")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn valid_color(color: &str) -> bool {
    let hex = color.strip_prefix('#').unwrap_or("");
    matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[tauri::command]
pub async fn upsert_category(category: Category) -> Result<(), String> {
    if category.name.trim().is_empty() {
        return Err("El nombre de la categoría no puede estar vacío".into());
    }
    if !valid_color(&category.color) {
        return Err("El color de la categoría no es válido".into());
    }

    let pool = db::pool().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO categories (id, name, color, position) VALUES (?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, color = excluded.color, position = excluded.position",
    )
    .bind(&category.id).bind(&category.name).bind(&category.color).bind(category.position)
    .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_category(id: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;

    let category: Option<Category> =
        sqlx::query_as("SELECT id, name, color, position FROM categories WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    let Some(category) = category else {
        return Err("No se encontró la categoría".into());
    };
    let category_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories")
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    if category_count <= 1 {
        return Err("Debe conservarse al menos una categoría".into());
    }

    let assigned_doc_ids: Vec<String> =
        sqlx::query_scalar("SELECT doc_id FROM document_categories WHERE category_id = ?")
            .bind(&id)
            .fetch_all(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;

    crate::commands::trash::push(
        &mut transaction,
        "category",
        &format!("Eliminar categoría «{}»", category.name),
        &serde_json::json!({
            "category": category,
            "documentIds": assigned_doc_ids,
        }),
    )
    .await?;

    sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    transaction.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Adds or removes one category without disturbing the document's other
/// assignments. The legacy column is kept in sync with the first relation.
#[tauri::command]
pub async fn set_document_category(
    doc_id: String,
    category_id: String,
    assigned: bool,
) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    set_document_category_inner(pool, &doc_id, &category_id, assigned).await
}

pub async fn set_document_category_inner(
    pool: &sqlx::SqlitePool,
    doc_id: &str,
    category_id: &str,
    assigned: bool,
) -> Result<(), String> {
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;

    let document_exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM documents WHERE id = ?")
        .bind(doc_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    if document_exists.is_none() {
        return Err("No se encontró el documento".into());
    }

    let category_exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM categories WHERE id = ?")
        .bind(category_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    if category_exists.is_none() {
        return Err("No se encontró la categoría".into());
    }

    if assigned {
        sqlx::query(
            "INSERT OR IGNORE INTO document_categories (doc_id, category_id) VALUES (?, ?)",
        )
        .bind(doc_id)
        .bind(category_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    } else {
        sqlx::query("DELETE FROM document_categories WHERE doc_id = ? AND category_id = ?")
            .bind(doc_id)
            .bind(category_id)
            .execute(&mut *transaction)
            .await
            .map_err(|e| e.to_string())?;
    }

    sync_legacy_category(&mut transaction, doc_id).await?;
    transaction.commit().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_document_categories(doc_id: String) -> Result<(), String> {
    let pool = db::pool().await.map_err(|e| e.to_string())?;
    clear_document_categories_inner(pool, &doc_id).await
}

pub async fn clear_document_categories_inner(
    pool: &sqlx::SqlitePool,
    doc_id: &str,
) -> Result<(), String> {
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let document_exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM documents WHERE id = ?")
        .bind(doc_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    if document_exists.is_none() {
        return Err("No se encontró el documento".into());
    }
    sqlx::query("DELETE FROM document_categories WHERE doc_id = ?")
        .bind(doc_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    sync_legacy_category(&mut transaction, doc_id).await?;
    transaction.commit().await.map_err(|e| e.to_string())
}

async fn sync_legacy_category(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    doc_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE documents SET category_id = ( \
           SELECT category_id FROM document_categories \
           WHERE doc_id = ? ORDER BY rowid LIMIT 1 \
         ) WHERE id = ?",
    )
    .bind(doc_id)
    .bind(doc_id)
    .execute(&mut **transaction)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{clear_document_categories_inner, set_document_category_inner, valid_color};
    use crate::db::test_pool;

    async fn seed(pool: &sqlx::SqlitePool) {
        sqlx::query(
            "INSERT INTO documents (id, path, title, pages, last_page, last_zoom, \
             in_recent, in_library, created_at, view_json, position) \
             VALUES ('doc-1', '/tmp/doc.pdf', 'Doc', 1, 0, 1, 0, 1, 1, '{}', 0)",
        )
        .execute(pool)
        .await
        .unwrap();
        for (id, position) in [("cat-a", 0), ("cat-b", 1)] {
            sqlx::query(
                "INSERT INTO categories (id, name, color, position) VALUES (?, ?, '#fff', ?)",
            )
            .bind(id)
            .bind(id)
            .bind(position)
            .execute(pool)
            .await
            .unwrap();
        }
    }

    #[test]
    fn validates_hex_category_colors() {
        assert!(valid_color("#fff"));
        assert!(valid_color("#112233"));
        assert!(valid_color("#11223380"));
        assert!(!valid_color("fff"));
        assert!(!valid_color("#12"));
        assert!(!valid_color("#ggg"));
    }

    #[tokio::test]
    async fn document_keeps_multiple_categories_and_legacy_value_tracks_first_relation() {
        let pool = test_pool().await;
        seed(&pool).await;

        set_document_category_inner(&pool, "doc-1", "cat-a", true)
            .await
            .unwrap();
        set_document_category_inner(&pool, "doc-1", "cat-b", true)
            .await
            .unwrap();
        set_document_category_inner(&pool, "doc-1", "cat-b", true)
            .await
            .unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM document_categories WHERE doc_id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 2);

        set_document_category_inner(&pool, "doc-1", "cat-a", false)
            .await
            .unwrap();
        let legacy: Option<String> =
            sqlx::query_scalar("SELECT category_id FROM documents WHERE id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(legacy.as_deref(), Some("cat-b"));

        clear_document_categories_inner(&pool, "doc-1")
            .await
            .unwrap();
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM document_categories WHERE doc_id = 'doc-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn deleting_category_cascades_its_document_relations() {
        let pool = test_pool().await;
        seed(&pool).await;
        set_document_category_inner(&pool, "doc-1", "cat-a", true)
            .await
            .unwrap();

        sqlx::query("DELETE FROM categories WHERE id = 'cat-a'")
            .execute(&pool)
            .await
            .unwrap();

        let relation: Option<String> = sqlx::query_scalar(
            "SELECT category_id FROM document_categories WHERE doc_id = 'doc-1'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(relation, None);
    }
}
