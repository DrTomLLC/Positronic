// positronic-bridge/tests/input_tests.rs
//
// Integration tests for the Intelli-Input editor (Pillar II).
// Tests all public API surface of input/mod.rs: cursor movement,
// word navigation, selection, editing, kill/yank, undo/redo,
// history, modes, vim, and edge cases.

use positronic_bridge::input::{EditMode, InputEditor, Selection, VimMode};

// ============================================================================
// Construction & Defaults
// ============================================================================

#[test]
fn test_new_editor_is_empty() {
    let ed = InputEditor::new();
    assert!(ed.is_empty());
    assert_eq!(ed.len(), 0);
    assert_eq!(ed.char_count(), 0);
    assert_eq!(ed.cursor(), 0);
    assert!(ed.at_start());
    assert!(ed.at_end());
    assert_eq!(ed.value, "");
}

#[test]
fn test_default_is_same_as_new() {
    let ed = InputEditor::default();
    assert!(ed.is_empty());
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_default_modes() {
    let ed = InputEditor::new();
    assert_eq!(ed.edit_mode(), EditMode::Insert);
    assert_eq!(ed.vim_mode(), VimMode::Disabled);
    assert!(ed.selection().is_none());
    assert!(ed.history().is_empty());
    assert!(ed.kill_ring().is_empty());
}

#[test]
fn test_display_empty() {
    let ed = InputEditor::new();
    assert_eq!(format!("{}", ed), "");
}

#[test]
fn test_display_with_content() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    assert_eq!(format!("{}", ed), "hello world");
}

#[test]
fn test_debug_display_shows_cursor() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_left();
    assert_eq!(ed.debug_display(), "ab|c");
}

// ============================================================================
// Character-Level Cursor Movement
// ============================================================================

#[test]
fn test_move_left_from_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_left();
    assert_eq!(ed.cursor(), 2);
    assert!(!ed.at_end());
}

#[test]
fn test_move_left_at_start_stays() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.move_left();
    assert_eq!(ed.cursor(), 0);
    assert!(ed.at_start());
}

#[test]
fn test_move_right_from_start() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.move_right();
    assert_eq!(ed.cursor(), 1);
}

#[test]
fn test_move_right_at_end_stays() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_right();
    assert_eq!(ed.cursor(), 3);
    assert!(ed.at_end());
}

#[test]
fn test_move_home() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    assert_eq!(ed.cursor(), 5);
    ed.move_home();
    assert_eq!(ed.cursor(), 0);
    assert!(ed.at_start());
}

#[test]
fn test_move_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.move_home();
    ed.move_end();
    assert_eq!(ed.cursor(), 5);
    assert!(ed.at_end());
}

#[test]
fn test_set_cursor() {
    let mut ed = InputEditor::new();
    ed.insert_str("abcdef");
    ed.set_cursor(3);
    assert_eq!(ed.cursor(), 3);
}

#[test]
fn test_set_cursor_clamped() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.set_cursor(999);
    assert_eq!(ed.cursor(), 3); // clamped to len
}

// ============================================================================
// Word-Level Cursor Movement
// ============================================================================

#[test]
fn test_move_word_right() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world foo");
    ed.move_home();
    ed.move_word_right();
    assert_eq!(ed.cursor(), 6); // after "hello "
}

#[test]
fn test_move_word_right_multiple() {
    let mut ed = InputEditor::new();
    ed.insert_str("aaa bbb ccc");
    ed.move_home();
    ed.move_word_right(); // past "aaa "
    ed.move_word_right(); // past "bbb "
    assert_eq!(ed.cursor(), 8);
}

#[test]
fn test_move_word_right_at_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.move_word_right();
    assert_eq!(ed.cursor(), 5); // stays at end
}

#[test]
fn test_move_word_left() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.move_word_left();
    assert_eq!(ed.cursor(), 6); // before "world"
}

#[test]
fn test_move_word_left_multiple() {
    let mut ed = InputEditor::new();
    ed.insert_str("aaa bbb ccc");
    ed.move_word_left(); // before "ccc"
    ed.move_word_left(); // before "bbb"
    assert_eq!(ed.cursor(), 4);
}

