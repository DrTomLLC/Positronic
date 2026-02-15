//! CWD tracking and extraction tests.
//! Groups 7-8: CWD extraction from snapshots and cd command tracking.

use positronic_core::state_machine::{MyColor, Snapshot};
use positronic_bridge::cwd::{track_cd_command, update_cwd_from_snapshot, resolve_tilde};

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
// Group 7: CWD extraction from PTY snapshot
// ════════════════════════════════════════════════════════════════

#[test]
fn extract_cwd_powershell() {
    let snap = snapshot_with_lines(&["PS C:\\Users\\Doctor>"], 80, 24);
    let mut cwd = String::from("C:\\");
    update_cwd_from_snapshot(&snap, &mut cwd);
    assert_eq!(cwd, "C:\\Users\\Doctor");
}

#[test]
fn extract_cwd_powershell_with_space() {
    let snap = snapshot_with_lines(&["PS C:\\Program Files>"], 80, 24);
    let mut cwd = String::from("C:\\");
    update_cwd_from_snapshot(&snap, &mut cwd);
    assert_eq!(cwd, "C:\\Program Files");
}

#[test]
fn extract_cwd_unix_bash() {
    let snap = snapshot_with_lines(&["user@host:~/projects$"], 80, 24);
    let mut cwd = String::from("/home");
    update_cwd_from_snapshot(&snap, &mut cwd);
    // Should resolve ~ to home dir or use raw path
    assert!(cwd.contains("projects") || cwd.contains("~"));
}

#[test]
fn extract_cwd_cmd_exe() {
    let snap = snapshot_with_lines(&["C:\\Windows\\System32>"], 80, 24);
    let mut cwd = String::from("C:\\");
    update_cwd_from_snapshot(&snap, &mut cwd);
    assert_eq!(cwd, "C:\\Windows\\System32");
}

#[test]
fn extract_cwd_skips_empty_lines() {
    // Empty trailing lines should be skipped; prompt is on last non-empty line
    let snap = snapshot_with_lines(&["PS C:\\Dev>", ""], 80, 24);
    let mut cwd = String::from("C:\\");
    update_cwd_from_snapshot(&snap, &mut cwd);
    assert_eq!(cwd, "C:\\Dev");
}

#[test]
fn extract_cwd_empty_snapshot_no_change() {
    let snap = Snapshot::new(80, 24);
    let mut cwd = String::from("C:\\Original");
    update_cwd_from_snapshot(&snap, &mut cwd);
    assert_eq!(cwd, "C:\\Original");
}

// ════════════════════════════════════════════════════════════════
// Group 8: cd command tracking
// ════════════════════════════════════════════════════════════════

#[test]
fn track_cd_ignores_non_cd_commands() {
    let mut cwd = String::from("/home/user");
    track_cd_command("ls -la", &mut cwd);
    assert_eq!(cwd, "/home/user");
}

#[test]
fn track_cd_ignores_empty() {
    let mut cwd = String::from("/home/user");
    track_cd_command("", &mut cwd);
    assert_eq!(cwd, "/home/user");
}

#[test]
fn track_cd_recognizes_cd() {
    // cd with a non-existent path won't update, but the function should
    // not panic
    let mut cwd = String::from("/home/user");
    track_cd_command("cd /nonexistent_path_12345", &mut cwd);
    // cwd unchanged because path doesn't exist
    assert_eq!(cwd, "/home/user");
}

#[test]
fn track_cd_recognizes_pushd() {
    let mut cwd = String::from("/home/user");
    track_cd_command("pushd /nonexistent_path_12345", &mut cwd);
    assert_eq!(cwd, "/home/user"); // unchanged, path doesn't exist
}

#[test]
fn track_cd_recognizes_set_location() {
    let mut cwd = String::from("C:\\Users");
    track_cd_command("Set-Location C:\\nonexistent_12345", &mut cwd);
    assert_eq!(cwd, "C:\\Users"); // unchanged, path doesn't exist
}

#[test]
fn track_cd_ignores_special_dash() {
    let mut cwd = String::from("/home/user");
    track_cd_command("cd -", &mut cwd);
    // "-" is a special case left for PTY snapshot to resolve
    assert_eq!(cwd, "/home/user");
}

#[test]
fn track_cd_ignores_special_tilde() {
    let mut cwd = String::from("/home/user");
    track_cd_command("cd ~", &mut cwd);
    // "~" is a special case left for PTY snapshot to resolve
    assert_eq!(cwd, "/home/user");
}

#[test]
fn track_cd_bare_cd_no_args() {
    let mut cwd = String::from("/home/user");
    track_cd_command("cd", &mut cwd);
    // No args = no change
    assert_eq!(cwd, "/home/user");
}

// ════════════════════════════════════════════════════════════════
// Resolve tilde
// ════════════════════════════════════════════════════════════════

#[test]
fn resolve_tilde_no_tilde() {
    assert_eq!(resolve_tilde("/home/user"), "/home/user");
}

#[test]
fn resolve_tilde_bare_tilde() {
    let result = resolve_tilde("~");
    // Should either resolve to home dir or return "~" if HOME not set
    assert!(!result.is_empty());
}

#[test]
fn resolve_tilde_with_path() {
    let result = resolve_tilde("~/projects");
    // Should contain "projects" regardless of home resolution
    assert!(result.contains("projects"));
}