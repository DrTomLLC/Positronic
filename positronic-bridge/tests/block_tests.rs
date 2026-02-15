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
    assert_eq!(BlockLine::classify("⚠ Caution required").kind, LineKind::Warning);
}

#[test]
fn test_classify_warning_warn() {
    assert_eq!(BlockLine::classify("WARN: disk space low").kind, LineKind::Warning);
}

#[test]
fn test_classify_success_checkmark() {
    assert_eq!(BlockLine::classify("✅ All tests passed").kind, LineKind::Success);
}

#[test]
fn test_classify_success_ok() {
    assert_eq!(BlockLine::classify("ok 42 tests").kind, LineKind::Success);
}

#[test]
fn test_classify_success_compiling() {
    assert_eq!(BlockLine::classify("Compiling myapp v0.1.0").kind, LineKind::Success);
}

#[test]
fn test_classify_success_finished() {
    assert_eq!(BlockLine::classify("Finished release [optimized]").kind, LineKind::Success);
}

#[test]
fn test_classify_success_passed() {
    assert_eq!(BlockLine::classify("PASSED all checks").kind, LineKind::Success);
}

#[test]
fn test_classify_info_lightbulb() {
    assert_eq!(BlockLine::classify("💡 Did you mean: ls?").kind, LineKind::Info);
}

#[test]
fn test_classify_info_brain() {
    assert_eq!(BlockLine::classify("🧠 Analyzing...").kind, LineKind::Info);
}

#[test]
fn test_classify_info_satellite() {
    assert_eq!(BlockLine::classify("📡 Connected to server").kind, LineKind::Info);
}

#[test]
fn test_classify_info_plug() {
    assert_eq!(BlockLine::classify("🔌 Device attached").kind, LineKind::Info);
}

#[test]
fn test_classify_info_uppercase() {
    assert_eq!(BlockLine::classify("INFO: starting daemon").kind, LineKind::Info);
}

#[test]
fn test_classify_info_note() {
    assert_eq!(BlockLine::classify("note: consider adding a derive").kind, LineKind::Info);
}

#[test]
fn test_classify_normal_fallback() {
    assert_eq!(BlockLine::classify("hello world").kind, LineKind::Normal);
}

#[test]
fn test_classify_normal_empty() {
    assert_eq!(BlockLine::classify("").kind, LineKind::Normal);
}

#[test]
fn test_classify_preserves_leading_whitespace() {
    let line = BlockLine::classify("  error: indented error");
    assert_eq!(line.kind, LineKind::Error);
    assert_eq!(line.text, "  error: indented error");
}

// ============================================================================
// BlockLine::is_blank Tests
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
// BlockLine Display Tests
// ============================================================================

#[test]
fn test_blockline_display() {
    let line = BlockLine::normal("output text");
    assert_eq!(format!("{}", line), "output text");
}

// ============================================================================
// LineKind Display Tests
// ============================================================================

#[test]
fn test_linekind_display() {
    assert_eq!(format!("{}", LineKind::Normal), "normal");
    assert_eq!(format!("{}", LineKind::Error), "error");
    assert_eq!(format!("{}", LineKind::Warning), "warning");
    assert_eq!(format!("{}", LineKind::Success), "success");
    assert_eq!(format!("{}", LineKind::Info), "info");
    assert_eq!(format!("{}", LineKind::Muted), "muted");
}

// ============================================================================
// BlockSource Display Tests
// ============================================================================

#[test]
fn test_blocksource_display() {
    assert_eq!(format!("{}", BlockSource::Shell), "shell");
    assert_eq!(format!("{}", BlockSource::Native), "native");
    assert_eq!(format!("{}", BlockSource::Neural), "neural");
    assert_eq!(format!("{}", BlockSource::System), "system");
}

// ============================================================================
// BlockManager Creation Tests
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
    assert_eq!(mgr.total_lines(), 0);
}

// ============================================================================
// BlockManager Lifecycle Tests
// ============================================================================

#[test]
fn test_manager_begin_creates_running_block() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("cargo test", "/home/user/project", BlockSource::Shell);

    let block = mgr.get(id).unwrap();
    assert!(block.running);
    assert_eq!(block.command, "cargo test");
    assert_eq!(block.cwd, "/home/user/project");
    assert_eq!(block.source, BlockSource::Shell);
    assert!(block.output.is_empty());
    assert!(block.exit_code.is_none());
    assert!(block.duration.is_none());
    assert!(!block.collapsed);
}

