ALTER TABLE synced_file_state ADD COLUMN device_id INTEGER;
ALTER TABLE synced_file_state ADD COLUMN inode INTEGER;

CREATE INDEX IF NOT EXISTS idx_synced_file_state_identity
  ON synced_file_state(device_id, inode)
  WHERE device_id IS NOT NULL AND inode IS NOT NULL;