#[test]
fn test_move_word_left_at_start() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.move_home();
    ed.move_word_left();
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_word_navigation_round_trip() {
    let mut ed = InputEditor::new();
    ed.insert_str("cargo build --release");
    ed.move_home();
    ed.move_word_right(); // past "cargo "
    ed.move_word_right(); // past "build "
    ed.move_word_left(); // back to "build"
    // cursor should be at "build"
    assert_eq!(&ed.value[ed.cursor()..ed.cursor() + 5], "build");
}

// ============================================================================
// Character Insertion
// ============================================================================

#[test]
fn test_insert_char() {
    let mut ed = InputEditor::new();
    ed.insert_char('a');
    ed.insert_char('b');
    ed.insert_char('c');
    assert_eq!(ed.value, "abc");
    assert_eq!(ed.cursor(), 3);
}

#[test]
fn test_insert_char_midpoint() {
    let mut ed = InputEditor::new();
    ed.insert_str("ac");
    ed.move_left(); // cursor before 'c'
    ed.insert_char('b');
    assert_eq!(ed.value, "abc");
    assert_eq!(ed.cursor(), 2); // after 'b'
}

#[test]
fn test_insert_str() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    assert_eq!(ed.value, "hello world");
    assert_eq!(ed.cursor(), 11);
}

#[test]
fn test_insert_str_empty() {
    let mut ed = InputEditor::new();
    ed.insert_str("");
    assert!(ed.is_empty());
}

#[test]
fn test_insert_str_midpoint() {
    let mut ed = InputEditor::new();
    ed.insert_str("hd");
    ed.move_left(); // before 'd'
    ed.insert_str("ello worl");
    assert_eq!(ed.value, "hello world");
}

// ============================================================================
// Backspace & Delete
// ============================================================================

#[test]
fn test_backspace() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.backspace();
    assert_eq!(ed.value, "hell");
    assert_eq!(ed.cursor(), 4);
}

#[test]
fn test_backspace_at_start_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.backspace();
    assert_eq!(ed.value, "abc");
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_backspace_empty_is_noop() {
    let mut ed = InputEditor::new();
    ed.backspace();
    assert!(ed.is_empty());
}

#[test]
fn test_delete() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.delete();
    assert_eq!(ed.value, "bc");
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_delete_at_end_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.delete();
    assert_eq!(ed.value, "abc");
}

#[test]
fn test_delete_midpoint() {
    let mut ed = InputEditor::new();
    ed.insert_str("abcd");
    ed.set_cursor(2);
    ed.delete();
    assert_eq!(ed.value, "abd");
    assert_eq!(ed.cursor(), 2);
}

#[test]
fn test_clear() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.clear();
    assert!(ed.is_empty());
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_clear_empty_is_noop() {
    let mut ed = InputEditor::new();
    ed.clear(); // should not panic or push undo
    assert!(!ed.can_undo());
}

// ============================================================================
// Selection Tests
// ============================================================================

#[test]
fn test_select_right() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.move_home();
    ed.select_right();
    ed.select_right();
    ed.select_right();

    let sel = ed.selection().unwrap();
    assert_eq!(sel.range(), (0, 3));
    assert_eq!(ed.selected_text(), Some("hel"));
}

#[test]
fn test_select_left() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    // cursor at 5
    ed.select_left();
    ed.select_left();

    let sel = ed.selection().unwrap();
    assert_eq!(sel.range(), (3, 5));
    assert_eq!(ed.selected_text(), Some("lo"));
}

#[test]
fn test_select_home() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.select_home();

    assert_eq!(ed.selected_text(), Some("hello"));
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_select_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.move_home();
    ed.select_end();

    assert_eq!(ed.selected_text(), Some("hello"));
}

#[test]
fn test_select_all() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.select_all();

    assert_eq!(ed.selected_text(), Some("hello world"));
}

#[test]
fn test_select_all_empty() {
    let mut ed = InputEditor::new();
    ed.select_all();
    assert!(ed.selection().is_none());
}

#[test]
fn test_select_word_right() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.move_home();
    ed.select_word_right();

    let text = ed.selected_text().unwrap();
    assert!(text.starts_with("hello"));
}

#[test]
fn test_select_word_left() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.select_word_left();

    assert_eq!(ed.selected_text(), Some("world"));
}

#[test]
fn test_deselect() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.select_all();
    assert!(ed.selection().is_some());
    ed.deselect();
    assert!(ed.selection().is_none());
}

#[test]
fn test_move_clears_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.select_all();
    ed.move_left(); // should clear selection
    assert!(ed.selection().is_none());
}

