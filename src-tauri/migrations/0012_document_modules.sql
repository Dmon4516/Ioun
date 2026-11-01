-- Document format and format-specific view state.
ALTER TABLE documents ADD COLUMN kind TEXT NOT NULL DEFAULT 'pdf';
ALTER TABLE documents ADD COLUMN view_state TEXT NOT NULL DEFAULT '{}';

UPDATE documents
SET view_state = json_object(
  'page', COALESCE(last_page, 1),
  'zoom', COALESCE(last_zoom, 100)
)
WHERE kind = 'pdf';
