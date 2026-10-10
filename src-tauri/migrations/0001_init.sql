-- 0001_init.sql

CREATE TABLE IF NOT EXISTS categories (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  color       TEXT NOT NULL,
  position    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS documents (
  id            TEXT PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  title         TEXT NOT NULL,
  pages         INTEGER NOT NULL,
  size_bytes    INTEGER,
  hash          TEXT,
  category_id   TEXT REFERENCES categories(id) ON DELETE SET NULL,
  last_page     INTEGER DEFAULT 1,
  last_zoom     REAL DEFAULT 100,
  last_opened   INTEGER,
  in_recent     INTEGER DEFAULT 0,
  created_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_category ON documents(category_id);
CREATE INDEX IF NOT EXISTS idx_documents_recent ON documents(in_recent, last_opened DESC);

CREATE TABLE IF NOT EXISTS folders (
  id          TEXT PRIMARY KEY,
  parent_id   TEXT REFERENCES folders(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL,
  expanded    INTEGER DEFAULT 0,
  position    INTEGER NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id, position);

CREATE TABLE IF NOT EXISTS folder_docs (
  folder_id  TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  doc_id     TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  position   INTEGER NOT NULL,
  PRIMARY KEY (folder_id, doc_id)
);

CREATE INDEX IF NOT EXISTS idx_folder_docs_pos ON folder_docs(folder_id, position);

CREATE TABLE IF NOT EXISTS bookmarks (
  id          TEXT PRIMARY KEY,
  doc_id      TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  page        INTEGER NOT NULL,
  name        TEXT DEFAULT '',
  created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_bookmarks_doc ON bookmarks(doc_id, page);

CREATE TABLE IF NOT EXISTS comments (
  id          TEXT PRIMARY KEY,
  doc_id      TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  page        INTEGER,
  text        TEXT NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_comments_doc ON comments(doc_id, page);