#[test]
fn test_selected_text_none_when_no_selection() {
    let ed = InputEditor::new();
    assert!(ed.selected_text().is_none());
}

// ============================================================================
// Selection + Editing Integration
// ============================================================================

#[test]
fn test_insert_replaces_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.select_all();
    ed.insert_char('X');
    assert_eq!(ed.value, "X");
    assert_eq!(ed.cursor(), 1);
}

#[test]
fn test_insert_str_replaces_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.move_home();
    ed.select_word_right(); // select "hello "
    ed.insert_str("hi ");
    assert!(ed.value.starts_with("hi "));
}

#[test]
fn test_backspace_deletes_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.select_all();
    ed.backspace();
    assert!(ed.is_empty());
}

#[test]
fn test_delete_deletes_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("abcdef");
    ed.move_home();
    ed.select_right();
    ed.select_right(); // select "ab"
    ed.delete();
    assert_eq!(ed.value, "cdef");
}

#[test]
fn test_cut_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.move_home();
    ed.select_right();
    ed.select_right();
    ed.select_right();
    ed.select_right();
    ed.select_right(); // select "hello"

    let cut = ed.cut_selection().unwrap();
    assert_eq!(cut, "hello");
    assert_eq!(ed.value, " world");
}

#[test]
fn test_cut_no_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    assert!(ed.cut_selection().is_none());
}

// ============================================================================
// Kill/Yank Tests
// ============================================================================

#[test]
fn test_kill_to_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.set_cursor(5);
    ed.kill_to_end();
    assert_eq!(ed.value, "hello");
    assert_eq!(ed.cursor(), 5);
    assert_eq!(ed.kill_ring().last().unwrap(), " world");
}

#[test]
fn test_kill_to_end_at_end_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.kill_to_end();
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_kill_to_start() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.set_cursor(5);
    ed.kill_to_start();
    assert_eq!(ed.value, " world");
    assert_eq!(ed.cursor(), 0);
    assert_eq!(ed.kill_ring().last().unwrap(), "hello");
}

#[test]
fn test_kill_to_start_at_start_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.move_home();
    ed.kill_to_start();
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_kill_word_back() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.kill_word_back();
    assert_eq!(ed.value, "hello ");
    assert_eq!(ed.kill_ring().last().unwrap(), "world");
}

#[test]
fn test_kill_word_back_at_start_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.move_home();
    ed.kill_word_back();
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_kill_word_forward() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.move_home();
    ed.kill_word_forward();
    assert_eq!(ed.value, "world");
}

#[test]
fn test_kill_word_forward_at_end_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.kill_word_forward();
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_yank() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.kill_word_back(); // kills "world", leaves "hello "
    ed.move_home();
    ed.yank(); // pastes "world" at beginning
    assert!(ed.value.starts_with("world"));
}

#[test]
fn test_yank_empty_ring() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.yank(); // should be a no-op
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_kill_ring_stacks() {
    let mut ed = InputEditor::new();
    ed.insert_str("aaa bbb ccc");
    ed.kill_word_back(); // kills "ccc"
    ed.kill_word_back(); // kills "bbb "
    // Now kill ring should have both entries
    assert_eq!(ed.kill_ring().len(), 2);
}

#[test]
fn test_kill_ring_max_size() {
    let mut ed = InputEditor::new();
    // Generate 15 kills — ring should cap at 10
    for i in 0..15 {
        ed.insert_str(&format!("word{} ", i));
        ed.kill_word_back();
    }
    assert!(ed.kill_ring().len() <= 10);
}

// ============================================================================
// Undo/Redo Tests
// ============================================================================

#[test]
fn test_undo_insert() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    assert!(ed.can_undo());
    ed.undo();
    assert!(ed.is_empty());
}

#[test]
fn test_redo_after_undo() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.undo();
    assert!(ed.can_redo());
    ed.redo();
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_undo_multiple() {
    let mut ed = InputEditor::new();
    ed.insert_char('a');
    ed.insert_char('b');
    ed.insert_char('c');
    assert_eq!(ed.value, "abc");

    ed.undo();
    assert_eq!(ed.value, "ab");
    ed.undo();
    assert_eq!(ed.value, "a");
    ed.undo();
    assert_eq!(ed.value, "");
}

