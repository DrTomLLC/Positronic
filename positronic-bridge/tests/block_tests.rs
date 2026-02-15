// positronic-bridge/tests/block_tests.rs
//
// Integration tests for the Block-based output system (Pillar IV).
// Tests all public API surface of block.rs: constructors, lifecycle,
// filtering, serialization, display, limits, search, and export.

use positronic_bridge::block::{
    BlockId, BlockLine, BlockManager, BlockSource, BlockStats, LineKind, SearchHit,
    TerminalBlock, format_duration, quick_block, quick_error_block,
};
use std::time::Duration;

// ============================================================================
// BlockLine Constructor Tests
// ============================================================================

#[test]
fn test_blockline_normal() {
    let line = BlockLine::normal("hello world");
    assert_eq!(line.text, "hello world");
    assert_eq!(line.kind, LineKind::Normal);
}

#[test]
fn test_blockline_error() {
    let line = BlockLine::error("fatal error");
    assert_eq!(line.text, "fatal error");
    assert_eq!(line.kind, LineKind::Error);
}

#[test]
fn test_blockline_warning() {
    let line = BlockLine::warning("unused variable");
    assert_eq!(line.text, "unused variable");
    assert_eq!(line.kind, LineKind::Warning);
}

#[test]
fn test_blockline_info() {
    let line = BlockLine::info("compiling...");
    assert_eq!(line.text, "compiling...");
    assert_eq!(line.kind, LineKind::Info);
}

#[test]
fn test_blockline_success() {
    let line = BlockLine::success("all tests passed");
    assert_eq!(line.text, "all tests passed");
    assert_eq!(line.kind, LineKind::Success);
}

#[test]
fn test_blockline_muted() {
    let line = BlockLine::muted("debug trace...");
    assert_eq!(line.text, "debug trace...");
    assert_eq!(line.kind, LineKind::Muted);
}

#[test]
fn test_blockline_from_string_owned() {
    let owned = String::from("owned text");
    let line = BlockLine::normal(owned);
    assert_eq!(line.text, "owned text");
}

// ============================================================================
// BlockLine::classify Tests
// ============================================================================

#[test]
fn test_classify_error_lowercase() {
    assert_eq!(BlockLine::classify("error: something failed").kind, LineKind::Error);
}

#[test]
fn test_classify_error_titlecase() {
    assert_eq!(BlockLine::classify("Error: something failed").kind, LineKind::Error);
}

#[test]
fn test_classify_error_uppercase() {
    assert_eq!(BlockLine::classify("ERROR: something failed").kind, LineKind::Error);
}

#[test]
fn test_classify_error_emoji() {
    assert_eq!(BlockLine::classify("❌ Build failed").kind, LineKind::Error);
}

#[test]
fn test_classify_error_failed() {
    assert_eq!(BlockLine::classify("FAILED test_xyz").kind, LineKind::Error);
}

#[test]
fn test_classify_warning_lowercase() {
    assert_eq!(BlockLine::classify("warning: unused variable").kind, LineKind::Warning);
}

#[test]
fn test_classify_warning_titlecase() {
    assert_eq!(BlockLine::classify("Warning: deprecated API").kind, LineKind::Warning);
}

#[test]
fn test_classify_warning_emoji() {
    assert_eq!(BlockLine::classify("⚠ risky operation").kind, LineKind::Warning);
}

#[test]
fn test_classify_warning_warn() {
    assert_eq!(BlockLine::classify("WARN: disk 80% full").kind, LineKind::Warning);
}

#[test]
fn test_classify_success_checkmark() {
    assert_eq!(BlockLine::classify("✅ All tests passed").kind, LineKind::Success);
}

#[test]
fn test_classify_success_ok() {
    assert_eq!(BlockLine::classify("ok test_something").kind, LineKind::Success);
}

#[test]
fn test_classify_success_compiling() {
    assert_eq!(BlockLine::classify("Compiling myapp v0.1.0").kind, LineKind::Success);
}

#[test]
fn test_classify_success_finished() {
    assert_eq!(BlockLine::classify("Finished dev [unoptimized]").kind, LineKind::Success);
}

#[test]
fn test_classify_success_passed() {
    assert_eq!(BlockLine::classify("PASSED test_abc").kind, LineKind::Success);
}

#[test]
fn test_classify_info_lightbulb() {
    assert_eq!(BlockLine::classify("💡 Did you mean: ls?").kind, LineKind::Info);
}

