-- 0007_indexes.sql
--
-- Índices que faltaban:
--  * folders.path único (parcial): evita carpetas sincronizadas duplicadas.
--  * folder_docs.doc_id: acelera DELETE ... WHERE doc_id = ? y JOINs por documento.

DELETE FROM folder_docs
WHERE folder_id IN (
  SELECT id FROM folders
  WHERE path IS NOT NULL
    AND id NOT IN (
      SELECT id FROM (
        SELECT id,
               ROW_NUMBER() OVER (
                 PARTITION BY path
                 ORDER BY created_at ASC, id ASC
               ) AS rn
        FROM folders
        WHERE path IS NOT NULL
      )
      WHERE rn = 1
    )
);

DELETE FROM folders
WHERE path IS NOT NULL
  AND id NOT IN (
    SELECT id FROM (
      SELECT id,
             ROW_NUMBER() OVER (
               PARTITION BY path
               ORDER BY created_at ASC, id ASC
             ) AS rn
      FROM folders
      WHERE path IS NOT NULL
    )
    WHERE rn = 1
  );

CREATE UNIQUE INDEX IF NOT EXISTS idx_folders_path_unique
  ON folders(path)
  WHERE path IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_folder_docs_doc
  ON folder_docs(doc_id);