#[test]
fn test_undo_backspace() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.backspace();
    assert_eq!(ed.value, "hell");
    ed.undo();
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_undo_kill() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.kill_to_end();
    // kill_to_end at end is a noop... let's go from middle
    ed.undo(); // undo kill_to_end... but wait, kill was noop here
    // redo the test properly:
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.set_cursor(5);
    ed.kill_to_end();
    assert_eq!(ed.value, "hello");
    ed.undo();
    assert_eq!(ed.value, "hello world");
}

#[test]
fn test_redo_cleared_on_new_edit() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.undo();
    assert!(ed.can_redo());
    ed.insert_char('x'); // new edit should clear redo stack
    assert!(!ed.can_redo());
}

#[test]
fn test_undo_empty_is_noop() {
    let mut ed = InputEditor::new();
    ed.undo(); // should not panic
    assert!(ed.is_empty());
}

#[test]
fn test_redo_empty_is_noop() {
    let mut ed = InputEditor::new();
    ed.redo();
    assert!(ed.is_empty());
}

#[test]
fn test_undo_preserves_cursor() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    let saved_cursor = ed.cursor();
    ed.backspace();
    ed.undo();
    assert_eq!(ed.cursor(), saved_cursor);
}

// ============================================================================
// Command History Tests
// ============================================================================

#[test]
fn test_push_history() {
    let mut ed = InputEditor::new();
    ed.push_history("cargo build");
    ed.push_history("cargo test");
    assert_eq!(ed.history().len(), 2);
}

#[test]
fn test_push_history_deduplicates_consecutive() {
    let mut ed = InputEditor::new();
    ed.push_history("ls");
    ed.push_history("ls");
    ed.push_history("ls");
    assert_eq!(ed.history().len(), 1);
}

#[test]
fn test_push_history_ignores_empty() {
    let mut ed = InputEditor::new();
    ed.push_history("");
    ed.push_history("   ");
    assert!(ed.history().is_empty());
}

#[test]
fn test_history_up() {
    let mut ed = InputEditor::new();
    ed.push_history("first");
    ed.push_history("second");
    ed.push_history("third");

    ed.insert_str("current");
    ed.history_up();
    assert_eq!(ed.value, "third");
    ed.history_up();
    assert_eq!(ed.value, "second");
    ed.history_up();
    assert_eq!(ed.value, "first");
}

#[test]
fn test_history_up_at_oldest_stays() {
    let mut ed = InputEditor::new();
    ed.push_history("only");
    ed.history_up();
    assert_eq!(ed.value, "only");
    ed.history_up(); // should stay
    assert_eq!(ed.value, "only");
}

#[test]
fn test_history_down() {
    let mut ed = InputEditor::new();
    ed.push_history("first");
    ed.push_history("second");

    ed.insert_str("typing");
    ed.history_up(); // second
    ed.history_up(); // first
    ed.history_down(); // second
    assert_eq!(ed.value, "second");
    ed.history_down(); // back to stashed "typing"
    assert_eq!(ed.value, "typing");
}

#[test]
fn test_history_down_at_bottom_is_noop() {
    let mut ed = InputEditor::new();
    ed.push_history("cmd");
    ed.history_down(); // no cursor, should be noop
    assert!(ed.value.is_empty());
}

#[test]
fn test_history_stashes_current_input() {
    let mut ed = InputEditor::new();
    ed.push_history("old");
    ed.insert_str("new typing");
    ed.history_up(); // stashes "new typing", shows "old"
    assert_eq!(ed.value, "old");
    ed.history_down(); // restores stash
    assert_eq!(ed.value, "new typing");
}

#[test]
fn test_history_empty_up_is_noop() {
    let mut ed = InputEditor::new();
    ed.history_up();
    assert!(ed.is_empty());
}

#[test]
fn test_history_position() {
    let mut ed = InputEditor::new();
    assert!(ed.history_position().is_none());
    ed.push_history("a");
    ed.push_history("b");
    ed.history_up();
    assert_eq!(ed.history_position(), Some(1));
    ed.history_up();
    assert_eq!(ed.history_position(), Some(0));
}

#[test]
fn test_clear_history() {
    let mut ed = InputEditor::new();
    ed.push_history("a");
    ed.push_history("b");
    ed.clear_history();
    assert!(ed.history().is_empty());
}

// ============================================================================
// Submit Tests
// ============================================================================

