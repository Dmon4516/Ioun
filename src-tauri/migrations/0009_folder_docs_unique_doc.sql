-- 0009_folder_docs_unique_doc.sql
--
-- Un documento pertenece a una sola carpeta. La PK compuesta permitía
-- múltiples membresías, pero la UI y los comandos asumen una sola.

DELETE FROM folder_docs
WHERE rowid NOT IN (
  SELECT MIN(rowid) FROM folder_docs GROUP BY doc_id
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_folder_docs_doc_unique
  ON folder_docs(doc_id);
