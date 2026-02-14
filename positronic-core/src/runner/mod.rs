//! # Reflex Engine
//!
//! The "Instinct" Layer (Tier 3) - Heuristic command correction.
//! Uses Levenshtein distance and pattern matching for common typos.
//! Zero-ML fallback that works without NPU or network connectivity.
//!
//! Eventually this will also wrap `ort` (ONNX Runtime) to run a
//! quantized SLM locally for Tier 2 inference.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Known command corrections: common typos -> correct commands
static KNOWN_TYPOS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

/// Common shell commands used for Levenshtein matching
static COMMON_COMMANDS: OnceLock<Vec<&'static str>> = OnceLock::new();

fn known_typos() -> &'static HashMap<&'static str, &'static str> {
    KNOWN_TYPOS.get_or_init(|| {
        let mut m = HashMap::new();
        // Git typos
        m.insert("git psuh", "git push");
        m.insert("git pul", "git pull");
        m.insert("git comit", "git commit");
        m.insert("git commti", "git commit");
        m.insert("git sttaus", "git status");
        m.insert("git statis", "git status");
        m.insert("git stauts", "git status");
        m.insert("git chekout", "git checkout");
        m.insert("git chekcout", "git checkout");
        m.insert("git brnach", "git branch");
        m.insert("git branh", "git branch");
        m.insert("git marge", "git merge");
        m.insert("git staus", "git status");
        m.insert("git ad", "git add");
        m.insert("git dif", "git diff");
        m.insert("git lgo", "git log");
        m.insert("git fetc", "git fetch");
        m.insert("git rbase", "git rebase");
        m.insert("git reste", "git reset");
        // Cargo typos
        m.insert("carog build", "cargo build");
        m.insert("cargo biuld", "cargo build");
        m.insert("cargo buld", "cargo build");
        m.insert("cargo tset", "cargo test");
        m.insert("cargo tes", "cargo test");
        m.insert("cargo rn", "cargo run");
        m.insert("cargo chesk", "cargo check");
        m.insert("cargo clipppy", "cargo clippy");
        m.insert("crago", "cargo");
        // Common CLI typos
        m.insert("cta", "cat");
        m.insert("sl", "ls");
        m.insert("pyhton", "python");
        m.insert("pytohn", "python");
        m.insert("pyton", "python");
        m.insert("ndoe", "node");
        m.insert("noed", "node");
        m.insert("dokcer", "docker");
        m.insert("dcoker", "docker");
        m.insert("kuebctl", "kubectl");
        m.insert("kubeclt", "kubectl");
        m.insert("mkidr", "mkdir");
        m.insert("mdkir", "mkdir");
        m.insert("claer", "clear");
        m.insert("cealr", "clear");
        m.insert("grpe", "grep");
        m.insert("gerp", "grep");
        m.insert("les", "less");
        m.insert("mroe", "more");
        m.insert("tial", "tail");
        m.insert("ehco", "echo");
        m.insert("ecoh", "echo");
        m.insert("sudp", "sudo");
        m.insert("suod", "sudo");
        m.insert("cd..", "cd ..");
        m.insert("cd...", "cd ../..");
        m
    })
}

fn common_commands() -> &'static Vec<&'static str> {
    COMMON_COMMANDS.get_or_init(|| {
        vec![
            "git", "cargo", "rustup", "npm", "node", "python", "pip", "docker", "kubectl", "ls",
            "cd", "cat", "grep", "find", "mkdir", "rmdir", "rm", "cp", "mv", "touch", "chmod",
            "chown", "echo", "less", "more", "head", "tail", "sort", "uniq", "wc", "sed", "awk",
            "curl", "wget", "ssh", "scp", "tar", "zip", "unzip", "make", "cmake", "gcc", "clear",
            "history", "man", "which", "whereis", "sudo", "apt", "brew", "pacman", "dnf", "yum",
        ]
    })
}

/// A suggestion from the Reflex Engine
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// The corrected command
    pub corrected: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Source of the suggestion
    pub source: SuggestionSource,
}

/// How the suggestion was derived
#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionSource {
    /// Exact match from known typo database
    KnownTypo,
    /// Levenshtein distance match against common commands
    Levenshtein,
    /// Character transposition detection
    Transposition,
}

/// The Reflex Engine: zero-ML heuristic command correction.
#[derive(Debug)]
pub struct ReflexEngine {
    /// Maximum Levenshtein distance to consider a match
    max_distance: usize,
    /// Minimum confidence threshold to return a suggestion.
    /// Raised from 0.40 → 0.55 to prevent false positives like
    /// exit→git (0.50), quit→git (0.50), abort→sort (0.60).
    min_confidence: f64,
}