#[test]
fn test_manager_begin_increments_ids() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("cmd1", ".", BlockSource::Shell);
    let id2 = mgr.begin("cmd2", ".", BlockSource::Shell);
    let id3 = mgr.begin("cmd3", ".", BlockSource::Shell);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn test_manager_append_adds_output() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("echo test", ".", BlockSource::Shell);

    mgr.append(id, vec![
        BlockLine::normal("line 1"),
        BlockLine::normal("line 2"),
    ]);

    let block = mgr.get(id).unwrap();
    assert_eq!(block.output.len(), 2);
    assert_eq!(block.output[0].text, "line 1");
    assert_eq!(block.output[1].text, "line 2");
}

#[test]
fn test_manager_append_line_single() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("echo test", ".", BlockSource::Shell);

    mgr.append_line(id, BlockLine::normal("single line"));

    let block = mgr.get(id).unwrap();
    assert_eq!(block.output.len(), 1);
    assert_eq!(block.output[0].text, "single line");
}

#[test]
fn test_manager_append_to_nonexistent_id() {
    let mut mgr = BlockManager::default();
    // Appending to a block that doesn't exist should be a no-op
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
    assert_eq!(mgr.len(), 1);

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
// BlockManager Retrieval Tests
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
    let id = mgr.begin("test", ".", BlockSource::Native);

    let block = mgr.get_mut(id).unwrap();
    block.collapsed = true;

    assert!(mgr.get(id).unwrap().collapsed);
}

// ============================================================================
// BlockManager Remove Tests
// ============================================================================

#[test]
fn test_manager_remove_existing() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("to remove", ".", BlockSource::Shell);
    mgr.begin("to keep", ".", BlockSource::Shell);

    let removed = mgr.remove(id);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().command, "to remove");
    assert_eq!(mgr.len(), 1);
    assert_eq!(mgr.blocks()[0].command, "to keep");
}

#[test]
fn test_manager_remove_nonexistent() {
    let mut mgr = BlockManager::default();
    mgr.begin("test", ".", BlockSource::Shell);

    let removed = mgr.remove(999);
    assert!(removed.is_none());
    assert_eq!(mgr.len(), 1);
}

#[test]
fn test_manager_remove_only_block() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("lonely", ".", BlockSource::Shell);

    mgr.remove(id);
    assert!(mgr.is_empty());
}

// ============================================================================
// BlockManager Collapse/Expand Tests
// ============================================================================

#[test]
fn test_manager_toggle_collapse() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Native);
    assert!(!mgr.get(id).unwrap().collapsed);

    mgr.toggle_collapse(id);
    assert!(mgr.get(id).unwrap().collapsed);

    mgr.toggle_collapse(id);
    assert!(!mgr.get(id).unwrap().collapsed);
}

#[test]
fn test_manager_collapse_all() {
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
fn test_manager_expand_all() {
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
fn test_collapse_then_expand_round_trip() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("a", ".", BlockSource::Shell);
    let id2 = mgr.begin("b", ".", BlockSource::Shell);

    // Collapse one, collapse all, expand all
    mgr.toggle_collapse(id1);
    mgr.collapse_all();
    mgr.expand_all();

    assert!(!mgr.get(id1).unwrap().collapsed);
    assert!(!mgr.get(id2).unwrap().collapsed);
}

// ============================================================================
// BlockManager Filter Tests
// ============================================================================

#[test]
fn test_filter_by_source() {
    let mut mgr = BlockManager::default();
    mgr.begin("shell cmd", ".", BlockSource::Shell);
    mgr.begin("native cmd", ".", BlockSource::Native);
    mgr.begin("neural cmd", ".", BlockSource::Neural);
    mgr.begin("system msg", ".", BlockSource::System);
    mgr.begin("shell cmd 2", ".", BlockSource::Shell);

    let shell_blocks = mgr.filter_by_source(BlockSource::Shell);
    assert_eq!(shell_blocks.len(), 2);

    let neural_blocks = mgr.filter_by_source(BlockSource::Neural);
    assert_eq!(neural_blocks.len(), 1);
    assert_eq!(neural_blocks[0].command, "neural cmd");
}

#[test]
fn test_filter_by_source_none_found() {
    let mut mgr = BlockManager::default();
    mgr.begin("shell cmd", ".", BlockSource::Shell);

    let neural = mgr.filter_by_source(BlockSource::Neural);
    assert!(neural.is_empty());
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

    let successes = mgr.filter_by_exit_code(0);
    assert_eq!(successes.len(), 2);

    let failures = mgr.filter_by_exit_code(1);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].command, "fail");
}

