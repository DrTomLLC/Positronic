// positronic-bridge/src/block.rs
//
// Block-based output system (Roadmap Pillar IV).
//
// A TerminalBlock captures one command execution cycle: the input, all output
// lines, and metadata (timestamp, duration, exit code, CWD). The UI can
// render blocks as collapsible cards with copy/search support.

use chrono::{DateTime, Local};
use std::time::{Duration, Instant};

/// Unique identifier for a block.
pub type BlockId = u64;

/// A single terminal block representing one command execution.
#[derive(Debug, Clone)]
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

/// A single line within a block, with optional semantic classification.
#[derive(Debug, Clone)]
pub struct BlockLine {
    pub text: String,
    pub kind: LineKind,
}

/// Semantic line classification for coloring/filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Normal output text.
    Normal,
    /// Error or stderr content.
    Error,
    /// Warning messages.
    Warning,
    /// Success/completion messages.
    Success,
    /// Informational (headers, dividers, etc).
    Info,
    /// Muted/dimmed text.
    Muted,
}

/// Where the block's output came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSource {
    /// Output from a PTY shell command.
    Shell,
    /// Direct output from a native ! command.
    Native,
    /// Response from the Neural LLM.
    Neural,
    /// System message (startup, clear, etc).
    System,
}

impl BlockLine {
    pub fn normal(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Normal }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Error }
    }

    pub fn info(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Info }
    }

    pub fn success(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: LineKind::Success }
    }

    /// Classify a line by its content (heuristic).
    pub fn classify(text: impl Into<String>) -> Self {
        let s: String = text.into();
        let trimmed = s.trim_start();
        let kind = if trimmed.starts_with("error") || trimmed.starts_with("Error")
            || trimmed.starts_with("❌") || trimmed.starts_with("FAILED")
        {
            LineKind::Error
        } else if trimmed.starts_with("warning") || trimmed.starts_with("Warning")
            || trimmed.starts_with("⚠") || trimmed.starts_with("WARN")
        {
            LineKind::Warning
        } else if trimmed.starts_with("✅") || trimmed.starts_with("ok ")
            || trimmed.starts_with("Compiling") || trimmed.starts_with("Finished")
        {
            LineKind::Success
        } else if trimmed.starts_with("💡") || trimmed.starts_with("🧠")
            || trimmed.starts_with("📡") || trimmed.starts_with("🔌")
        {
            LineKind::Info
        } else {
            LineKind::Normal
        };
        Self { text: s, kind }
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
        for block in &mut self.blocks {
            block.collapsed = true;
        }
    }

    /// Get all blocks (for rendering).
    pub fn blocks(&self) -> &[TerminalBlock] {
        &self.blocks
    }

    /// Get the most recent block (if any).
    pub fn latest(&self) -> Option<&TerminalBlock> {
        self.blocks.last()
    }

    /// Get a block by ID.
    pub fn get(&self, block_id: BlockId) -> Option<&TerminalBlock> {
        self.blocks.iter().find(|b| b.id == block_id)
    }

    /// Search all blocks for lines containing a query string.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let lower_query = query.to_lowercase();
        let mut hits = Vec::new();

        for block in &self.blocks {
            // Check command
            if block.command.to_lowercase().contains(&lower_query) {
                hits.push(SearchHit {
                    block_id: block.id,
                    line_index: None,
                    context: block.command.clone(),
                });
            }
            // Check output lines
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

    /// Total number of output lines across all blocks.
    fn total_lines(&self) -> usize {
        self.blocks.iter().map(|b| b.output.len()).sum()
    }

    /// Remove oldest blocks to stay within limits.
    fn enforce_limits(&mut self) {
        // Block count limit
        while self.blocks.len() > self.max_blocks {
            self.blocks.remove(0);
        }
        // Total line count limit
        while self.total_lines() > self.max_total_lines && self.blocks.len() > 1 {
            self.blocks.remove(0);
        }
    }

    /// Clear all blocks (e.g. on Ctrl+L).
    pub fn clear(&mut self) {
        self.blocks.clear();
    }

    /// Get summary stats.
    pub fn stats(&self) -> BlockStats {
        let total_lines = self.total_lines();
        let running = self.blocks.iter().filter(|b| b.running).count();
        let errors = self.blocks.iter()
            .filter(|b| b.exit_code.map(|c| c != 0).unwrap_or(false))
            .count();
        BlockStats {
            total_blocks: self.blocks.len(),
            total_lines,
            running,
            errors,
        }
    }
}

/// A search result hit within the block history.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub block_id: BlockId,
    pub line_index: Option<usize>,
    pub context: String,
}

/// Summary statistics about blocks.
#[derive(Debug, Clone)]
pub struct BlockStats {
    pub total_blocks: usize,
    pub total_lines: usize,
    pub running: usize,
    pub errors: usize,
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
    let block_lines: Vec<BlockLine> = lines.into_iter()
        .map(BlockLine::classify)
        .collect();
    manager.append(id, block_lines);
    manager.finish(id, Some(0), start.elapsed());
    id
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

    #[test]
    fn test_enforce_limits() {
        let mut mgr = BlockManager::new(3, 1000);
        for i in 0..5 {
            mgr.begin(&format!("cmd {}", i), ".", BlockSource::Shell);
        }
        assert_eq!(mgr.blocks().len(), 3); // oldest 2 pruned
    }

    #[test]
    fn test_copy_block() {
        let mut mgr = BlockManager::default();
        let id = mgr.begin("ls -la", "/home", BlockSource::Shell);
        mgr.append(id, vec![BlockLine::normal("total 42")]);
        mgr.finish(id, Some(0), Duration::from_millis(5));

        let copied = mgr.copy_block(id).unwrap();
        assert!(copied.contains("$ ls -la"));
        assert!(copied.contains("total 42"));
        assert!(copied.contains("[exit 0]"));
    }

    #[test]
    fn test_line_classification() {
        assert_eq!(BlockLine::classify("error: something failed").kind, LineKind::Error);
        assert_eq!(BlockLine::classify("warning: unused variable").kind, LineKind::Warning);
        assert_eq!(BlockLine::classify("✅ All tests passed").kind, LineKind::Success);
        assert_eq!(BlockLine::classify("💡 Did you mean: ls?").kind, LineKind::Info);
        assert_eq!(BlockLine::classify("hello world").kind, LineKind::Normal);
    }

    #[test]
    fn test_collapse_toggle() {
        let mut mgr = BlockManager::default();
        let id = mgr.begin("test", ".", BlockSource::Native);
        assert!(!mgr.get(id).unwrap().collapsed);

        mgr.toggle_collapse(id);
        assert!(mgr.get(id).unwrap().collapsed);

        mgr.toggle_collapse(id);
        assert!(!mgr.get(id).unwrap().collapsed);
    }

    #[test]
    fn test_quick_block() {
        let mut mgr = BlockManager::default();
        let id = quick_block(
            &mut mgr,
            "!version",
            "/home",
            vec!["⚡ Positronic v0.2.0".to_string()],
            BlockSource::Native,
        );
        let block = mgr.get(id).unwrap();
        assert!(!block.running);
        assert_eq!(block.output.len(), 1);
    }
}