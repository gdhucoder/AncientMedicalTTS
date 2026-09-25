CREATE TABLE IF NOT EXISTS app_meta (
    schema_version INTEGER NOT NULL,
    app_version TEXT NOT NULL
);

INSERT INTO app_meta (schema_version, app_version)
SELECT 1, '0.1.0'
WHERE NOT EXISTS (SELECT 1 FROM app_meta);
