// positronic-bridge/src/block.rs
//
// Block-based output system (Roadmap Pillar IV).
//
// A TerminalBlock captures one command execution cycle: the input, all output
// lines, and metadata (timestamp, duration, exit code, CWD). The UI can
// render blocks as collapsible cards with copy/search support.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{Duration, Instant};

/// Unique identifier for a block.
pub type BlockId = u64;

/// A single terminal block representing one command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBlock {
    /// Monotonically increasing block ID.
    pub id: BlockId,
    /// The command the user typed.
    pub command: String,
    /// All output lines from this command.
    pub output: Vec<BlockLine>,
    /// When the command was submitted.
    pub timestamp: DateTime<Local>,
    /// How long the command took (None if still running).
    #[serde(with = "optional_duration_millis")]
    pub duration: Option<Duration>,
    /// Exit code from shell (None for native/neural commands).
    pub exit_code: Option<i32>,
    /// Working directory when the command was run.
    pub cwd: String,
    /// Source of the output.
    pub source: BlockSource,
    /// UI state: collapsed or expanded.
    pub collapsed: bool,
    /// Whether this block is still receiving output.
    pub running: bool,
}

impl TerminalBlock {
    /// Number of output lines in this block.
    pub fn line_count(&self) -> usize {
        self.output.len()
    }

    /// Whether this block finished with a non-zero exit code.
    pub fn failed(&self) -> bool {
        self.exit_code.map(|c| c != 0).unwrap_or(false)
    }

    /// Whether this block finished successfully (exit code 0).
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Get a human-readable duration string.
    pub fn duration_display(&self) -> String {
        match self.duration {
            Some(d) => format_duration(d),
            None if self.running => "running…".to_string(),
            None => "—".to_string(),
        }
    }

    /// Count lines of a specific kind.
    pub fn count_lines_of_kind(&self, kind: LineKind) -> usize {
        self.output.iter().filter(|l| l.kind == kind).count()
    }

    /// Get only error lines from this block.
    pub fn error_lines(&self) -> Vec<&BlockLine> {
        self.output.iter().filter(|l| l.kind == LineKind::Error).collect()
    }

    /// Get only warning lines from this block.
    pub fn warning_lines(&self) -> Vec<&BlockLine> {
        self.output.iter().filter(|l| l.kind == LineKind::Warning).collect()
    }
}

impl fmt::Display for TerminalBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.running {
            "⏳ running".to_string()
        } else {
            match self.exit_code {
                Some(0) => "✅ ok".to_string(),
                Some(code) => format!("❌ exit {}", code),
                None => "— done".to_string(),
            }
        };
        write!(f, "[{}] $ {} ({})", status, self.command, self.duration_display())
    }
}

/// Where the block's output came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockSource {
    /// A command executed via the PTY (legacy shell).
    Shell,
    /// A native P-Shell command (!alias, !history, etc.).
    Native,
    /// An AI-generated response (!ai, !explain, etc.).
    Neural,
    /// Hardware/IO output.
    Hardware,
    /// System-generated message (internal).
    System,
}

impl fmt::Display for BlockSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockSource::Shell => write!(f, "shell"),
            BlockSource::Native => write!(f, "native"),
            BlockSource::Neural => write!(f, "neural"),
            BlockSource::Hardware => write!(f, "hw"),
            BlockSource::System => write!(f, "system"),
        }
    }
}

/// Classification of a single output line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineKind {
    Normal,
    Error,
    Warning,
    Info,
    Success,
    Muted,
}

/// A single line of block output with classification metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockLine {
    pub text: String,
    pub kind: LineKind,
}

impl fmt::Display for LineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineKind::Normal => write!(f, "normal"),
            LineKind::Error => write!(f, "error"),
            LineKind::Warning => write!(f, "warning"),
            LineKind::Info => write!(f, "info"),
            LineKind::Success => write!(f, "success"),
            LineKind::Muted => write!(f, "muted"),
        }
    }
}

