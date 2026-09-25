CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_versions (
    id TEXT PRIMARY KEY,
    segment_id TEXT NOT NULL,
    version_no INTEGER NOT NULL,
    provider TEXT NOT NULL,
    voice_type INTEGER NOT NULL,
    sample_rate INTEGER NOT NULL,
    codec TEXT NOT NULL,
    speed REAL NOT NULL,
    volume REAL NOT NULL,
    ssml TEXT,
    audio_path TEXT NOT NULL,
    provider_request_id TEXT,
    provider_session_id TEXT,
    duration_ms INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY(segment_id) REFERENCES segments(id) ON DELETE CASCADE,
    UNIQUE(segment_id, version_no)
);

CREATE INDEX IF NOT EXISTS idx_audio_versions_segment
ON audio_versions(segment_id, version_no);

UPDATE app_meta SET schema_version = 4, app_version = '0.1.0';