#[test]
fn test_running_blocks() {
    let mut mgr = BlockManager::default();
    mgr.begin("running1", ".", BlockSource::Shell);
    let id2 = mgr.begin("done", ".", BlockSource::Shell);
    mgr.finish(id2, Some(0), Duration::from_millis(1));
    mgr.begin("running2", ".", BlockSource::Shell);

    let running = mgr.running_blocks();
    assert_eq!(running.len(), 2);
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
// BlockManager Search Tests
// ============================================================================

#[test]
fn test_search_in_command() {
    let mut mgr = BlockManager::default();
    mgr.begin("cargo build --release", ".", BlockSource::Shell);
    mgr.begin("ls -la", ".", BlockSource::Shell);

    let hits = mgr.search("cargo");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].line_index.is_none()); // hit was in command, not output
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
    // Should hit both commands
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
    // Empty string matches everything
    assert!(!hits.is_empty());
}

// ============================================================================
// BlockManager Copy Tests
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

// ============================================================================
// BlockManager Export Tests
// ============================================================================

#[test]
fn test_export_all_empty() {
    let mgr = BlockManager::default();
    let exported = mgr.export_all();
    assert!(exported.is_empty());
}

#[test]
fn test_export_all_single_block() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("echo hi", "/home", BlockSource::Shell);
    mgr.append(id, vec![BlockLine::normal("hi")]);
    mgr.finish(id, Some(0), Duration::from_millis(5));

    let exported = mgr.export_all();
    assert!(exported.contains("$ echo hi"));
    assert!(exported.contains("[shell]"));
    assert!(exported.contains("(/home)"));
    assert!(exported.contains("hi"));
    assert!(exported.contains("[exit 0]"));
}

#[test]
fn test_export_all_multiple_blocks() {
    let mut mgr = BlockManager::default();
    let id1 = mgr.begin("cmd1", ".", BlockSource::Shell);
    mgr.finish(id1, Some(0), Duration::from_millis(1));

    let id2 = mgr.begin("cmd2", ".", BlockSource::Native);
    mgr.finish(id2, Some(0), Duration::from_millis(1));

    let exported = mgr.export_all();
    assert!(exported.contains("cmd1"));
    assert!(exported.contains("cmd2"));
    assert!(exported.contains("────")); // Separator between blocks
}

// ============================================================================
// BlockManager Limits Tests
// ============================================================================

#[test]
fn test_enforce_block_count_limit() {
    let mut mgr = BlockManager::new(3, 100_000);
    for i in 0..5 {
        mgr.begin(&format!("cmd {}", i), ".", BlockSource::Shell);
    }
    assert_eq!(mgr.len(), 3);
    // Oldest blocks pruned: cmd 0, cmd 1 removed, cmd 2, 3, 4 remain
    assert_eq!(mgr.blocks()[0].command, "cmd 2");
}

#[test]
fn test_enforce_line_count_limit() {
    let mut mgr = BlockManager::new(1000, 10);

    // First block with 6 lines
    let id1 = mgr.begin("big1", ".", BlockSource::Shell);
    mgr.append(id1, (0..6).map(|i| BlockLine::normal(format!("line {}", i))).collect());

    // Second block with 6 lines — total now 12, over limit of 10
    let id2 = mgr.begin("big2", ".", BlockSource::Shell);
    mgr.append(id2, (0..6).map(|i| BlockLine::normal(format!("line {}", i))).collect());

    // Third block triggers enforcement
    mgr.begin("small", ".", BlockSource::Shell);

    // The oldest block(s) should have been pruned to get under 10 lines
    assert!(mgr.total_lines() <= 10 || mgr.len() <= 2);
}

#[test]
fn test_limits_preserve_at_least_one_block() {
    let mut mgr = BlockManager::new(1, 5);
    let id = mgr.begin("only", ".", BlockSource::Shell);
    mgr.append(id, (0..20).map(|i| BlockLine::normal(format!("line {}", i))).collect());

    // Even though over line limit, the one block should still exist
    assert_eq!(mgr.len(), 1);
}

// ============================================================================
// BlockManager Clear Tests
// ============================================================================

#[test]
fn test_clear() {
    let mut mgr = BlockManager::default();
    mgr.begin("a", ".", BlockSource::Shell);
    mgr.begin("b", ".", BlockSource::Shell);
    mgr.begin("c", ".", BlockSource::Shell);

    mgr.clear();
    assert!(mgr.is_empty());
    assert_eq!(mgr.len(), 0);
    assert!(mgr.latest().is_none());
}

// ============================================================================
// BlockManager Stats Tests
// ============================================================================