impl fmt::Display for BlockLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl BlockLine {
    pub fn normal(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Normal }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Error }
    }
    pub fn warning(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Warning }
    }
    pub fn info(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Info }
    }
    pub fn success(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Success }
    }
    pub fn muted(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Muted }
    }

    /// Auto-classify a line by content heuristics.
    pub fn classify(text: impl Into<String>) -> Self {
        let text = text.into();
        let trimmed = text.trim().to_lowercase();

        let kind = if trimmed.starts_with("error")
            || trimmed.starts_with("❌")
            || trimmed.starts_with("failed")
        {
            LineKind::Error
        } else if trimmed.starts_with("warning")
            || trimmed.starts_with("warn:")
            || trimmed.starts_with("warn ")
            || trimmed.starts_with("⚠")
        {
            LineKind::Warning
        } else if trimmed.starts_with("info")
            || trimmed.starts_with("note:")
            || trimmed.starts_with("💡")
            || trimmed.starts_with("📡")
        {
            LineKind::Info
        } else if trimmed.starts_with("✅")
            || trimmed.starts_with("ok ")
            || trimmed == "ok"
            || trimmed.starts_with("compiling")
            || trimmed.starts_with("finished")
            || trimmed.starts_with("passed")
        {
            LineKind::Success
        } else {
            LineKind::Normal
        };

        Self { text, kind }
    }

    /// Whether this line is effectively blank.
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Manages a scrolling list of terminal blocks with memory limits.
#[derive(Debug)]
pub struct BlockManager {
    /// All blocks in order.
    blocks: Vec<TerminalBlock>,
    /// Next block ID.
    next_id: BlockId,
    /// Maximum number of blocks to retain.
    max_blocks: usize,
    /// Total output line budget (oldest blocks pruned when exceeded).
    max_total_lines: usize,
}

impl Default for BlockManager {
    fn default() -> Self {
        Self::new(500, 50_000)
    }
}

impl BlockManager {
    /// Create a new block manager with limits.
    pub fn new(max_blocks: usize, max_total_lines: usize) -> Self {
        Self {
            blocks: Vec::with_capacity(64),
            next_id: 1,
            max_blocks,
            max_total_lines,
        }
    }

    /// Begin a new block for a command. Returns the block ID.
    pub fn begin(&mut self, command: &str, cwd: &str, source: BlockSource) -> BlockId {
        let id = self.next_id;
        self.next_id += 1;

        self.blocks.push(TerminalBlock {
            id,
            command: command.to_string(),
            output: Vec::new(),
            timestamp: Local::now(),
            duration: None,
            exit_code: None,
            cwd: cwd.to_string(),
            source,
            collapsed: false,
            running: true,
        });

        self.enforce_limits();
        id
    }

    /// Append output lines to a running block.
    pub fn append(&mut self, block_id: BlockId, lines: Vec<BlockLine>) {
        if let Some(block) = self.blocks.iter_mut().find(|b| b.id == block_id) {
            block.output.extend(lines);
        }
    }

    /// Append a single line to a running block.
    pub fn append_line(&mut self, block_id: BlockId, line: BlockLine) {
        if let Some(block) = self.blocks.iter_mut().find(|b| b.id == block_id) {
            block.output.push(line);
        }
    }

    /// Mark a block as finished with an optional exit code and duration.
    pub fn finish(&mut self, block_id: BlockId, exit_code: Option<i32>, duration: Duration) {
        if let Some(block) = self.blocks.iter_mut().find(|b| b.id == block_id) {
            block.running = false;
            block.exit_code = exit_code;
            block.duration = Some(duration);
        }
    }

    /// Toggle collapse state for a block.
    pub fn toggle_collapse(&mut self, block_id: BlockId) {
        if let Some(block) = self.blocks.iter_mut().find(|b| b.id == block_id) {
            block.collapsed = !block.collapsed;
        }
    }

    /// Collapse all blocks.
    pub fn collapse_all(&mut self) {
        for block in &mut self.blocks { block.collapsed = true; }
    }

    /// Expand all blocks.
    pub fn expand_all(&mut self) {
        for block in &mut self.blocks { block.collapsed = false; }
    }

    /// Get all blocks (for rendering).
    pub fn blocks(&self) -> &[TerminalBlock] { &self.blocks }

