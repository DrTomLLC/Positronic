// positronic-bridge/src/detection.rs
//
// Terminal mode detection.
//
// Scans the PTY snapshot to determine whether the terminal is in a
// pager (`-- More --`), continuation prompt (`>>`), normal prompt,
// or running state. Used to display mode indicators in the status bar
// and to inform the UI about when control signals are needed.

use positronic_core::state_machine::Snapshot;

/// Current terminal mode as detected from the PTY snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMode {
    /// Normal shell prompt is visible (PS path>, $, #, >).
    Normal,
    /// A pager is active (`-- More --`, `--More--`, `:`, `lines N-N`).
    Pager,
    /// A continuation/multi-line prompt is active (`>>`).
    Continuation,
    /// A command is running (no recognizable prompt).
    Running,
}

impl TerminalMode {
    /// Human-readable label for the status bar.
    pub fn label(self) -> &'static str {
        match self {
            TerminalMode::Normal => "",
            TerminalMode::Pager => "📖 PAGER",
            TerminalMode::Continuation => "⏎ CONTINUATION",
            TerminalMode::Running => "⏳ RUNNING",
        }
    }
}

/// Scan the snapshot from the bottom to detect the current terminal mode.
///
/// Heuristics (checked in order):
/// 1. `-- More --` or `--More--` or bare `:` at end of line → Pager
/// 2. `lines N-N` → Pager (less/more style)
/// 3. `>>` as the entire prompt → Continuation
/// 4. `PS path>` or line ending with `$`, `#`, `>` → Normal
/// 5. Anything else → Running
pub fn detect_terminal_mode(snapshot: &Snapshot) -> TerminalMode {
    let rows = snapshot.rows();
    if rows == 0 {
        return TerminalMode::Running;
    }

    // Find the last non-empty line (scanning from bottom)
    for row_idx in (0..rows).rev() {
        let row = &snapshot[row_idx];
        let line: String = row.iter().map(|(c, _)| *c).collect();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // ── Pager patterns ──
        if trimmed.contains("-- More --")
            || trimmed.contains("--More--")
            || trimmed == ":"
            || trimmed.starts_with("lines ")
            && trimmed.contains('-')
            && trimmed.chars().filter(|c| c.is_ascii_digit()).count() > 0
        {
            return TerminalMode::Pager;
        }

        // ── Continuation prompt ──
        if trimmed == ">>" || trimmed.ends_with(">> ") {
            return TerminalMode::Continuation;
        }

        // ── Normal prompt patterns ──
        // PowerShell: PS C:\path>
        if trimmed.starts_with("PS ") && trimmed.contains('>') {
            return TerminalMode::Normal;
        }
        // Unix: ends with $ or # (common prompt terminators)
        if trimmed.ends_with('$') || trimmed.ends_with("$ ") {
            return TerminalMode::Normal;
        }
        if trimmed.ends_with('#') || trimmed.ends_with("# ") {
            return TerminalMode::Normal;
        }
        // cmd.exe: C:\path>
        if trimmed.ends_with('>') && trimmed.len() >= 3 {
            let before = &trimmed[..trimmed.len() - 1];
            if before.len() >= 2 && before.as_bytes()[1] == b':' {
                return TerminalMode::Normal;
            }
        }

        // Last non-empty line doesn't match any known prompt
        return TerminalMode::Running;
    }

    TerminalMode::Running
}