#[test]
fn test_classify_info_note() {
    assert_eq!(BlockLine::classify("note: see also `--help`").kind, LineKind::Info);
}

#[test]
fn test_classify_info_satellite() {
    assert_eq!(BlockLine::classify("📡 connected to device").kind, LineKind::Info);
}

#[test]
fn test_classify_normal_plain_text() {
    assert_eq!(BlockLine::classify("hello world").kind, LineKind::Normal);
}

#[test]
fn test_classify_empty_is_normal() {
    assert_eq!(BlockLine::classify("").kind, LineKind::Normal);
}

#[test]
fn test_classify_preserves_leading_whitespace() {
    let line = BlockLine::classify("  error: indented error");
    assert_eq!(line.kind, LineKind::Error);
    assert_eq!(line.text, "  error: indented error");
}

// ============================================================================
// BlockLine::is_blank
// ============================================================================

#[test]
fn test_blockline_is_blank_empty() {
    assert!(BlockLine::normal("").is_blank());
}

#[test]
fn test_blockline_is_blank_whitespace() {
    assert!(BlockLine::normal("   ").is_blank());
}

#[test]
fn test_blockline_is_blank_content() {
    assert!(!BlockLine::normal("hello").is_blank());
}

// ============================================================================
// Display Implementations
// ============================================================================

#[test]
fn test_blockline_display() {
    let line = BlockLine::normal("output text");
    assert_eq!(format!("{}", line), "output text");
}

#[test]
fn test_linekind_display() {
    assert_eq!(format!("{}", LineKind::Normal), "normal");
    assert_eq!(format!("{}", LineKind::Error), "error");
    assert_eq!(format!("{}", LineKind::Warning), "warning");
    assert_eq!(format!("{}", LineKind::Success), "success");
    assert_eq!(format!("{}", LineKind::Info), "info");
    assert_eq!(format!("{}", LineKind::Muted), "muted");
}

#[test]
fn test_blocksource_display() {
    assert_eq!(format!("{}", BlockSource::Shell), "shell");
    assert_eq!(format!("{}", BlockSource::Native), "native");
    assert_eq!(format!("{}", BlockSource::Neural), "neural");
    assert_eq!(format!("{}", BlockSource::System), "system");
}

// ============================================================================
// format_duration
// ============================================================================

#[test]
fn test_format_duration_millis() {
    assert_eq!(format_duration(Duration::from_millis(42)), "42ms");
}

#[test]
fn test_format_duration_seconds() {
    assert_eq!(format_duration(Duration::from_millis(3_500)), "3.500s");
}

#[test]
fn test_format_duration_minutes() {
    assert_eq!(format_duration(Duration::from_secs(125)), "2m 05s");
}

#[test]
fn test_format_duration_hours() {
    assert_eq!(format_duration(Duration::from_secs(3661)), "1h 01m");
}

#[test]
fn test_format_duration_zero() {
    assert_eq!(format_duration(Duration::ZERO), "0ms");
}

// ============================================================================
// BlockManager — Creation
// ============================================================================

#[test]
fn test_manager_new() {
    let mgr = BlockManager::new(100, 5000);
    assert!(mgr.is_empty());
    assert_eq!(mgr.len(), 0);
    assert!(mgr.blocks().is_empty());
}

#[test]
fn test_manager_default() {
    let mgr = BlockManager::default();
    assert!(mgr.is_empty());
    assert_eq!(mgr.len(), 0);
}

// ============================================================================
// BlockManager — Lifecycle
// ============================================================================

#[test]
fn test_manager_begin_creates_running_block() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("ls -la", "/home", BlockSource::Shell);
    assert_eq!(mgr.len(), 1);

    let block = mgr.get(id).unwrap();
    assert!(block.running);
    assert_eq!(block.command, "ls -la");
    assert_eq!(block.cwd, "/home");
    assert_eq!(block.source, BlockSource::Shell);
    assert!(block.exit_code.is_none());
    assert!(block.duration.is_none());
    assert!(!block.collapsed);
}

#[test]
fn test_manager_monotonic_ids() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("a", ".", BlockSource::Shell);
    let id2 = mgr.begin("b", ".", BlockSource::Shell);
    let id3 = mgr.begin("c", ".", BlockSource::Shell);
    assert!(id1 < id2);
    assert!(id2 < id3);
}

