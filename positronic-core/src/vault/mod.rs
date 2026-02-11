use chrono::Utc;
use rusqlite::{Connection, Result, params};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub mod schema;

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
pub struct Vault {
    conn: Arc<Mutex<Connection>>,
    session_id: String,
}

impl Vault {
    /// Open the Vault at the specified path.
    /// Creates the database file if it doesn't exist.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrency
        conn.pragma_update(None, "journal_mode", "WAL")?;

        // Run migrations
        conn.execute_batch(schema::MIGRATION_INIT)?;

        let session_id = Uuid::new_v4().to_string();

        let vault = Self {
            conn: Arc::new(Mutex::new(conn)),
            session_id,
        };

        vault.start_session()?;

        Ok(vault)
    }

    fn start_session(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO session (id, start_time) VALUES (?1, ?2)",
            params![self.session_id, Utc::now().timestamp()],
        )?;
        Ok(())
    }

    /// Log a command execution to the Vault.
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
}
