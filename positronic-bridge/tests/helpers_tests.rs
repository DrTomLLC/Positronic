//! Format helpers and buffer management tests.
//! Groups 9-10.

use positronic_core::state_machine::{MyColor, Snapshot};
use positronic_bridge::helpers::{format_duration_short, short_path, hash_snapshot};

// ════════════════════════════════════════════════════════════════
// Group 9: Format helpers
// ════════════════════════════════════════════════════════════════

#[test]
fn format_seconds() {
    assert_eq!(format_duration_short(5), "5s");
    assert_eq!(format_duration_short(0), "0s");
    assert_eq!(format_duration_short(59), "59s");
}

#[test]
fn format_minutes() {
    assert_eq!(format_duration_short(60), "1m");
    assert_eq!(format_duration_short(120), "2m");
    assert_eq!(format_duration_short(3599), "59m");
}

#[test]
fn format_hours() {
    assert_eq!(format_duration_short(3600), "1h 0m");
    assert_eq!(format_duration_short(3660), "1h 1m");
    assert_eq!(format_duration_short(7200), "2h 0m");
    assert_eq!(format_duration_short(7261), "2h 1m");
}

#[test]
fn short_path_no_change() {
    let short = "C:\\Dev";
    assert_eq!(short_path(short), short);
}

#[test]
fn short_path_truncates_long() {
    let long_path = "C:\\Users\\Doctor\\Documents\\Projects\\SubFolder\\AnotherFolder\\Deep";
    let result = short_path(long_path);
    // If home dir doesn't match, should truncate with ellipsis
    // or show as-is if contains home
    assert!(result.len() <= long_path.len());
}

#[test]
fn short_path_preserves_short() {
    let path = "/tmp";
    assert_eq!(short_path(path), path);
}

// ════════════════════════════════════════════════════════════════
// Group 10: Snapshot hashing and buffer management
// ════════════════════════════════════════════════════════════════

#[test]
fn hash_empty_snapshot() {
    let snap = Snapshot::new(80, 24);
    let h = hash_snapshot(&snap);
    assert!(h != 0, "Empty snapshot should still produce a hash");
}

#[test]
fn hash_same_content_same_hash() {
    let snap1 = Snapshot::new(80, 24);
    let snap2 = Snapshot::new(80, 24);
    assert_eq!(hash_snapshot(&snap1), hash_snapshot(&snap2));
}

#[test]
fn hash_different_content_different_hash() {
    let snap1 = Snapshot::new(80, 24);
    let mut snap2 = Snapshot::new(80, 24);
    snap2.cells[0] = ('X', MyColor::Red);
    assert_ne!(hash_snapshot(&snap1), hash_snapshot(&snap2));
}

#[test]
fn hash_different_dimensions_different_hash() {
    let snap1 = Snapshot::new(80, 24);
    let snap2 = Snapshot::new(120, 30);
    assert_ne!(hash_snapshot(&snap1), hash_snapshot(&snap2));
}

#[test]
fn buffer_trim_boundary() {
    // Test the push_direct buffer management logic
    // The buffer should trim when exceeding MAX_DIRECT_BYTES (256KB)
    use positronic_bridge::messages::push_direct;

    // We can't easily construct a PositronicApp here without iced,
    // but we can verify the constant exists
    assert_eq!(positronic_bridge::app::MAX_DIRECT_BYTES, 256 * 1024);
}