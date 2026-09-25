INSERT INTO app_settings (key, value, updated_at)
VALUES
    ('reader.display.font_size', '25', strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('reader.display.pinyin_mode', 'risky', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
ON CONFLICT(key) DO NOTHING;

UPDATE app_meta SET schema_version = 7, app_version = '0.1.0';