    /// Get the most recent block (if any).
    pub fn latest(&self) -> Option<&TerminalBlock> { self.blocks.last() }

    /// Get a mutable reference to the most recent block.
    pub fn latest_mut(&mut self) -> Option<&mut TerminalBlock> { self.blocks.last_mut() }

    /// Get a block by ID.
    pub fn get(&self, block_id: BlockId) -> Option<&TerminalBlock> {
        self.blocks.iter().find(|b| b.id == block_id)
    }

    /// Get a mutable block by ID.
    pub fn get_mut(&mut self, block_id: BlockId) -> Option<&mut TerminalBlock> {
        self.blocks.iter_mut().find(|b| b.id == block_id)
    }

    /// Remove a block by ID.
    pub fn remove(&mut self, block_id: BlockId) -> Option<TerminalBlock> {
        if let Some(pos) = self.blocks.iter().position(|b| b.id == block_id) {
            Some(self.blocks.remove(pos))
        } else {
            None
        }
    }

    /// Get the total number of blocks.
    pub fn len(&self) -> usize { self.blocks.len() }

    /// Whether the block manager has no blocks.
    pub fn is_empty(&self) -> bool { self.blocks.is_empty() }

    /// Filter blocks by source type.
    pub fn filter_by_source(&self, source: BlockSource) -> Vec<&TerminalBlock> {
        self.blocks.iter().filter(|b| b.source == source).collect()
    }

    /// Filter blocks by exit code.
    pub fn filter_by_exit_code(&self, exit_code: i32) -> Vec<&TerminalBlock> {
        self.blocks.iter().filter(|b| b.exit_code == Some(exit_code)).collect()
    }

    /// Get all currently running blocks.
    pub fn running_blocks(&self) -> Vec<&TerminalBlock> {
        self.blocks.iter().filter(|b| b.running).collect()
    }

    /// Get all blocks that finished with a non-zero exit code.
    pub fn failed_blocks(&self) -> Vec<&TerminalBlock> {
        self.blocks.iter().filter(|b| b.failed()).collect()
    }

    /// Get all blocks that finished successfully (exit code 0).
    pub fn succeeded_blocks(&self) -> Vec<&TerminalBlock> {
        self.blocks.iter().filter(|b| b.succeeded()).collect()
    }

    /// Search all blocks for lines containing a query string.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let lower_query = query.to_lowercase();
        let mut hits = Vec::new();

        for block in &self.blocks {
            if block.command.to_lowercase().contains(&lower_query) {
                hits.push(SearchHit {
                    block_id: block.id,
                    line_index: None,
                    context: block.command.clone(),
                });
            }
            for (i, line) in block.output.iter().enumerate() {
                if line.text.to_lowercase().contains(&lower_query) {
                    hits.push(SearchHit {
                        block_id: block.id,
                        line_index: Some(i),
                        context: line.text.clone(),
                    });
                }
            }
        }
        hits
    }

    /// Copy a single block's output to a string (for clipboard).
    pub fn copy_block(&self, block_id: BlockId) -> Option<String> {
        self.get(block_id).map(|block| {
            let mut out = format!("$ {}\n", block.command);
            for line in &block.output {
                out.push_str(&line.text);
                out.push('\n');
            }
            if let Some(code) = block.exit_code {
                out.push_str(&format!("[exit {}]", code));
            }
            out
        })
    }

    /// Export all blocks to a single text string.
    pub fn export_all(&self) -> String {
        let mut out = String::new();
        for (i, block) in self.blocks.iter().enumerate() {
            if i > 0 {
                out.push_str("\n────────────────────────────────────\n\n");
            }
            out.push_str(&format!("$ {} [{}] ({})\n", block.command, block.source, block.cwd));
            for line in &block.output {
                out.push_str(&line.text);
                out.push('\n');
            }
            if let Some(code) = block.exit_code {
                out.push_str(&format!("[exit {}]", code));
            }
            if let Some(d) = block.duration {
                out.push_str(&format!(" [{}]", format_duration(d)));
            }
            out.push('\n');
        }
        out
    }

    /// Total number of output lines across all blocks.
    pub fn total_lines(&self) -> usize {
        self.blocks.iter().map(|b| b.output.len()).sum()
    }

    /// Remove oldest blocks to stay within limits.
    fn enforce_limits(&mut self) {
        while self.blocks.len() > self.max_blocks {
            self.blocks.remove(0);
        }
        while self.total_lines() > self.max_total_lines && self.blocks.len() > 1 {
            self.blocks.remove(0);
        }
    }

    /// Clear all blocks (e.g. on Ctrl+L).
    pub fn clear(&mut self) { self.blocks.clear(); }

    /// Get summary stats.
    pub fn stats(&self) -> BlockStats {
        BlockStats {
            total_blocks: self.blocks.len(),
            total_lines: self.total_lines(),
            running: self.blocks.iter().filter(|b| b.running).count(),
            errors: self.blocks.iter().filter(|b| b.exit_code.map(|c| c != 0).unwrap_or(false)).count(),
        }
    }
}

