ALTER TABLE segments ADD COLUMN reading_text TEXT;
ALTER TABLE segments ADD COLUMN speak_enabled INTEGER NOT NULL DEFAULT 1;

UPDATE app_meta SET schema_version = 8, app_version = '0.1.0';