#[test]
fn test_manager_append() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("echo hi", ".", BlockSource::Shell);
    mgr.append(id, vec![BlockLine::normal("hi")]);

    let block = mgr.get(id).unwrap();
    assert_eq!(block.output.len(), 1);
    assert_eq!(block.output[0].text, "hi");
}

#[test]
fn test_manager_append_line() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    mgr.append_line(id, BlockLine::normal("single line"));

    let block = mgr.get(id).unwrap();
    assert_eq!(block.output.len(), 1);
    assert_eq!(block.output[0].text, "single line");
}

#[test]
fn test_manager_append_to_nonexistent_is_noop() {
    let mut mgr = BlockManager::default();
    mgr.append(999, vec![BlockLine::normal("ghost")]);
    assert!(mgr.is_empty());
}

#[test]
fn test_manager_finish_marks_complete() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("make", ".", BlockSource::Shell);
    mgr.finish(id, Some(0), Duration::from_millis(250));

    let block = mgr.get(id).unwrap();
    assert!(!block.running);
    assert_eq!(block.exit_code, Some(0));
    assert!(block.duration.is_some());
}

#[test]
fn test_manager_finish_with_error_code() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("make", ".", BlockSource::Shell);
    mgr.finish(id, Some(2), Duration::from_secs(1));

    let block = mgr.get(id).unwrap();
    assert_eq!(block.exit_code, Some(2));
    assert!(block.failed());
    assert!(!block.succeeded());
}

#[test]
fn test_manager_finish_nonexistent_is_noop() {
    let mut mgr = BlockManager::default();
    mgr.finish(999, Some(0), Duration::from_millis(1));
    assert!(mgr.is_empty());
}

#[test]
fn test_full_lifecycle() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("cargo test", "/project", BlockSource::Shell);

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
    assert!(block.succeeded());
    assert!(!block.failed());
}

// ============================================================================
// BlockManager — Retrieval
// ============================================================================

#[test]
fn test_manager_latest_empty() {
    let mgr = BlockManager::default();
    assert!(mgr.latest().is_none());
}

#[test]
fn test_manager_latest_returns_last() {
    let mut mgr = BlockManager::default();
    mgr.begin("first", ".", BlockSource::Shell);
    mgr.begin("second", ".", BlockSource::Shell);
    mgr.begin("third", ".", BlockSource::Shell);
    assert_eq!(mgr.latest().unwrap().command, "third");
}

#[test]
fn test_manager_latest_mut() {
    let mut mgr = BlockManager::default();
    mgr.begin("test", ".", BlockSource::Shell);

    let block = mgr.latest_mut().unwrap();
    block.command = "modified".to_string();
    assert_eq!(mgr.latest().unwrap().command, "modified");
}

#[test]
fn test_manager_get_nonexistent() {
    let mgr = BlockManager::default();
    assert!(mgr.get(999).is_none());
}

#[test]
fn test_manager_get_mut() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    mgr.get_mut(id).unwrap().collapsed = true;
    assert!(mgr.get(id).unwrap().collapsed);
}

#[test]
fn test_manager_remove() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    let removed = mgr.remove(id);
    assert!(removed.is_some());
    assert!(mgr.is_empty());
}

#[test]
fn test_manager_remove_nonexistent() {
    let mut mgr = BlockManager::default();
    assert!(mgr.remove(999).is_none());
}

#[test]
fn test_manager_clear() {
    let mut mgr = BlockManager::default();
    mgr.begin("a", ".", BlockSource::Shell);
    mgr.begin("b", ".", BlockSource::Shell);
    mgr.clear();
    assert!(mgr.is_empty());
}

// ============================================================================
// BlockManager — Collapse/Expand
// ============================================================================

#[test]
fn test_toggle_collapse() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Native);
    assert!(!mgr.get(id).unwrap().collapsed);

    mgr.toggle_collapse(id);
    assert!(mgr.get(id).unwrap().collapsed);

    mgr.toggle_collapse(id);
    assert!(!mgr.get(id).unwrap().collapsed);
}

#[test]
fn test_collapse_all() {
    let mut mgr = BlockManager::default();
    mgr.begin("a", ".", BlockSource::Shell);
    mgr.begin("b", ".", BlockSource::Shell);
    mgr.begin("c", ".", BlockSource::Shell);

    mgr.collapse_all();
    for block in mgr.blocks() {
        assert!(block.collapsed);
    }
}

