CREATE TABLE IF NOT EXISTS synced_file_state (
  path          TEXT PRIMARY KEY,
  size_bytes    INTEGER NOT NULL,
  modified_at   INTEGER NOT NULL,
  error         TEXT
);
