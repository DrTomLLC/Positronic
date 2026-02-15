//! Pager, Continuation, and Normal prompt detection tests.
//! Groups 1-3 from the original test suite.

use positronic_core::state_machine::{MyColor, Snapshot};
use positronic_bridge::detection::{detect_terminal_mode, TerminalMode};

/// Helper: build a Snapshot with the given lines of text at the bottom.
fn snapshot_with_lines(lines: &[&str], cols: usize, rows: usize) -> Snapshot {
    let mut snap = Snapshot::new(cols, rows);
    let start_row = rows.saturating_sub(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let row = start_row + i;
        for (col, ch) in line.chars().enumerate() {
            if col < cols {
                snap.cells[row * cols + col] = (ch, MyColor::Default);
            }
        }
    }
    snap
}

// ════════════════════════════════════════════════════════════════
// Group 1: Pager detection
// ════════════════════════════════════════════════════════════════

#[test]
fn pager_more_double_dash() {
    let snap = snapshot_with_lines(&["-- More --"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Pager);
}

#[test]
fn pager_more_no_spaces() {
    let snap = snapshot_with_lines(&["--More--"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Pager);
}

#[test]
fn pager_colon_prompt() {
    let snap = snapshot_with_lines(&[":"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Pager);
}

#[test]
fn pager_with_content_above() {
    let snap = snapshot_with_lines(&[
        "SYNOPSIS",
        "    Get-Help",
        "-- More --",
    ], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Pager);
}

#[test]
fn pager_lines_indicator() {
    let snap = snapshot_with_lines(&["lines 1-24"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Pager);
}

// ════════════════════════════════════════════════════════════════
// Group 2: Continuation prompt detection
// ════════════════════════════════════════════════════════════════

#[test]
fn continuation_double_angle() {
    let snap = snapshot_with_lines(&[">>"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Continuation);
}

#[test]
fn continuation_with_trailing_space() {
    let snap = snapshot_with_lines(&[">> "], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Continuation);
}

#[test]
fn continuation_after_partial_input() {
    let snap = snapshot_with_lines(&[
        "PS C:\\Users\\Doctor> echo \"hello",
        ">>",
    ], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Continuation);
}

// ════════════════════════════════════════════════════════════════
// Group 3: Normal prompt detection
// ════════════════════════════════════════════════════════════════

#[test]
fn normal_powershell_prompt() {
    let snap = snapshot_with_lines(&["PS C:\\Users\\Doctor>"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn normal_powershell_with_space() {
    let snap = snapshot_with_lines(&["PS C:\\Users\\Doctor> "], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn normal_unix_dollar() {
    let snap = snapshot_with_lines(&["user@host:~/project$"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn normal_unix_hash() {
    let snap = snapshot_with_lines(&["root@host:/etc#"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn normal_cmd_prompt() {
    let snap = snapshot_with_lines(&["C:\\Windows\\System32>"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn running_no_prompt() {
    let snap = snapshot_with_lines(&["Building project... 45%"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Running);
}

#[test]
fn running_empty_snapshot() {
    let snap = Snapshot::new(80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Running);
}

#[test]
fn mode_label_normal_is_empty() {
    assert_eq!(TerminalMode::Normal.label(), "");
}

#[test]
fn mode_label_pager_has_text() {
    assert!(!TerminalMode::Pager.label().is_empty());
}

#[test]
fn mode_label_continuation_has_text() {
    assert!(!TerminalMode::Continuation.label().is_empty());
}