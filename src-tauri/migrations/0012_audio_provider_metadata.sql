ALTER TABLE audio_versions ADD COLUMN provider_metadata_json TEXT;

UPDATE app_meta SET schema_version = 12, app_version = '0.1.0';