#[test]
fn test_expand_all() {
    let mut mgr = BlockManager::default();
    mgr.begin("a", ".", BlockSource::Shell);
    mgr.begin("b", ".", BlockSource::Shell);

    mgr.collapse_all();
    mgr.expand_all();
    for block in mgr.blocks() {
        assert!(!block.collapsed);
    }
}

#[test]
fn test_collapse_expand_round_trip() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("a", ".", BlockSource::Shell);
    let id2 = mgr.begin("b", ".", BlockSource::Shell);

    mgr.toggle_collapse(id1);
    mgr.collapse_all();
    mgr.expand_all();

    assert!(!mgr.get(id1).unwrap().collapsed);
    assert!(!mgr.get(id2).unwrap().collapsed);
}

// ============================================================================
// BlockManager — Filters
// ============================================================================

#[test]
fn test_filter_by_source() {
    let mut mgr = BlockManager::default();
    mgr.begin("shell cmd", ".", BlockSource::Shell);
    mgr.begin("native cmd", ".", BlockSource::Native);
    mgr.begin("neural cmd", ".", BlockSource::Neural);
    mgr.begin("system msg", ".", BlockSource::System);
    mgr.begin("shell cmd 2", ".", BlockSource::Shell);

    assert_eq!(mgr.filter_by_source(BlockSource::Shell).len(), 2);
    assert_eq!(mgr.filter_by_source(BlockSource::Neural).len(), 1);
    assert_eq!(mgr.filter_by_source(BlockSource::Neural)[0].command, "neural cmd");
}

#[test]
fn test_filter_by_source_none_found() {
    let mut mgr = BlockManager::default();
    mgr.begin("shell cmd", ".", BlockSource::Shell);
    assert!(mgr.filter_by_source(BlockSource::Neural).is_empty());
}

#[test]
fn test_filter_by_exit_code() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("success", ".", BlockSource::Shell);
    mgr.finish(id1, Some(0), Duration::from_millis(1));
    let id2 = mgr.begin("fail", ".", BlockSource::Shell);
    mgr.finish(id2, Some(1), Duration::from_millis(1));
    let id3 = mgr.begin("also success", ".", BlockSource::Shell);
    mgr.finish(id3, Some(0), Duration::from_millis(1));

    assert_eq!(mgr.filter_by_exit_code(0).len(), 2);
    assert_eq!(mgr.filter_by_exit_code(1).len(), 1);
    assert_eq!(mgr.filter_by_exit_code(1)[0].command, "fail");
}

#[test]
fn test_running_blocks() {
    let mut mgr = BlockManager::default();
    mgr.begin("running1", ".", BlockSource::Shell);
    let id2 = mgr.begin("done", ".", BlockSource::Shell);
    mgr.finish(id2, Some(0), Duration::from_millis(1));
    mgr.begin("running2", ".", BlockSource::Shell);

    assert_eq!(mgr.running_blocks().len(), 2);
}

#[test]
fn test_failed_blocks() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("ok", ".", BlockSource::Shell);
    mgr.finish(id1, Some(0), Duration::from_millis(1));
    let id2 = mgr.begin("bad", ".", BlockSource::Shell);
    mgr.finish(id2, Some(1), Duration::from_millis(1));
    let id3 = mgr.begin("worse", ".", BlockSource::Shell);
    mgr.finish(id3, Some(127), Duration::from_millis(1));

    assert_eq!(mgr.failed_blocks().len(), 2);
}

#[test]
fn test_succeeded_blocks() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("ok1", ".", BlockSource::Shell);
    mgr.finish(id1, Some(0), Duration::from_millis(1));
    let id2 = mgr.begin("bad", ".", BlockSource::Shell);
    mgr.finish(id2, Some(1), Duration::from_millis(1));
    let id3 = mgr.begin("ok2", ".", BlockSource::Shell);
    mgr.finish(id3, Some(0), Duration::from_millis(1));

    assert_eq!(mgr.succeeded_blocks().len(), 2);
}

// ============================================================================
// BlockManager — Search
// ============================================================================

#[test]
fn test_search_in_command() {
    let mut mgr = BlockManager::default();
    mgr.begin("cargo build --release", ".", BlockSource::Shell);
    mgr.begin("ls -la", ".", BlockSource::Shell);

    let hits = mgr.search("cargo");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].line_index.is_none()); // hit was in command
}

