ALTER TABLE segments ADD COLUMN corrected_text TEXT;

UPDATE app_meta SET schema_version = 14, app_version = '0.1.0';
