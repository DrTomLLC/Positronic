// positronic-core/src/vault/mod.rs

use chrono::Utc;
use rusqlite::{Connection, Result, params};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub mod schema;

// ════════════════════════════════════════════════════════════════════
// Data types
// ════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub id: Option<i64>,
    pub session_id: String,
    pub command: String,
    pub output: Option<String>,
    pub exit_code: Option<i32>,
    pub directory: String,
    pub duration_ms: Option<i64>,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct Alias {
    pub name: String,
    pub expansion: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub id: i64,
    pub command: String,
    pub label: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct VaultStats {
    pub total_commands: i64,
    pub session_commands: i64,
    pub total_sessions: i64,
    pub unique_commands: i64,
    pub alias_count: i64,
    pub bookmark_count: i64,
    pub earliest_timestamp: Option<i64>,
    pub db_size_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct TopCommand {
    pub command: String,
    pub count: i64,
}

// ════════════════════════════════════════════════════════════════════
// Vault
// ════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Vault {
    conn: Arc<Mutex<Connection>>,
    session_id: String,
    start_time: i64,
}

impl Vault {
    /// Open the Vault at the specified path.
    /// Creates the database file and runs all migrations if needed.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(&path)?;

        // WAL mode for better concurrency
        conn.pragma_update(None, "journal_mode", "WAL")?;

        // Run migrations in order
        conn.execute_batch(schema::MIGRATION_INIT)?;
        conn.execute_batch(schema::MIGRATION_V2)?;

        let session_id = Uuid::new_v4().to_string();
        let start_time = Utc::now().timestamp();

        let vault = Self {
            conn: Arc::new(Mutex::new(conn)),
            session_id,
            start_time,
        };

        vault.start_session()?;

        Ok(vault)
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn start_time(&self) -> i64 {
        self.start_time
    }

    // ────────────────────────────────────────────────────────────────
    // Sessions
    // ────────────────────────────────────────────────────────────────

    fn start_session(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO session (id, start_time) VALUES (?1, ?2)",
            params![self.session_id, self.start_time],
        )?;
        Ok(())
    }

    /// Mark the current session as ended.
    pub fn close_session(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE session SET end_time = ?1 WHERE id = ?2",
            params![Utc::now().timestamp(), self.session_id],
        )?;
        Ok(())
    }

    // ────────────────────────────────────────────────────────────────
    // Command History
    // ────────────────────────────────────────────────────────────────

    /// Log a command execution.
    pub fn log_command(
        &self,
        cmd: &str,
        output: Option<&str>,
        exit_code: Option<i32>,
        cwd: &str,
        duration_ms: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO history (session_id, command, output, exit_code, timestamp, directory, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.session_id,
                cmd,
                output,
                exit_code,
                Utc::now().timestamp(),
                cwd,
                duration_ms
            ],
        )?;
        Ok(())
    }

    /// Search history for commands matching the query.
    pub fn search_history(&self, query: &str) -> Result<Vec<CommandRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, command, output, exit_code, timestamp, directory, duration_ms
             FROM history
             WHERE command LIKE ?1
             ORDER BY timestamp DESC
             LIMIT 50",
        )?;

        let search_term = format!("%{}%", query);
        let rows = stmt.query_map(params![search_term], |row| {
            Ok(CommandRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                command: row.get(2)?,
                output: row.get(3)?,
                exit_code: row.get(4)?,
                timestamp: row.get(5)?,
                directory: row.get(6)?,
                duration_ms: row.get(7)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Get the last N unique commands (deduplicated, most recent first).
    pub fn recent_unique(&self, limit: usize) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT command FROM history
             GROUP BY command
             ORDER BY MAX(timestamp) DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| row.get::<_, String>(0))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Get top N most-used commands.
    pub fn top_commands(&self, limit: usize) -> Result<Vec<TopCommand>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT command, COUNT(*) as cnt FROM history
             GROUP BY command
             ORDER BY cnt DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(TopCommand {
                command: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Count commands in the current session.
    pub fn session_command_count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM history WHERE session_id = ?1",
            params![self.session_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get the last command's directory (best guess for CWD).
    pub fn last_directory(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT directory FROM history
             WHERE session_id = ?1
             ORDER BY timestamp DESC LIMIT 1",
            params![self.session_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(dir) => Ok(Some(dir)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ────────────────────────────────────────────────────────────────
    // Aliases
    // ────────────────────────────────────────────────────────────────

    /// Set (create or update) an alias.
    pub fn set_alias(&self, name: &str, expansion: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO aliases (name, expansion, created_at) VALUES (?1, ?2, ?3)",
            params![name, expansion, Utc::now().timestamp()],
        )?;
        Ok(())
    }

    /// Remove an alias.
    pub fn remove_alias(&self, name: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM aliases WHERE name = ?1",
            params![name],
        )?;
        Ok(affected > 0)
    }

    /// Get a specific alias expansion.
    pub fn get_alias(&self, name: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT expansion FROM aliases WHERE name = ?1",
            params![name],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(exp) => Ok(Some(exp)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// List all aliases.
    pub fn list_aliases(&self) -> Result<Vec<Alias>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, expansion, created_at FROM aliases ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Alias {
                name: row.get(0)?,
                expansion: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    // ────────────────────────────────────────────────────────────────
    // Bookmarks
    // ────────────────────────────────────────────────────────────────

    /// Add a bookmark.
    pub fn add_bookmark(&self, command: &str, label: Option<&str>) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO bookmarks (command, label, created_at) VALUES (?1, ?2, ?3)",
            params![command, label, Utc::now().timestamp()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Remove a bookmark by id.
    pub fn remove_bookmark(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "DELETE FROM bookmarks WHERE id = ?1",
            params![id],
        )?;
        Ok(affected > 0)
    }

    /// List all bookmarks.
    pub fn list_bookmarks(&self) -> Result<Vec<Bookmark>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, command, label, created_at FROM bookmarks ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                command: row.get(1)?,
                label: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    // ────────────────────────────────────────────────────────────────
    // Statistics
    // ────────────────────────────────────────────────────────────────

    /// Comprehensive stats about the vault.
    pub fn stats(&self) -> Result<VaultStats> {
        let conn = self.conn.lock().unwrap();

        let total_commands: i64 = conn.query_row(
            "SELECT COUNT(*) FROM history", [], |row| row.get(0),
        )?;

        let session_commands: i64 = conn.query_row(
            "SELECT COUNT(*) FROM history WHERE session_id = ?1",
            params![self.session_id],
            |row| row.get(0),
        )?;

        let total_sessions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM session", [], |row| row.get(0),
        )?;

        let unique_commands: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT command) FROM history", [], |row| row.get(0),
        )?;

        let alias_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM aliases", [], |row| row.get(0),
        )?;

        let bookmark_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0),
        )?;

        let earliest_timestamp: Option<i64> = conn.query_row(
            "SELECT MIN(timestamp) FROM history", [], |row| row.get(0),
        ).ok();

        // page_count * page_size gives approximate DB size
        let page_count: i64 = conn.query_row(
            "PRAGMA page_count", [], |row| row.get(0),
        ).unwrap_or(0);
        let page_size: i64 = conn.query_row(
            "PRAGMA page_size", [], |row| row.get(0),
        ).unwrap_or(4096);

        Ok(VaultStats {
            total_commands,
            session_commands,
            total_sessions,
            unique_commands,
            alias_count,
            bookmark_count,
            earliest_timestamp,
            db_size_bytes: page_count * page_size,
        })
    }

    // ────────────────────────────────────────────────────────────────
    // Export
    // ────────────────────────────────────────────────────────────────

    /// Export history as lines of text suitable for a shell history file.
    pub fn export_history(&self, limit: usize) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT command, timestamp FROM history ORDER BY timestamp ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let cmd: String = row.get(0)?;
            let ts: i64 = row.get(1)?;
            Ok(format!("# {}\n{}", ts, cmd))
        })?;
        let mut lines = Vec::new();
        for row in rows {
            lines.push(row?);
        }
        Ok(lines)
    }

    // ────────────────────────────────────────────────────────────────
    // Config (key-value settings)
    // ────────────────────────────────────────────────────────────────

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}