/// A search result hit within the block history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub block_id: BlockId,
    pub line_index: Option<usize>,
    pub context: String,
}

/// Summary statistics about blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStats {
    pub total_blocks: usize,
    pub total_lines: usize,
    pub running: usize,
    pub errors: usize,
}

impl fmt::Display for BlockStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} blocks, {} lines, {} running, {} errors",
            self.total_blocks, self.total_lines, self.running, self.errors
        )
    }
}

/// Helper to create a complete block for a native command in one shot.
pub fn quick_block(
    manager: &mut BlockManager,
    command: &str,
    cwd: &str,
    lines: Vec<String>,
    source: BlockSource,
) -> BlockId {
    let start = Instant::now();
    let id = manager.begin(command, cwd, source);
    let block_lines: Vec<BlockLine> = lines.into_iter().map(BlockLine::classify).collect();
    manager.append(id, block_lines);
    manager.finish(id, Some(0), start.elapsed());
    id
}

/// Helper to create a failed block in one shot.
pub fn quick_error_block(
    manager: &mut BlockManager,
    command: &str,
    cwd: &str,
    error_msg: &str,
    exit_code: i32,
) -> BlockId {
    let start = Instant::now();
    let id = manager.begin(command, cwd, BlockSource::Shell);
    manager.append(id, vec![BlockLine::error(error_msg)]);
    manager.finish(id, Some(exit_code), start.elapsed());
    id
}

/// Format a Duration into a human-readable string.
pub fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let millis = d.subsec_millis();

    if total_secs == 0 {
        format!("{}ms", millis)
    } else if total_secs < 60 {
        format!("{}.{:03}s", total_secs, millis)
    } else if total_secs < 3600 {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{}m {:02}s", mins, secs)
    } else {
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        format!("{}h {:02}m", hours, mins)
    }
}

/// Serde helper for optional Duration stored as milliseconds.
mod optional_duration_millis {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(dur: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        match dur {
            Some(d) => d.as_millis().serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
        let opt: Option<u128> = Option::deserialize(d)?;
        Ok(opt.map(|ms| Duration::from_millis(ms as u64)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_lifecycle() {
        let mut mgr = BlockManager::default();
        let id = mgr.begin("cargo test", "/home/user/project", BlockSource::Shell);
        assert!(mgr.get(id).unwrap().running);
        mgr.append(id, vec![
            BlockLine::normal("running 5 tests"),
            BlockLine::success("test result: ok. 5 passed"),
        ]);
        mgr.finish(id, Some(0), Duration::from_millis(250));
        let block = mgr.get(id).unwrap();
        assert!(!block.running);
        assert_eq!(block.exit_code, Some(0));
        assert_eq!(block.output.len(), 2);
    }

    #[test]
    fn test_search() {
        let mut mgr = BlockManager::default();
        let id = mgr.begin("cargo build", ".", BlockSource::Shell);
        mgr.append(id, vec![
            BlockLine::normal("Compiling myapp v0.1.0"),
            BlockLine::error("error[E0308]: mismatched types"),
        ]);
        mgr.finish(id, Some(1), Duration::from_secs(1));

        let hits = mgr.search("E0308");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id, id);
    }
}