#[test]
fn test_search_in_output() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("cargo build", ".", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("Compiling myapp v0.1.0"),
        BlockLine::error("error[E0308]: mismatched types"),
    ]);

    let hits = mgr.search("E0308");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line_index, Some(1));
}

#[test]
fn test_search_case_insensitive() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("echo", ".", BlockSource::Shell);
    mgr.append(id, vec![BlockLine::normal("Hello World")]);

    let hits = mgr.search("hello");
    assert_eq!(hits.len(), 1);
}

#[test]
fn test_search_multiple_hits() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("cargo test", ".", BlockSource::Shell);
    mgr.append(id1, vec![BlockLine::normal("test_foo ... ok")]);
    let id2 = mgr.begin("cargo test --lib", ".", BlockSource::Shell);
    mgr.append(id2, vec![BlockLine::normal("test_bar ... ok")]);

    let hits = mgr.search("cargo test");
    assert!(hits.len() >= 2);
}

#[test]
fn test_search_no_results() {
    let mut mgr = BlockManager::default();
    mgr.begin("ls", ".", BlockSource::Shell);

    let hits = mgr.search("nonexistent_query_xyz");
    assert!(hits.is_empty());
}

#[test]
fn test_search_empty_query() {
    let mut mgr = BlockManager::default();
    mgr.begin("ls", ".", BlockSource::Shell);

    let hits = mgr.search("");
    assert!(!hits.is_empty());
}

// ============================================================================
// BlockManager — Copy & Export
// ============================================================================

#[test]
fn test_copy_block_format() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("ls -la", "/home", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("total 42"),
        BlockLine::normal("drwxr-xr-x 2 user user 4096"),
    ]);
    mgr.finish(id, Some(0), Duration::from_millis(5));

    let copied = mgr.copy_block(id).unwrap();
    assert!(copied.starts_with("$ ls -la\n"));
    assert!(copied.contains("total 42\n"));
    assert!(copied.contains("drwxr-xr-x"));
    assert!(copied.contains("[exit 0]"));
}

#[test]
fn test_copy_block_no_exit_code() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("running cmd", ".", BlockSource::Shell);
    mgr.append(id, vec![BlockLine::normal("output")]);

    let copied = mgr.copy_block(id).unwrap();
    assert!(copied.contains("$ running cmd"));
    assert!(!copied.contains("[exit"));
}

#[test]
fn test_copy_block_nonexistent() {
    let mgr = BlockManager::default();
    assert!(mgr.copy_block(999).is_none());
}

#[test]
fn test_export_all_empty() {
    let mgr = BlockManager::default();
    assert!(mgr.export_all().is_empty());
}

#[test]
fn test_export_all_single_block() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("echo hi", "/home", BlockSource::Shell);
    mgr.append(id, vec![BlockLine::normal("hi")]);
    mgr.finish(id, Some(0), Duration::from_millis(5));

    let exported = mgr.export_all();
    assert!(exported.contains("$ echo hi"));
    assert!(exported.contains("hi\n"));
}

#[test]
fn test_export_all_separators() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("cmd1", ".", BlockSource::Shell);
    mgr.finish(id1, Some(0), Duration::from_millis(1));
    let id2 = mgr.begin("cmd2", ".", BlockSource::Shell);
    mgr.finish(id2, Some(0), Duration::from_millis(1));

    let exported = mgr.export_all();
    assert!(exported.contains("────"));
}

// ============================================================================
// BlockManager — Limits
// ============================================================================

#[test]
fn test_enforce_block_count_limit() {
    let mut mgr = BlockManager::new(3, 1000);
    for i in 0..5 {
        mgr.begin(&format!("cmd {}", i), ".", BlockSource::Shell);
    }
    assert_eq!(mgr.blocks().len(), 3);
}

#[test]
fn test_total_lines() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("a", ".", BlockSource::Shell);
    mgr.append(id1, vec![BlockLine::normal("1"), BlockLine::normal("2")]);
    let id2 = mgr.begin("b", ".", BlockSource::Shell);
    mgr.append(id2, vec![BlockLine::normal("3")]);

    assert_eq!(mgr.total_lines(), 3);
}

// ============================================================================
// BlockManager — Stats
// ============================================================================

