//! Block recorder.
//!
//! Inputs:
//! - PTY output bytes (to accumulate output)
//! - OSC events (cwd + prompt boundaries)
//! - "command sent" (the actual command string we wrote to the PTY)
//!
//! Output:
//! - finalized TerminalBlockV2
//! - persisted to Vault (history table)
//!
//! This file intentionally keeps `eprintln!` debug lines for deep tracing.

use chrono::Utc;

use crate::term::{OscEvent, OscParser, SemanticEvent, SemanticState};
use crate::vault::Vault;

use super::model::TerminalBlockV2;

#[derive(Debug, Clone)]
pub enum RecorderEvent {
    BlockStarted,
    BlockFinished(TerminalBlockV2),
}

#[derive(Debug)]
pub struct BlockRecorder {
    vault: Vault,

    osc: OscParser,
    sem: SemanticState,

    in_flight: Option<TerminalBlockV2>,
    pending_command: Option<String>,
}

impl BlockRecorder {
    pub fn new(vault: Vault) -> Self {
        Self {
            vault,
            osc: OscParser::new(),
            sem: SemanticState::new(),
            in_flight: None,
            pending_command: None,
        }
    }

    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    /// Call when Runner sends a command to PTY (after alias expansion).
    pub fn on_command_sent(&mut self, cmd: &str) {
        eprintln!("[REC] command_sent: {:?}", cmd);
        self.pending_command = Some(cmd.to_string());
    }

    /// Feed PTY output bytes. This:
    /// - parses OSC sequences to keep semantic state current
    /// - appends output to the current in-flight block (if any)
    pub fn on_pty_output(&mut self, bytes: &[u8], out: &mut Vec<RecorderEvent>) {
        // 1) Parse OSC events
        let mut osc_events = Vec::new();
        self.osc.feed(bytes, &mut osc_events);

        for ev in &osc_events {
            if let Some(sem_ev) = self.sem.apply_osc(ev) {
                self.on_semantic_event(sem_ev, out);
            }
        }

        // 2) Append bytes to in-flight output (lossy OK; terminal snapshot holds truth)
        if let Some(block) = &mut self.in_flight {
            let text = String::from_utf8_lossy(bytes);
            block.output.push_str(&text);
        }
    }

    fn on_semantic_event(&mut self, ev: SemanticEvent, out: &mut Vec<RecorderEvent>) {
        match ev {
            SemanticEvent::CwdChanged(cwd) => {
                if let Some(b) = &mut self.in_flight {
                    b.cwd = Some(cwd);
                }
            }
            SemanticEvent::CommandStart => {
                // Start a new block using the most recent command_sent
                let cmd = self
                    .pending_command
                    .take()
                    .unwrap_or_else(|| "<unknown>".to_string());

                let now = Utc::now().timestamp();
                let mut block = TerminalBlockV2::new_now(cmd, now);
                block.cwd = self.sem.cwd.clone();
                self.in_flight = Some(block);
                eprintln!("[REC] BlockStarted");
                out.push(RecorderEvent::BlockStarted);
            }
            SemanticEvent::CommandEnd { exit_code } => {
                if let Some(mut block) = self.in_flight.take() {
                    let end = Utc::now().timestamp();
                    block.ended_at_unix = end;
                    block.exit_code = exit_code;
                    let dur = (end - block.started_at_unix) * 1000;
                    block.duration_ms = Some(dur);

                    // Persist to vault
                    let cwd = block.cwd.clone().unwrap_or_else(|| ".".to_string());
                    let _ = self.vault.log_command(
                        &block.command,
                        Some(&block.output),
                        block.exit_code,
                        &cwd,
                        block.duration_ms,
                    );

                    eprintln!(
                        "[REC] BlockFinished cmd={:?} exit={:?} bytes={}",
                        block.command,
                        block.exit_code,
                        block.output.len()
                    );

                    out.push(RecorderEvent::BlockFinished(block));
                }
            }
            _ => {}
        }
    }
}
