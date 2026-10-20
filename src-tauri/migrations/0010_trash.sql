-- 0010_trash.sql
--
-- Papelera persistente. Cada operación destructiva guarda un snapshot JSON
-- de las filas afectadas. El usuario puede restaurar o vaciar la papelera.

CREATE TABLE IF NOT EXISTS trash (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL,
  label       TEXT NOT NULL,
  payload     TEXT NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_trash_created ON trash(created_at DESC);