#[test]
fn test_stats_empty() {
    let mgr = BlockManager::default();
    let stats = mgr.stats();
    assert_eq!(stats.total_blocks, 0);
    assert_eq!(stats.total_lines, 0);
    assert_eq!(stats.running, 0);
    assert_eq!(stats.errors, 0);
}

#[test]
fn test_stats_mixed() {
    let mut mgr = BlockManager::default();

    // Running block with output
    let id1 = mgr.begin("running", ".", BlockSource::Shell);
    mgr.append(id1, vec![BlockLine::normal("line1"), BlockLine::normal("line2")]);

    // Successful block
    let id2 = mgr.begin("ok", ".", BlockSource::Shell);
    mgr.append(id2, vec![BlockLine::normal("line3")]);
    mgr.finish(id2, Some(0), Duration::from_millis(1));

    // Failed block
    let id3 = mgr.begin("fail", ".", BlockSource::Shell);
    mgr.append(id3, vec![BlockLine::error("err1"), BlockLine::error("err2")]);
    mgr.finish(id3, Some(1), Duration::from_millis(1));

    let stats = mgr.stats();
    assert_eq!(stats.total_blocks, 3);
    assert_eq!(stats.total_lines, 5);
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
// TerminalBlock Method Tests
// ============================================================================

#[test]
fn test_block_line_count() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("test", ".", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("a"),
        BlockLine::normal("b"),
        BlockLine::normal("c"),
    ]);

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
    assert_eq!(errors[0].text, "E0308");
}

#[test]
fn test_block_warning_lines() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("build", ".", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("ok"),
        BlockLine::warning("unused var"),
    ]);

    let warnings = mgr.get(id).unwrap().warning_lines();
    assert_eq!(warnings.len(), 1);
}

// ============================================================================
// TerminalBlock Display Tests
// ============================================================================

#[test]
fn test_block_display_running() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("cargo test", ".", BlockSource::Shell);
    mgr.append(id, vec![BlockLine::normal("a"), BlockLine::normal("b")]);

    let display = format!("{}", mgr.get(id).unwrap());
    assert!(display.contains("cargo test"));
    assert!(display.contains("shell"));
    assert!(display.contains("2 lines"));
    assert!(display.contains("running"));
}

#[test]
fn test_block_display_success() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("ls", ".", BlockSource::Native);
    mgr.finish(id, Some(0), Duration::from_millis(1));

    let display = format!("{}", mgr.get(id).unwrap());
    assert!(display.contains("✅ ok"));
}

#[test]
fn test_block_display_failure() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("make", ".", BlockSource::Shell);
    mgr.finish(id, Some(2), Duration::from_millis(1));

    let display = format!("{}", mgr.get(id).unwrap());
    assert!(display.contains("❌ exit 2"));
}

// ============================================================================
// Quick Block Helper Tests
// ============================================================================

#[test]
fn test_quick_block_basic() {
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
    assert_eq!(block.exit_code, Some(0));
    assert_eq!(block.output.len(), 1);
    assert_eq!(block.source, BlockSource::Native);
}

#[test]
fn test_quick_block_classifies_lines() {
    let mut mgr = BlockManager::default();
    let id = quick_block(
        &mut mgr,
        "build",
        ".",
        vec![
            "Compiling myapp".to_string(),
            "error: failed".to_string(),
            "warning: unused".to_string(),
        ],
        BlockSource::Shell,
    );

    let block = mgr.get(id).unwrap();
    assert_eq!(block.output[0].kind, LineKind::Success); // Compiling
    assert_eq!(block.output[1].kind, LineKind::Error);
    assert_eq!(block.output[2].kind, LineKind::Warning);
}

#[test]
fn test_quick_block_empty_lines() {
    let mut mgr = BlockManager::default();
    let id = quick_block(&mut mgr, "noop", ".", vec![], BlockSource::System);
    let block = mgr.get(id).unwrap();
    assert!(block.output.is_empty());
    assert!(!block.running);
}

#[test]
fn test_quick_error_block() {
    let mut mgr = BlockManager::default();
    let id = quick_error_block(
        &mut mgr,
        "bad_cmd",
        "/home",
        "command not found: bad_cmd",
        127,
    );

    let block = mgr.get(id).unwrap();
    assert!(!block.running);
    assert_eq!(block.exit_code, Some(127));
    assert!(block.failed());
    assert_eq!(block.output.len(), 1);
    assert_eq!(block.output[0].kind, LineKind::Error);
    assert_eq!(block.output[0].text, "command not found: bad_cmd");
}

// ============================================================================
// format_duration Tests
// ============================================================================

