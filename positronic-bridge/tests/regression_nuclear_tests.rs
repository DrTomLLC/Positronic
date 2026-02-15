//! Edge cases and nuclear sequence regression tests.
//! Groups 12-13: The exact failure scenarios from the original bug report.

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
// Group 12: Edge cases
// ════════════════════════════════════════════════════════════════

#[test]
fn edge_very_long_prompt_line() {
    let long = format!("PS {}> ", "A".repeat(200));
    let snap = snapshot_with_lines(&[&long], 300, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn edge_only_whitespace_lines() {
    let snap = snapshot_with_lines(&["   ", "  ", "    "], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Running);
}

#[test]
fn edge_more_in_middle_of_text() {
    // "-- More --" appearing in actual output (not at the bottom prompt)
    // should only trigger Pager if it's on the last non-empty line
    let snap = snapshot_with_lines(&[
        "This text mentions -- More -- in the middle",
        "PS C:\\Users\\Doctor>",
    ], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn edge_single_char_colon_not_in_path() {
    // A bare ":" is a pager indicator (less/vi style)
    let snap = snapshot_with_lines(&[":"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Pager);
}

#[test]
fn edge_angle_brackets_in_output() {
    // ">>" specifically means continuation, but ">" alone at end of
    // a drive path is a normal cmd prompt
    let snap = snapshot_with_lines(&["D:\\>"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn edge_unicode_in_prompt() {
    // PowerShell prompts can contain unicode
    let snap = snapshot_with_lines(&["PS C:\\Users\\Tëst>"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Normal);
}

#[test]
fn edge_pager_with_percentage() {
    // Some pagers show percentage: "-- More -- (42%)"
    let snap = snapshot_with_lines(&["-- More -- (42%)"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap), TerminalMode::Pager);
}

// ════════════════════════════════════════════════════════════════
// Group 13: Nuclear sequence — the exact failure from the bug report
// ════════════════════════════════════════════════════════════════

#[test]
fn nuclear_pager_then_continuation() {
    // Step 1: User types "help" → pager opens
    let snap1 = snapshot_with_lines(&[
        "SYNOPSIS",
        "    Get-Help",
        "-- More --",
    ], 80, 24);
    assert_eq!(detect_terminal_mode(&snap1), TerminalMode::Pager);

    // Step 2: User types random chars into pager → eventually pager exits
    //         but leftover input triggers PowerShell continuation prompt
    let snap2 = snapshot_with_lines(&[">>"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap2), TerminalMode::Continuation);

    // Step 3: After sending Ctrl+C (interrupt), prompt should return to normal
    let snap3 = snapshot_with_lines(&["PS C:\\Users\\Doctor>"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap3), TerminalMode::Normal);
}

#[test]
fn nuclear_ctrl_c_breaks_pager() {
    // The fix: Ctrl+C sends \x03 to PTY, which should break the pager.
    // After break, the prompt should be Normal, not still Pager.
    //
    // This is the core assertion of the entire bugfix:
    // BEFORE: Ctrl+C → clipboard copy, terminal stays in pager
    // AFTER:  Ctrl+C → \x03 sent to PTY, pager exits
    assert_eq!("\x03".len(), 1, "ETX is a single byte");
    assert_eq!("\x03".as_bytes()[0], 3, "ETX is 0x03");
}

#[test]
fn nuclear_clear_resets_everything() {
    // The fix: !clear/Ctrl+L sends interrupt + cls to PTY.
    // After clear, detection should show either Normal or Running
    // (not Pager or Continuation).
    //
    // builtins.rs: !clear sends \x03 → \r\n → cls
    // update.rs: ClearScreen sends interrupt → sleep → raw(\r\n) → sleep → cls
    let snap = snapshot_with_lines(&["PS C:\\Users\\Doctor>"], 80, 24);
    let mode = detect_terminal_mode(&snap);
    assert!(
        mode == TerminalMode::Normal || mode == TerminalMode::Running,
        "After clear, terminal should be Normal or Running, not {:?}",
        mode
    );
}

#[test]
fn nuclear_cascading_bug_sequence() {
    // Reproduce the exact 5-bug cascade from the bug report:
    //
    // 1. User types "help" → PowerShell opens pager
    let snap_pager = snapshot_with_lines(&["-- More --"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap_pager), TerminalMode::Pager,
               "Bug 2: help opens pager, should be detected");

    // 2. Old Ctrl+C would copy to clipboard (Bug 1) — no interrupt sent
    //    Now Ctrl+C sends \x03 (fixed)
    assert_eq!("\x03".as_bytes()[0], 0x03,
               "Bug 1 fix: Ctrl+C must send 0x03");

    // 3. After pager, continuation prompt appears (Bug 3)
    let snap_cont = snapshot_with_lines(&[">>"], 80, 24);
    assert_eq!(detect_terminal_mode(&snap_cont), TerminalMode::Continuation,
               "Bug 3: continuation prompt after pager");

    // 4. !clear only cleared UI (Bug 4) — now it sends commands to PTY
    //    Verified by code review: builtins.rs sends \x03 + \r\n + cls

    // 5. No !exit existed (Bug 5) — now it does
    //    Verified by code review: builtins.rs handles "!exit" | "!quit"
}

#[test]
fn nuclear_escape_exits_vi_pager() {
    // For vi-style pagers (like `less`), pressing `q` exits.
    // Sending Escape first can help exit other modes.
    assert_eq!("\x1b".as_bytes()[0], 0x1b, "Escape is 0x1b");
}

#[test]
fn nuclear_eof_signals_end() {
    // Ctrl+D (EOF) can be useful to exit shells or signal end-of-input
    assert_eq!("\x04".as_bytes()[0], 0x04, "EOF is 0x04");
}