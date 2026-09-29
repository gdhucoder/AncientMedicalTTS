ALTER TABLE books ADD COLUMN import_format TEXT;
ALTER TABLE books ADD COLUMN import_format_version TEXT;
ALTER TABLE books ADD COLUMN import_pronunciation_mode TEXT;
ALTER TABLE books ADD COLUMN import_dataset_id TEXT;
ALTER TABLE books ADD COLUMN imported_at TEXT;

ALTER TABLE chapters ADD COLUMN collection TEXT;
ALTER TABLE chapters ADD COLUMN subtitle TEXT;

ALTER TABLE segments ADD COLUMN translation TEXT;

UPDATE app_meta SET schema_version = 14, app_version = '0.1.0';
