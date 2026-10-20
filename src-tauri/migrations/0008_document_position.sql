-- 0008_document_position.sql
--
-- Los documentos raíz se ordenaban por created_at. Añadimos `position` que
-- solo aplica cuando el documento no está en ninguna carpeta.

ALTER TABLE documents ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

UPDATE documents
SET position = (
  SELECT rn FROM (
    SELECT id, ROW_NUMBER() OVER (ORDER BY created_at ASC, rowid ASC) - 1 AS rn
    FROM documents
    WHERE in_library = 1
  ) AS ordered
  WHERE ordered.id = documents.id
)
WHERE in_library = 1;

CREATE INDEX IF NOT EXISTS idx_documents_position
  ON documents(position);
