/// The initial schema for the Positronic Vault.
pub const MIGRATION_INIT: &str = r#"
CREATE TABLE IF NOT EXISTS session (
    id TEXT PRIMARY KEY,
    start_time INTEGER NOT NULL,
    end_time INTEGER
);

CREATE TABLE IF NOT EXISTS history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    command TEXT NOT NULL,
    output TEXT, -- Can be NULL if command had no output or was huge
    exit_code INTEGER,
    timestamp INTEGER NOT NULL,
    directory TEXT NOT NULL,
    duration_ms INTEGER,
    FOREIGN KEY(session_id) REFERENCES session(id)
);

CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp);
CREATE INDEX IF NOT EXISTS idx_history_command ON history(command);
"#;