#[test]
fn test_submit_returns_and_clears() {
    let mut ed = InputEditor::new();
    ed.insert_str("cargo build");
    let submitted = ed.submit();
    assert_eq!(submitted, "cargo build");
    assert!(ed.is_empty());
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_submit_pushes_to_history() {
    let mut ed = InputEditor::new();
    ed.insert_str("cargo test");
    ed.submit();
    assert_eq!(ed.history().len(), 1);
    assert_eq!(ed.history()[0], "cargo test");
}

#[test]
fn test_submit_resets_history_cursor() {
    let mut ed = InputEditor::new();
    ed.push_history("old");
    ed.history_up();
    ed.insert_str("new");
    ed.submit();
    assert!(ed.history_position().is_none());
}

#[test]
fn test_submit_clears_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.select_all();
    ed.submit();
    assert!(ed.selection().is_none());
}

// ============================================================================
// Edit Mode Tests
// ============================================================================

#[test]
fn test_edit_mode_default_insert() {
    let ed = InputEditor::new();
    assert_eq!(ed.edit_mode(), EditMode::Insert);
}

#[test]
fn test_toggle_edit_mode() {
    let mut ed = InputEditor::new();
    ed.toggle_edit_mode();
    assert_eq!(ed.edit_mode(), EditMode::Overwrite);
    ed.toggle_edit_mode();
    assert_eq!(ed.edit_mode(), EditMode::Insert);
}

#[test]
fn test_set_edit_mode() {
    let mut ed = InputEditor::new();
    ed.set_edit_mode(EditMode::Overwrite);
    assert_eq!(ed.edit_mode(), EditMode::Overwrite);
}

#[test]
fn test_overwrite_mode_replaces_char() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.set_edit_mode(EditMode::Overwrite);
    ed.insert_char('X');
    assert_eq!(ed.value, "Xbc");
    assert_eq!(ed.cursor(), 1);
}

#[test]
fn test_overwrite_mode_at_end_appends() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.set_edit_mode(EditMode::Overwrite);
    ed.insert_char('d');
    assert_eq!(ed.value, "abcd");
}

#[test]
fn test_edit_mode_display() {
    assert_eq!(format!("{}", EditMode::Insert), "INS");
    assert_eq!(format!("{}", EditMode::Overwrite), "OVR");
}

// ============================================================================
// Vim Mode Tests
// ============================================================================

#[test]
fn test_vim_disabled_by_default() {
    let ed = InputEditor::new();
    assert_eq!(ed.vim_mode(), VimMode::Disabled);
}

#[test]
fn test_enable_vim() {
    let mut ed = InputEditor::new();
    ed.enable_vim();
    assert_eq!(ed.vim_mode(), VimMode::Normal);
}

#[test]
fn test_disable_vim() {
    let mut ed = InputEditor::new();
    ed.enable_vim();
    ed.disable_vim();
    assert_eq!(ed.vim_mode(), VimMode::Disabled);
}

#[test]
fn test_vim_mode_display() {
    assert_eq!(format!("{}", VimMode::Disabled), "—");
    assert_eq!(format!("{}", VimMode::Normal), "NORMAL");
    assert_eq!(format!("{}", VimMode::Insert), "INSERT");
    assert_eq!(format!("{}", VimMode::Command), "COMMAND");
}

#[test]
fn test_vim_h_moves_left() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.enable_vim();
    assert!(ed.vim_motion('h'));
    assert_eq!(ed.cursor(), 2);
}

#[test]
fn test_vim_l_moves_right() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.enable_vim();
    assert!(ed.vim_motion('l'));
    assert_eq!(ed.cursor(), 1);
}

#[test]
fn test_vim_0_goes_home() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.enable_vim();
    assert!(ed.vim_motion('0'));
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_vim_dollar_goes_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.enable_vim();
    assert!(ed.vim_motion('$'));
    assert_eq!(ed.cursor(), 3);
}

#[test]
fn test_vim_w_word_forward() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.move_home();
    ed.enable_vim();
    assert!(ed.vim_motion('w'));
    assert_eq!(ed.cursor(), 6);
}

#[test]
fn test_vim_b_word_backward() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.enable_vim();
    assert!(ed.vim_motion('b'));
    assert_eq!(ed.cursor(), 6);
}

#[test]
fn test_vim_i_enters_insert() {
    let mut ed = InputEditor::new();
    ed.enable_vim();
    assert!(ed.vim_motion('i'));
    assert_eq!(ed.vim_mode(), VimMode::Insert);
}

