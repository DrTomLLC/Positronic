//! Tab Completion Engine
//!
//! Provides context-aware completions for:
//! - ! commands (e.g., !hi → !history, !help, !hive)
//! - Sub-commands (e.g., !alias s → !alias set)
//! - Theme names (e.g., !theme c → !theme cyberpunk)
//! - File/directory paths (e.g., cd Dow → cd Downloads)
//! - Alias names

use std::path::Path;

/// Known top-level ! commands.
const BANG_COMMANDS: &[&str] = &[
    "ai", "alias", "ask", "bm", "bookmark", "clear", "debug",
    "explain", "export", "fix", "get", "help", "history", "hive",
    "io", "pwd", "run", "set", "stats", "suggest", "theme", "top",
    "ver", "version", "wasm",
];

/// Sub-commands for specific ! commands.
fn subcommands_for(cmd: &str) -> &'static [&'static str] {
    match cmd {
        "alias" => &["set", "rm", "list"],
        "bm" | "bookmark" => &["add", "rm"],
        "hive" => &["scan", "status"],
        "io" => &["scan", "list", "connect"],
        _ => &[],
    }
}

/// Available theme names for !theme completion.
const THEME_NAMES: &[&str] = &["default", "cyberpunk", "solarized", "monokai"];

/// Completion state that tracks cycling through results.
#[derive(Debug, Clone)]
pub struct CompletionState {
    /// The original input text when Tab was first pressed.
    pub original: String,
    /// All matching completions.
    pub completions: Vec<String>,
    /// Current index into completions (cycles on repeated Tab).
    pub index: usize,
}

impl CompletionState {
    /// Advance to the next completion, cycling around.
    pub fn next(&mut self) -> &str {
        if self.completions.is_empty() {
            return &self.original;
        }
        self.index = (self.index + 1) % self.completions.len();
        &self.completions[self.index]
    }

    /// Get the current completion.
    pub fn current(&self) -> &str {
        if self.completions.is_empty() {
            &self.original
        } else {
            &self.completions[self.index]
        }
    }

    /// How many completions are available.
    pub fn len(&self) -> usize {
        self.completions.len()
    }
}

/// Generate completions for the given input.
/// `aliases` should be a list of known alias names.
/// `cwd` is the current working directory for path completion.
pub fn complete(input: &str, aliases: &[String], cwd: &str) -> Option<CompletionState> {
    let trimmed = input.trim_start();

    if trimmed.is_empty() {
        return None;
    }

    // ── ! command completion ──
    if trimmed.starts_with('!') {
        return complete_bang(trimmed);
    }

    // ── Path completion for shell commands ──
    // If the input has spaces, try to complete the last token as a path
    if let Some(last_space) = trimmed.rfind(' ') {
        let prefix = &trimmed[..=last_space];
        let partial = &trimmed[last_space + 1..];
        if !partial.is_empty() {
            if let Some(state) = complete_path(partial, prefix, cwd) {
                return Some(state);
            }
        }
    } else {
        // Single token — could be an alias or a command/path
        // Try alias completion first
        if !aliases.is_empty() {
            let matches: Vec<String> = aliases
                .iter()
                .filter(|a| a.starts_with(trimmed))
                .cloned()
                .collect();
            if !matches.is_empty() && !(matches.len() == 1 && matches[0] == trimmed) {
                return Some(CompletionState {
                    original: input.to_string(),
                    completions: matches,
                    index: 0,
                });
            }
        }

        // Try path/command completion from CWD
        if let Some(state) = complete_path(trimmed, "", cwd) {
            return Some(state);
        }
    }

    None
}

/// Complete ! commands and their sub-commands.
fn complete_bang(input: &str) -> Option<CompletionState> {
    let without_bang = &input[1..]; // strip leading !
    let parts: Vec<&str> = without_bang.splitn(2, ' ').collect();

    if parts.len() == 1 {
        // Completing the command name: !his → !history
        let partial = parts[0].to_lowercase();
        let matches: Vec<String> = BANG_COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(&partial))
            .map(|cmd| format!("!{}", cmd))
            .collect();

        if matches.is_empty() || (matches.len() == 1 && matches[0] == input) {
            return None;
        }

        Some(CompletionState {
            original: input.to_string(),
            completions: matches,
            index: 0,
        })
    } else {
        // Completing a sub-command or argument: !alias s → !alias set
        let cmd = parts[0];
        let arg_partial = parts[1].trim();

        // Special case: !theme <name>
        if cmd == "theme" {
            let matches: Vec<String> = THEME_NAMES
                .iter()
                .filter(|t| t.starts_with(&arg_partial.to_lowercase()))
                .map(|t| format!("!theme {}", t))
                .collect();
            if !matches.is_empty() && !(matches.len() == 1 && matches[0] == input) {
                return Some(CompletionState {
                    original: input.to_string(),
                    completions: matches,
                    index: 0,
                });
            }
            return None;
        }

        // Sub-command completion
        let subs = subcommands_for(cmd);
        if !subs.is_empty() {
            let matches: Vec<String> = subs
                .iter()
                .filter(|s| s.starts_with(&arg_partial.to_lowercase()))
                .map(|s| format!("!{} {}", cmd, s))
                .collect();
            if !matches.is_empty() && !(matches.len() == 1 && matches[0] == input) {
                return Some(CompletionState {
                    original: input.to_string(),
                    completions: matches,
                    index: 0,
                });
            }
        }

        None
    }
}