impl ReflexEngine {
    pub fn new() -> Self {
        Self {
            max_distance: 3,
            min_confidence: 0.55,
        }
    }

    /// Create a ReflexEngine with custom thresholds.
    pub fn with_thresholds(max_distance: usize, min_confidence: f64) -> Self {
        Self {
            max_distance,
            min_confidence,
        }
    }

    /// Attempt to fix a mistyped command.
    /// Returns `Some(Suggestion)` if a correction is found above the confidence threshold.
    pub fn fix_command(&self, input: &str) -> Option<Suggestion> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Strategy 1: Check the known typo database (highest confidence)
        if let Some(suggestion) = self.check_known_typos(trimmed) {
            return Some(suggestion);
        }

        // Strategy 2: Try to fix just the first word (the command itself)
        if let Some(suggestion) = self.fix_first_word(trimmed) {
            if suggestion.confidence >= self.min_confidence {
                return Some(suggestion);
            }
        }

        None
    }

    /// Check against the known typo database for exact matches.
    fn check_known_typos(&self, input: &str) -> Option<Suggestion> {
        let lower = input.to_lowercase();
        let typos = known_typos();

        // Try full command match first
        if let Some(corrected) = typos.get(lower.as_str()) {
            return Some(Suggestion {
                corrected: corrected.to_string(),
                confidence: 1.0,
                source: SuggestionSource::KnownTypo,
            });
        }

        // Try matching just the first word against known single-word typos
        let first_word = lower.split_whitespace().next()?;
        if let Some(corrected) = typos.get(first_word) {
            let rest: String = input.trim().chars().skip(first_word.len()).collect();
            return Some(Suggestion {
                corrected: format!("{}{}", corrected, rest),
                confidence: 0.95,
                source: SuggestionSource::KnownTypo,
            });
        }

        None
    }

    /// Fix the first word of the command using Levenshtein distance.
    fn fix_first_word(&self, input: &str) -> Option<Suggestion> {
        let mut split = input.splitn(2, char::is_whitespace);
        let first_word_raw = split.next()?;
        let first_word = first_word_raw.to_lowercase();
        let rest = split.next().map(|s| format!(" {}", s)).unwrap_or_default();

        // Check for transposition first (higher confidence)
        if let Some(corrected) = self.detect_transposition(&first_word) {
            return Some(Suggestion {
                corrected: format!("{}{}", corrected, rest),
                confidence: 0.85,
                source: SuggestionSource::Transposition,
            });
        }

        // Levenshtein distance matching
        let mut best_match: Option<(&str, usize)> = None;

        for cmd in common_commands() {
            let dist = levenshtein_distance(&first_word, cmd);
            if dist <= self.max_distance && dist > 0 {
                match best_match {
                    None => best_match = Some((cmd, dist)),
                    Some((_, best_dist)) if dist < best_dist => {
                        best_match = Some((cmd, dist));
                    }
                    _ => {}
                }
            }
        }

        if let Some((corrected_cmd, dist)) = best_match {
            let max_len = first_word.len().max(corrected_cmd.len()) as f64;
            let confidence = 1.0 - (dist as f64 / max_len);

            return Some(Suggestion {
                corrected: format!("{}{}", corrected_cmd, rest),
                confidence,
                source: SuggestionSource::Levenshtein,
            });
        }

        None
    }

    /// Detect if the input is a character transposition of a known command.
    fn detect_transposition(&self, word: &str) -> Option<&'static str> {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() < 2 {
            return None;
        }

        for cmd in common_commands() {
            let cmd_chars: Vec<char> = cmd.chars().collect();
            if chars.len() != cmd_chars.len() {
                continue;
            }

            let diffs: Vec<usize> = chars
                .iter()
                .zip(cmd_chars.iter())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, _)| i)
                .collect();

            if diffs.len() == 2
                && diffs[1] - diffs[0] == 1
                && chars[diffs[0]] == cmd_chars[diffs[1]]
                && chars[diffs[1]] == cmd_chars[diffs[0]]
            {
                return Some(cmd);
            }
        }

        None
    }
}

impl Default for ReflexEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the Levenshtein edit distance between two strings.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for i in 1..=a_len {
        curr_row[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr_row[j] = (prev_row[j] + 1)
                .min(curr_row[j - 1] + 1)
                .min(prev_row[j - 1] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}