#[test]
fn test_format_duration_millis() {
    assert_eq!(format_duration(Duration::from_millis(42)), "42ms");
    assert_eq!(format_duration(Duration::from_millis(0)), "0ms");
    assert_eq!(format_duration(Duration::from_millis(999)), "999ms");
}

#[test]
fn test_format_duration_seconds() {
    assert_eq!(format_duration(Duration::from_secs(1)), "1.000s");
    assert_eq!(format_duration(Duration::from_millis(1500)), "1.500s");
    assert_eq!(format_duration(Duration::from_secs(59)), "59.000s");
}

#[test]
fn test_format_duration_minutes() {
    assert_eq!(format_duration(Duration::from_secs(60)), "1m 00s");
    assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
    assert_eq!(format_duration(Duration::from_secs(3599)), "59m 59s");
}

#[test]
fn test_format_duration_hours() {
    assert_eq!(format_duration(Duration::from_secs(3600)), "1h 00m");
    assert_eq!(format_duration(Duration::from_secs(7200)), "2h 00m");
    assert_eq!(format_duration(Duration::from_secs(5400)), "1h 30m");
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_blockline_serialize_deserialize() {
    let line = BlockLine::error("error: mismatched types");
    let json = serde_json::to_string(&line).unwrap();
    let deserialized: BlockLine = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.text, "error: mismatched types");
    assert_eq!(deserialized.kind, LineKind::Error);
}

#[test]
fn test_linekind_serialize_deserialize() {
    for kind in &[
        LineKind::Normal, LineKind::Error, LineKind::Warning,
        LineKind::Success, LineKind::Info, LineKind::Muted,
    ] {
        let json = serde_json::to_string(kind).unwrap();
        let deserialized: LineKind = serde_json::from_str(&json).unwrap();
        assert_eq!(*kind, deserialized);
    }
}

#[test]
fn test_blocksource_serialize_deserialize() {
    for source in &[
        BlockSource::Shell, BlockSource::Native,
        BlockSource::Neural, BlockSource::System,
    ] {
        let json = serde_json::to_string(source).unwrap();
        let deserialized: BlockSource = serde_json::from_str(&json).unwrap();
        assert_eq!(*source, deserialized);
    }
}

#[test]
fn test_terminal_block_serialize_deserialize() {
    let mut mgr = BlockManager::default();
    let id = mgr.begin("cargo build", "/project", BlockSource::Shell);
    mgr.append(id, vec![
        BlockLine::normal("Compiling..."),
        BlockLine::success("Finished release"),
    ]);
    mgr.finish(id, Some(0), Duration::from_millis(250));

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
fn test_terminal_block_serialize_running_no_duration() {
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
fn test_search_hit_serialize_deserialize() {
    let hit = SearchHit {
        block_id: 42,
        line_index: Some(7),
        context: "error[E0308]".to_string(),
    };
    let json = serde_json::to_string(&hit).unwrap();
    let deserialized: SearchHit = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.block_id, 42);
    assert_eq!(deserialized.line_index, Some(7));
    assert_eq!(deserialized.context, "error[E0308]");
}

#[test]
fn test_block_stats_serialize_deserialize() {
    let stats = BlockStats {
        total_blocks: 100,
        total_lines: 5000,
        running: 3,
        errors: 7,
    };
    let json = serde_json::to_string(&stats).unwrap();
    let deserialized: BlockStats = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.total_blocks, 100);
    assert_eq!(deserialized.errors, 7);
}

// ============================================================================
// Edge Case / Stress Tests
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
    let block = mgr.get(id).unwrap();
    assert_eq!(block.command, "");
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

    // Start two blocks simultaneously (simulating parallel commands)
    let id1 = mgr.begin("job1", ".", BlockSource::Shell);
    let id2 = mgr.begin("job2", ".", BlockSource::Shell);

    // Interleave output
    mgr.append_line(id1, BlockLine::normal("job1 line 1"));
    mgr.append_line(id2, BlockLine::normal("job2 line 1"));
    mgr.append_line(id1, BlockLine::normal("job1 line 2"));
    mgr.append_line(id2, BlockLine::normal("job2 line 2"));

    // Finish in reverse order
    mgr.finish(id2, Some(0), Duration::from_millis(100));
    mgr.finish(id1, Some(0), Duration::from_millis(200));

    assert_eq!(mgr.get(id1).unwrap().line_count(), 2);
    assert_eq!(mgr.get(id2).unwrap().line_count(), 2);
    assert!(!mgr.get(id1).unwrap().running);
    assert!(!mgr.get(id2).unwrap().running);
}