#[test]
fn test_vim_a_enters_insert_after() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.enable_vim();
    assert!(ed.vim_motion('a'));
    assert_eq!(ed.vim_mode(), VimMode::Insert);
    assert_eq!(ed.cursor(), 1); // moved right then insert
}

#[test]
fn test_vim_big_a_appends_at_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.enable_vim();
    assert!(ed.vim_motion('A'));
    assert_eq!(ed.vim_mode(), VimMode::Insert);
    assert_eq!(ed.cursor(), 3);
}

#[test]
fn test_vim_big_i_inserts_at_start() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.enable_vim();
    assert!(ed.vim_motion('I'));
    assert_eq!(ed.vim_mode(), VimMode::Insert);
    assert_eq!(ed.cursor(), 0);
}

#[test]
fn test_vim_x_deletes_char() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.enable_vim();
    assert!(ed.vim_motion('x'));
    assert_eq!(ed.value, "bc");
}

#[test]
fn test_vim_big_x_backspaces() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.enable_vim();
    assert!(ed.vim_motion('X'));
    assert_eq!(ed.value, "ab");
}

#[test]
fn test_vim_big_d_kills_to_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.set_cursor(5);
    ed.enable_vim();
    assert!(ed.vim_motion('D'));
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_vim_big_c_kills_and_inserts() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.set_cursor(5);
    ed.enable_vim();
    assert!(ed.vim_motion('C'));
    assert_eq!(ed.value, "hello");
    assert_eq!(ed.vim_mode(), VimMode::Insert);
}

#[test]
fn test_vim_u_undoes() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.enable_vim();
    assert!(ed.vim_motion('x')); // delete 'o' (wait, cursor is at end... let me fix)
    // Actually x at end: cursor is at 5 which is past the last char, so delete is noop
    // Let's move left first
    ed.vim_motion('h'); // cursor at 4
    ed.vim_motion('x'); // deletes 'o'
    assert_eq!(ed.value, "hell");
    ed.vim_motion('u'); // undo
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_vim_colon_enters_command_mode() {
    let mut ed = InputEditor::new();
    ed.enable_vim();
    assert!(ed.vim_motion(':'));
    assert_eq!(ed.vim_mode(), VimMode::Command);
}

#[test]
fn test_vim_escape_from_insert() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.enable_vim();
    ed.vim_motion('i'); // enter insert mode
    assert_eq!(ed.vim_mode(), VimMode::Insert);
    ed.vim_escape();
    assert_eq!(ed.vim_mode(), VimMode::Normal);
    // Cursor should have moved left by convention
    assert_eq!(ed.cursor(), 2);
}

#[test]
fn test_vim_escape_from_command() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.enable_vim();
    ed.vim_motion(':');
    ed.vim_escape();
    assert_eq!(ed.vim_mode(), VimMode::Normal);
}

#[test]
fn test_vim_motion_returns_false_for_unknown() {
    let mut ed = InputEditor::new();
    ed.enable_vim();
    assert!(!ed.vim_motion('z'));
}

#[test]
fn test_vim_motion_returns_false_when_not_normal() {
    let mut ed = InputEditor::new();
    ed.set_vim_mode(VimMode::Insert);
    assert!(!ed.vim_motion('h')); // motions only work in Normal
}

// ============================================================================
// Transpose Tests
// ============================================================================

#[test]
fn test_transpose_chars() {
    let mut ed = InputEditor::new();
    ed.insert_str("ab");
    ed.set_cursor(1);
    ed.transpose_chars();
    assert_eq!(ed.value, "ba");
}

#[test]
fn test_transpose_at_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    // cursor at end (3) — should transpose last two chars
    ed.transpose_chars();
    assert_eq!(ed.value, "acb");
}

#[test]
fn test_transpose_at_start_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.transpose_chars();
    assert_eq!(ed.value, "abc"); // can't transpose with nothing before cursor
}

#[test]
fn test_transpose_single_char_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_char('a');
    ed.move_home();
    ed.transpose_chars();
    assert_eq!(ed.value, "a");
}

// ============================================================================
// set_value Tests
// ============================================================================

#[test]
fn test_set_value() {
    let mut ed = InputEditor::new();
    ed.set_value("hello world");
    assert_eq!(ed.value, "hello world");
    assert_eq!(ed.cursor(), 11); // at end
    assert!(ed.selection().is_none());
}

