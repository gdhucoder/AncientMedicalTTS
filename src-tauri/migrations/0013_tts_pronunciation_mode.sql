ALTER TABLE audio_versions ADD COLUMN pronunciation_mode TEXT NOT NULL DEFAULT 'locked';

UPDATE app_meta SET schema_version = 13, app_version = '0.1.0';