#[test]
fn test_stats() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("ok", ".", BlockSource::Shell);
    mgr.append(id1, vec![BlockLine::normal("a"); 100]);
    mgr.finish(id1, Some(0), Duration::from_millis(1));

    let id2 = mgr.begin("bad", ".", BlockSource::Shell);
    mgr.append(id2, vec![BlockLine::normal("b"); 150]);
    mgr.finish(id2, Some(1), Duration::from_millis(1));

    mgr.begin("running", ".", BlockSource::Shell);

    let stats = mgr.stats();
    assert_eq!(stats.total_blocks, 3);
    assert_eq!(stats.total_lines, 250);
    assert_eq!(stats.running, 1);
    assert_eq!(stats.errors, 1);
}

#[test]
fn test_stats_display() {
    let stats = BlockStats {
        total_blocks: 10,
        total_lines: 250,
        running: 2,
        errors: 1,
    };
    let display = format!("{}", stats);
    assert!(display.contains("10 blocks"));
    assert!(display.contains("250 lines"));
    assert!(display.contains("2 running"));
    assert!(display.contains("1 errors"));
}

// ============================================================================
// TerminalBlock Methods
// ============================================================================

#[test]
fn test_block_line_count() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    mgr.append(id, vec![BlockLine::normal("a"), BlockLine::normal("b"), BlockLine::normal("c")]);
    assert_eq!(mgr.get(id).unwrap().line_count(), 3);
}

#[test]
fn test_block_failed() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    mgr.finish(id, Some(1), Duration::from_millis(1));
    assert!(mgr.get(id).unwrap().failed());
}

#[test]
fn test_block_succeeded() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    mgr.finish(id, Some(0), Duration::from_millis(1));
    assert!(mgr.get(id).unwrap().succeeded());
}

#[test]
fn test_block_failed_running_is_false() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    assert!(!mgr.get(id).unwrap().failed());
}

#[test]
fn test_block_succeeded_running_is_false() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    assert!(!mgr.get(id).unwrap().succeeded());
}

#[test]
fn test_block_duration_display_running() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    assert_eq!(mgr.get(id).unwrap().duration_display(), "running…");
}

#[test]
fn test_block_duration_display_finished() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    mgr.finish(id, Some(0), Duration::from_millis(42));
    assert_eq!(mgr.get(id).unwrap().duration_display(), "42ms");
}

#[test]
fn test_block_count_lines_of_kind() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("build", ".", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("output"),
        BlockLine::error("error 1"),
        BlockLine::error("error 2"),
        BlockLine::warning("warning 1"),
    ]);

    let block = mgr.get(id).unwrap();
    assert_eq!(block.count_lines_of_kind(LineKind::Error), 2);
    assert_eq!(block.count_lines_of_kind(LineKind::Warning), 1);
    assert_eq!(block.count_lines_of_kind(LineKind::Normal), 1);
    assert_eq!(block.count_lines_of_kind(LineKind::Success), 0);
}

#[test]
fn test_block_error_lines() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("build", ".", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("ok"),
        BlockLine::error("E0308"),
        BlockLine::error("E0599"),
    ]);

    let errors = mgr.get(id).unwrap().error_lines();
    assert_eq!(errors.len(), 2);
}

#[test]
fn test_block_warning_lines() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("build", ".", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::warning("unused var"),
        BlockLine::normal("ok"),
        BlockLine::warning("deprecated"),
    ]);

    let warnings = mgr.get(id).unwrap().warning_lines();
    assert_eq!(warnings.len(), 2);
}

// ============================================================================
// Serialization
// ============================================================================

#[test]
fn test_terminal_block_serialize_roundtrip() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("cargo build", "/project", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("Compiling..."),
        BlockLine::success("Finished"),
    ]);
    mgr.finish(id, Some(0), Duration::from_millis(500));

    let block = mgr.get(id).unwrap();
    let json = serde_json::to_string(block).unwrap();
    let deserialized: TerminalBlock = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, block.id);
    assert_eq!(deserialized.command, "cargo build");
    assert_eq!(deserialized.cwd, "/project");
    assert_eq!(deserialized.exit_code, Some(0));
    assert!(!deserialized.running);
    assert_eq!(deserialized.output.len(), 2);
    assert_eq!(deserialized.source, BlockSource::Shell);
}

