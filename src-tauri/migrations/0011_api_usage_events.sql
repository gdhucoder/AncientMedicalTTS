CREATE TABLE IF NOT EXISTS api_usage_events (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    provider TEXT NOT NULL,
    service TEXT NOT NULL,
    model TEXT,
    operation TEXT NOT NULL,
    unit_type TEXT NOT NULL,
    input_units INTEGER NOT NULL DEFAULT 0,
    output_units INTEGER NOT NULL DEFAULT 0,
    success INTEGER NOT NULL,
    error_code TEXT,
    metadata_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_api_usage_events_created_at
ON api_usage_events(created_at);

CREATE INDEX IF NOT EXISTS idx_api_usage_events_service_operation
ON api_usage_events(service, operation);

UPDATE app_meta SET schema_version = 11, app_version = '0.1.0';
