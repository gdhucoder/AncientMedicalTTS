ALTER TABLE segment_annotations ADD COLUMN rule_type TEXT;
ALTER TABLE segment_annotations ADD COLUMN confidence TEXT;

UPDATE app_meta SET schema_version = 9, app_version = '0.1.0';
