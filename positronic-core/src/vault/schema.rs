/// positronic-core/src/vault/schema.rs
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
    output TEXT,
    exit_code INTEGER,
    timestamp INTEGER NOT NULL,
    directory TEXT NOT NULL,
    duration_ms INTEGER,
    FOREIGN KEY(session_id) REFERENCES session(id)
);

CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp);
CREATE INDEX IF NOT EXISTS idx_history_command ON history(command);
"#;

/// V2 migration: aliases, bookmarks, config store.
pub const MIGRATION_V2: &str = r#"
-- User-defined command aliases
CREATE TABLE IF NOT EXISTS aliases (
    name TEXT PRIMARY KEY,
    expansion TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

-- Bookmarked commands for quick recall
CREATE TABLE IF NOT EXISTS bookmarks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    command TEXT NOT NULL,
    label TEXT,
    created_at INTEGER NOT NULL
);

-- Simple key-value config (settings that persist across sessions)
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Index for frequency queries on history
CREATE INDEX IF NOT EXISTS idx_history_session ON history(session_id);
CREATE INDEX IF NOT EXISTS idx_history_directory ON history(directory);
"#;