#[test]
fn test_terminal_block_serialize_running() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("long running", ".", BlockSource::Shell);

    let block = mgr.get(id).unwrap();
    let json = serde_json::to_string(block).unwrap();
    let deserialized: TerminalBlock = serde_json::from_str(&json).unwrap();

    assert!(deserialized.running);
    assert!(deserialized.duration.is_none());
    assert!(deserialized.exit_code.is_none());
}

#[test]
fn test_search_hit_serialize() {
    let hit = SearchHit {
        block_id: 42,
        line_index: Some(7),
        context: "error[E0308]".to_string(),
    };
    let json = serde_json::to_string(&hit).unwrap();
    let de: SearchHit = serde_json::from_str(&json).unwrap();
    assert_eq!(de.block_id, 42);
    assert_eq!(de.line_index, Some(7));
    assert_eq!(de.context, "error[E0308]");
}

#[test]
fn test_block_stats_serialize() {
    let stats = BlockStats {
        total_blocks: 100,
        total_lines: 5000,
        running: 3,
        errors: 7,
    };
    let json = serde_json::to_string(&stats).unwrap();
    let de: BlockStats = serde_json::from_str(&json).unwrap();
    assert_eq!(de.total_blocks, 100);
    assert_eq!(de.errors, 7);
}

// ============================================================================
// Quick Helpers
// ============================================================================

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
    assert_eq!(block.source, BlockSource::Native);
}

#[test]
fn test_quick_error_block() {
    let mut mgr = BlockManager::default();
    let id = quick_error_block(&mut mgr, "bad_cmd", "/home", "command not found", 127);
    let block = mgr.get(id).unwrap();
    assert!(!block.running);
    assert_eq!(block.exit_code, Some(127));
    assert!(block.failed());
    assert_eq!(block.output[0].kind, LineKind::Error);
}

// ============================================================================
// Edge Cases / Stress
// ============================================================================

#[test]
fn test_many_blocks_sequential() {
    let mut mgr = BlockManager::new(1000, 100_000);
    for i in 0..100 {
        let id = mgr.begin(&format!("cmd_{}", i), ".", BlockSource::Shell);
        mgr.append(id, vec![BlockLine::normal(format!("output {}", i))]);
        mgr.finish(id, Some(0), Duration::from_millis(i as u64));
    }
    assert_eq!(mgr.len(), 100);
    assert_eq!(mgr.total_lines(), 100);
}

#[test]
fn test_unicode_content() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("echo 你好世界", "/home/用户", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("你好世界"),
        BlockLine::info("📡 Réseau connecté — données prêtes"),
        BlockLine::success("✅ Тест пройден"),
    ]);
    mgr.finish(id, Some(0), Duration::from_millis(1));

    let block = mgr.get(id).unwrap();
    assert_eq!(block.output.len(), 3);
    assert_eq!(block.command, "echo 你好世界");

    let hits = mgr.search("你好");
    assert!(!hits.is_empty());
}

#[test]
fn test_empty_command() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("", ".", BlockSource::Shell);
    assert_eq!(mgr.get(id).unwrap().command, "");
}

#[test]
fn test_very_long_output() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("big output", ".", BlockSource::Shell);
    let lines: Vec<BlockLine> = (0..1000)
        .map(|i| BlockLine::normal(format!("line {}: {}", i, "x".repeat(100))))
        .collect();
    mgr.append(id, lines);

    assert_eq!(mgr.get(id).unwrap().line_count(), 1000);
    assert_eq!(mgr.total_lines(), 1000);
}

#[test]
fn test_interleaved_blocks() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("job1", ".", BlockSource::Shell);
    let id2 = mgr.begin("job2", ".", BlockSource::Shell);

    mgr.append_line(id1, BlockLine::normal("job1 line 1"));
    mgr.append_line(id2, BlockLine::normal("job2 line 1"));
    mgr.append_line(id1, BlockLine::normal("job1 line 2"));
    mgr.append_line(id2, BlockLine::normal("job2 line 2"));

    mgr.finish(id2, Some(0), Duration::from_millis(100));
    mgr.finish(id1, Some(0), Duration::from_millis(200));

    assert_eq!(mgr.get(id1).unwrap().line_count(), 2);
    assert_eq!(mgr.get(id2).unwrap().line_count(), 2);
    assert!(!mgr.get(id1).unwrap().running);
    assert!(!mgr.get(id2).unwrap().running);
}