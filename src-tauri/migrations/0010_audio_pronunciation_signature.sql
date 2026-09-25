ALTER TABLE audio_versions ADD COLUMN pronunciation_signature TEXT;

UPDATE app_meta SET schema_version = 10, app_version = '0.1.0';
