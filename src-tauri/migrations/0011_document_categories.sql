-- 0011_document_categories.sql
--
-- A document can belong to more than one category. The legacy
-- `documents.category_id` column is kept for compatibility with old data and
-- is maintained as the first assigned category; the join table is authoritative.

CREATE TABLE IF NOT EXISTS document_categories (
  doc_id      TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
  PRIMARY KEY (doc_id, category_id)
);

CREATE INDEX IF NOT EXISTS idx_document_categories_category
  ON document_categories(category_id);

INSERT OR IGNORE INTO document_categories (doc_id, category_id)
SELECT id, category_id
FROM documents
WHERE category_id IS NOT NULL;
