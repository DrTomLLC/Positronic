//! Current Working Directory tracking.
//!
//! Two strategies:
//! 1. **Proactive**: Parse `cd`/`pushd`/`Set-Location` commands before sending to PTY.
//! 2. **Reactive**: Parse the PTY snapshot prompt line for common shell prompt patterns.

use positronic_core::state_machine::Snapshot;

/// Track `cd` / `pushd` / `Set-Location` commands to update CWD proactively.
pub fn track_cd_command(cmd: &str, cwd: &mut String) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    let is_cd = matches!(
        parts[0].to_lowercase().as_str(),
        "cd" | "chdir" | "pushd" | "set-location" | "sl"
    );

    if !is_cd || parts.len() < 2 {
        return;
    }

    let target = parts[1..].join(" ");
    // Strip surrounding quotes
    let target = target.trim_matches('"').trim_matches('\'');

    if target == "-" || target == "~" {
        // Special cases — we can't resolve these without the shell,
        // so we'll let the PTY snapshot update pick it up.
        return;
    }

    let path = std::path::Path::new(target);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::Path::new(cwd.as_str()).join(path)
    };

    // Only update if the directory actually exists
    if let Ok(canonical) = resolved.canonicalize() {
        *cwd = canonical.to_string_lossy().to_string();
    }
}

/// Extract CWD from the PTY snapshot by looking for common prompt patterns.
///
/// Supported patterns:
///   - `PS C:\path>` (PowerShell)
///   - `user@host:~/path$` or `~/path$` (bash/zsh)
///   - `C:\path>` (cmd.exe)
pub fn update_cwd_from_snapshot(snapshot: &Snapshot, cwd: &mut String) {
    let rows = snapshot.rows();
    if rows == 0 {
        return;
    }

    // Scan from the bottom for the first non-empty line (the prompt)
    for row_idx in (0..rows).rev() {
        let row = &snapshot[row_idx];
        let line: String = row.iter().map(|(c, _)| *c).collect();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // ── PowerShell: PS C:\path> ──
        if let Some(rest) = trimmed.strip_prefix("PS ") {
            if let Some(path) = rest.strip_suffix('>').or_else(|| {
                rest.split('>').next()
            }) {
                let path = path.trim();
                if !path.is_empty()
                    && (path.contains('\\') || path.contains('/') || path.starts_with('~'))
                {
                    let resolved = resolve_tilde(path);
                    if std::path::Path::new(&resolved).is_dir()
                        || resolved.contains('\\')
                        || resolved.contains('/')
                    {
                        *cwd = resolved;
                    }
                }
            }
            return;
        }

        // ── Unix bash/zsh: user@host:~/path$ or ~/path$ ──
        if let Some(colon_pos) = trimmed.find(':') {
            let after_colon = &trimmed[colon_pos + 1..];
            if let Some(prompt_end) = after_colon.rfind(|c: char| c == '$' || c == '#') {
                let path = after_colon[..prompt_end].trim();
                if !path.is_empty() {
                    let resolved = resolve_tilde(path);
                    *cwd = resolved;
                    return;
                }
            }
        }

        // ── cmd.exe: C:\path> ──
        if trimmed.ends_with('>') && trimmed.len() >= 3 {
            let path = &trimmed[..trimmed.len() - 1];
            if path.len() >= 2 && path.as_bytes()[1] == b':' {
                *cwd = path.to_string();
            }
        }

        // Only check the last non-empty line (the prompt)
        return;
    }
}

/// Replace leading `~` with the home directory.
pub fn resolve_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            return format!("{}{}", home, &path[1..]);
        }
    }
    path.to_string()
}