#[test]
fn test_set_value_clears_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("old");
    ed.select_all();
    ed.set_value("new");
    assert!(ed.selection().is_none());
}

// ============================================================================
// Selection Struct Tests
// ============================================================================

#[test]
fn test_selection_range_forward() {
    let sel = Selection { anchor: 2, cursor: 5 };
    assert_eq!(sel.range(), (2, 5));
    assert_eq!(sel.len(), 3);
    assert!(!sel.is_empty());
}

#[test]
fn test_selection_range_backward() {
    let sel = Selection { anchor: 5, cursor: 2 };
    assert_eq!(sel.range(), (2, 5)); // normalized
    assert_eq!(sel.len(), 3);
}

#[test]
fn test_selection_empty() {
    let sel = Selection { anchor: 3, cursor: 3 };
    assert!(sel.is_empty());
    assert_eq!(sel.len(), 0);
}

// ============================================================================
// Unicode Edge Cases
// ============================================================================

#[test]
fn test_unicode_insert() {
    let mut ed = InputEditor::new();
    ed.insert_str("héllo wörld");
    assert_eq!(ed.value, "héllo wörld");
    assert_eq!(ed.char_count(), 11);
}

#[test]
fn test_unicode_backspace() {
    let mut ed = InputEditor::new();
    ed.insert_str("café");
    ed.backspace();
    assert_eq!(ed.value, "caf");
}

#[test]
fn test_unicode_move_left_right() {
    let mut ed = InputEditor::new();
    ed.insert_str("日本語");
    ed.move_left(); // before 語
    ed.move_left(); // before 本
    assert_eq!(&ed.value[ed.cursor()..], "本語");
    ed.move_right(); // after 本
    assert_eq!(&ed.value[ed.cursor()..], "語");
}

#[test]
fn test_unicode_delete() {
    let mut ed = InputEditor::new();
    ed.insert_str("αβγ");
    ed.move_home();
    ed.delete();
    assert_eq!(ed.value, "βγ");
}

#[test]
fn test_emoji_handling() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello 🌍!");
    ed.backspace(); // remove '!'
    assert_eq!(ed.value, "hello 🌍");
    ed.backspace(); // remove 🌍
    assert_eq!(ed.value, "hello ");
}

#[test]
fn test_unicode_overwrite_mode() {
    let mut ed = InputEditor::new();
    ed.insert_str("αβγ");
    ed.move_home();
    ed.set_edit_mode(EditMode::Overwrite);
    ed.insert_char('X');
    assert_eq!(ed.value, "Xβγ");
}

// ============================================================================
// Stress / Edge Cases
// ============================================================================

#[test]
fn test_rapid_insert_delete_cycle() {
    let mut ed = InputEditor::new();
    for i in 0..100 {
        ed.insert_char(char::from(b'a' + (i % 26)));
    }
    assert_eq!(ed.char_count(), 100);
    for _ in 0..100 {
        ed.backspace();
    }
    assert!(ed.is_empty());
}

#[test]
fn test_undo_redo_stress() {
    let mut ed = InputEditor::new();
    for i in 0..50 {
        ed.insert_char(char::from(b'a' + (i % 26)));
    }
    for _ in 0..50 {
        ed.undo();
    }
    assert!(ed.is_empty());
    for _ in 0..50 {
        ed.redo();
    }
    assert_eq!(ed.char_count(), 50);
}

#[test]
fn test_history_navigation_preserves_editor_state() {
    let mut ed = InputEditor::new();
    ed.push_history("cmd1");
    ed.push_history("cmd2");
    ed.push_history("cmd3");

    // Navigate up to oldest, then back down
    ed.insert_str("current");
    ed.history_up();
    ed.history_up();
    ed.history_up();
    assert_eq!(ed.value, "cmd1");

    ed.history_down();
    ed.history_down();
    ed.history_down();
    assert_eq!(ed.value, "current");
}

#[test]
fn test_word_boundaries_with_special_chars() {
    let mut ed = InputEditor::new();
    ed.insert_str("git commit -m \"hello\"");
    ed.move_word_left(); // before the last word token
    // The exact position depends on ASCII whitespace boundaries
    // Just verify it moved and didn't panic
    assert!(ed.cursor() < ed.value.len());
}