/// Complete a file/directory path relative to CWD.
/// `partial` is the fragment being completed, `prefix` is everything before it.
fn complete_path(partial: &str, prefix: &str, cwd: &str) -> Option<CompletionState> {
    // Determine the directory to search and the name fragment
    let (search_dir, name_fragment) = if partial.contains('/') || partial.contains('\\') {
        // Has path separator — split into dir + fragment
        let sep_idx = partial.rfind(|c: char| c == '/' || c == '\\').unwrap();
        let dir_part = &partial[..=sep_idx];
        let name_part = &partial[sep_idx + 1..];

        let full_dir = if Path::new(dir_part).is_absolute() {
            dir_part.to_string()
        } else {
            format!("{}/{}", cwd, dir_part)
        };
        (full_dir, name_part.to_string())
    } else {
        // No separator — search CWD
        (cwd.to_string(), partial.to_string())
    };

    // Read directory entries
    let entries = match std::fs::read_dir(&search_dir) {
        Ok(entries) => entries,
        Err(_) => return None,
    };

    let name_lower = name_fragment.to_lowercase();
    let mut matches: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Skip hidden files unless the user is explicitly typing a dot
        if name.starts_with('.') && !name_fragment.starts_with('.') {
            continue;
        }

        if name.to_lowercase().starts_with(&name_lower) {
            // Reconstruct the full input with this completion
            let completed_name = if partial.contains('/') || partial.contains('\\') {
                let sep_idx = partial.rfind(|c: char| c == '/' || c == '\\').unwrap();
                let dir_part = &partial[..=sep_idx];
                format!("{}{}", dir_part, name)
            } else {
                name.to_string()
            };

            // Add trailing separator for directories
            let display = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if cfg!(windows) {
                    format!("{}{}\\", prefix, completed_name)
                } else {
                    format!("{}{}/", prefix, completed_name)
                }
            } else {
                format!("{}{}", prefix, completed_name)
            };

            matches.push(display);
        }
    }

    if matches.is_empty() {
        return None;
    }

    // Sort: directories first, then alphabetical
    matches.sort_by(|a, b| {
        let a_dir = a.ends_with('/') || a.ends_with('\\');
        let b_dir = b.ends_with('/') || b.ends_with('\\');
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.to_lowercase().cmp(&b.to_lowercase()),
        }
    });

    // Don't return if the only match equals the input
    if matches.len() == 1 && matches[0].trim() == partial {
        return None;
    }

    let original = format!("{}{}", prefix, partial);

    Some(CompletionState {
        original,
        completions: matches,
        index: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bang_command_completion() {
        let state = complete("!hi", &[], ".").unwrap();
        assert!(state.completions.contains(&"!history".to_string()));
        assert!(state.completions.contains(&"!hive".to_string()));
        // "help" starts with "he", not "hi" — must NOT match
        assert!(!state.completions.contains(&"!help".to_string()));
        assert_eq!(state.completions.len(), 2);
    }

    #[test]
    fn test_bang_exact_no_completion() {
        // Exact match — nothing to complete
        let result = complete("!help", &[], ".");
        assert!(result.is_none());
    }

    #[test]
    fn test_alias_subcommand_completion() {
        let state = complete("!alias s", &[], ".").unwrap();
        assert!(state.completions.contains(&"!alias set".to_string()));
    }

    #[test]
    fn test_theme_completion() {
        let state = complete("!theme c", &[], ".").unwrap();
        assert!(state.completions.contains(&"!theme cyberpunk".to_string()));
    }

    #[test]
    fn test_empty_input() {
        assert!(complete("", &[], ".").is_none());
    }

    #[test]
    fn test_cycling() {
        let mut state = complete("!hi", &[], ".").unwrap();
        let count = state.len();
        assert!(count >= 2); // help, history, hive
        let first = state.current().to_string();
        let next = state.next().to_string();
        assert_ne!(first, next);
    }
}