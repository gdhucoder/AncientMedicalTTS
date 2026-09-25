CREATE TABLE IF NOT EXISTS segment_annotations (
    id TEXT PRIMARY KEY,
    segment_id TEXT NOT NULL,
    start_token INTEGER NOT NULL,
    end_token INTEGER NOT NULL,
    surface_text TEXT NOT NULL,
    default_pinyin TEXT,
    target_pinyin TEXT,
    candidates_json TEXT NOT NULL DEFAULT '[]',
    risk_type TEXT NOT NULL,
    reason TEXT,
    review_status TEXT NOT NULL,
    analyzer_version TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(segment_id) REFERENCES segments(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_annotations_segment
ON segment_annotations(segment_id, start_token);

UPDATE app_meta SET schema_version = 3, app_version = '0.1.0';
