//! Shared utility functions.
//!
//! Zero UI dependencies. Used by both shell and ui modules.

use positronic_core::state_machine::Snapshot;
use positronic_core::PositronicEngine;

use std::hash::{Hash, Hasher};

// ────────────────────────────────────────────────────────────────
// Snapshot hashing
// ────────────────────────────────────────────────────────────────

pub fn hash_snapshot(snapshot: &Snapshot) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    snapshot.rows().hash(&mut h);
    snapshot.cols().hash(&mut h);
    for row in snapshot.into_iter() {
        for (c, _) in row {
            c.hash(&mut h);
        }
    }
    h.finish()
}

// ────────────────────────────────────────────────────────────────
// Formatting
// ────────────────────────────────────────────────────────────────

pub fn format_duration_short(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    }
}

/// Shorten a path for status bar display.
pub fn short_path(path: &str) -> String {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        if let Some(rest) = path.strip_prefix(&home) {
            return format!("~{}", rest);
        }
    }
    if path.len() > 40 {
        return format!("…{}", &path[path.len() - 35..]);
    }
    path.to_string()
}

// ────────────────────────────────────────────────────────────────
// Alias helpers
// ────────────────────────────────────────────────────────────────

/// Retrieve alias names from the vault (for tab completion).
/// Takes an optional engine reference — no UI dependency.
pub fn get_alias_names_from(engine: Option<&PositronicEngine>) -> Vec<String> {
    let Some(engine) = engine else {
        return vec![];
    };
    match engine.runner.vault().list_aliases() {
        Ok(aliases) => aliases.into_iter().map(|a| a.name).collect(),
        Err(_) => vec![],
    }
}