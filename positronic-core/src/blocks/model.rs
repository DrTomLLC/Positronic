//! Terminal block model (V2).
//!
//! This complements the existing `TerminalBlock` in lib.rs.
//! V2 adds cwd + timing + stable UUID ids.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type BlockId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBlockV2 {
    pub id: BlockId,
    pub command: String,
    pub output: String,
    pub exit_code: Option<i32>,
    pub cwd: Option<String>,
    pub started_at_unix: i64,
    pub ended_at_unix: i64,
    pub duration_ms: Option<i64>,
}

impl TerminalBlockV2 {
    pub fn new_now(command: impl Into<String>, started_at_unix: i64) -> Self {
        Self {
            id: Uuid::new_v4(),
            command: command.into(),
            output: String::new(),
            exit_code: None,
            cwd: None,
            started_at_unix,
            ended_at_unix: started_at_unix,
            duration_ms: None,
        }
    }
}
