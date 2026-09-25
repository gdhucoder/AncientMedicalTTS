ALTER TABLE segment_annotations
ADD COLUMN source TEXT;

CREATE INDEX IF NOT EXISTS idx_annotations_source
ON segment_annotations(source);

UPDATE app_meta SET schema_version = 6, app_version = '0.1.0';
