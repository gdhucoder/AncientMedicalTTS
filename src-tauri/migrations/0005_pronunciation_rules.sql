CREATE TABLE IF NOT EXISTS pronunciation_rules (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    book_id TEXT,
    pattern_text TEXT NOT NULL,
    target_pinyin TEXT NOT NULL,
    rule_type TEXT NOT NULL,
    source TEXT,
    verified INTEGER NOT NULL DEFAULT 1,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(book_id) REFERENCES books(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_pronunciation_rules_book
ON pronunciation_rules(book_id);

CREATE INDEX IF NOT EXISTS idx_pronunciation_rules_pattern
ON pronunciation_rules(pattern_text);

ALTER TABLE segment_annotations
ADD COLUMN source_rule_id TEXT REFERENCES pronunciation_rules(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_annotations_source_rule
ON segment_annotations(source_rule_id);

UPDATE app_meta SET schema_version = 5, app_version = '0.1.0';
