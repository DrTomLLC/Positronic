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
fn test_set_cursor_clamped() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.set_cursor(999);
    assert_eq!(ed.cursor(), 3);
    ed.set_cursor(0);
    assert_eq!(ed.cursor(), 0);
}

// ============================================================================
// Word-Level Cursor Movement
// ============================================================================

#[test]
fn test_move_word_right() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.move_home();
    ed.move_word_right();
    // Should be at or past the space after "hello"
    assert!(ed.cursor() >= 5);
}

#[test]
fn test_move_word_left() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.move_word_left();
    // Should jump back to start of "world"
    assert_eq!(ed.cursor(), 6);
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
fn test_move_word_right_at_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello");
    ed.move_word_right();
    assert_eq!(ed.cursor(), 5);
}

// ============================================================================
// Text Insertion & Deletion
// ============================================================================

#[test]
fn test_insert_char() {
    let mut ed = InputEditor::new();
    ed.insert_char('h');
    ed.insert_char('i');
    assert_eq!(ed.value, "hi");
    assert_eq!(ed.cursor(), 2);
}

#[test]
fn test_insert_str() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    assert_eq!(ed.value, "hello world");
    assert_eq!(ed.cursor(), 11);
}

#[test]
fn test_insert_str_empty_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("");
    assert!(ed.is_empty());
}

#[test]
fn test_insert_at_middle() {
    let mut ed = InputEditor::new();
    ed.insert_str("helo");
    ed.set_cursor(2);
    ed.insert_char('l');
    assert_eq!(ed.value, "hello");
}

#[test]
fn test_backspace() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.backspace();
    assert_eq!(ed.value, "ab");
    assert_eq!(ed.cursor(), 2);
}

#[test]
fn test_backspace_at_start_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.backspace();
    assert_eq!(ed.value, "abc");
}

#[test]
fn test_delete() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.delete();
    assert_eq!(ed.value, "bc");
}

#[test]
fn test_delete_at_end_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.delete();
    assert_eq!(ed.value, "abc");
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
fn test_set_value() {
    let mut ed = InputEditor::new();
    ed.set_value("replaced");
    assert_eq!(ed.value, "replaced");
    assert_eq!(ed.cursor(), 8); // cursor at end
}

#[test]
fn test_submit() {
    let mut ed = InputEditor::new();
    ed.insert_str("cargo build");
    let submitted = ed.submit();
    assert_eq!(submitted, "cargo build");
    assert!(ed.is_empty());
    assert_eq!(ed.cursor(), 0);
    assert_eq!(ed.history().len(), 1);
}

// ============================================================================
// Selection
// ============================================================================

#[test]
fn test_selection_initially_none() {
    let ed = InputEditor::new();
    assert!(ed.selection().is_none());
}

#[test]
fn test_selection_range() {
    let sel = Selection { anchor: 5, cursor: 2 };
    let (start, end) = sel.range();
    assert_eq!(start, 2);
    assert_eq!(end, 5);
}

#[test]
fn test_selection_len() {
    let sel = Selection { anchor: 1, cursor: 4 };
    assert_eq!(sel.len(), 3);
}

#[test]
fn test_selection_is_empty() {
    let sel = Selection { anchor: 3, cursor: 3 };
    assert!(sel.is_empty());
}

#[test]
fn test_cut_selection() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    // Manually create a selection (select "world")
    // This depends on the selection API; if there's select_word etc.
    // For now test that cut_selection returns None when nothing selected
    assert!(ed.cut_selection().is_none());
}

// ============================================================================
// Kill/Yank (Emacs-style)
// ============================================================================

#[test]
fn test_kill_to_end() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.set_cursor(5);
    ed.kill_to_end();
    assert_eq!(ed.value, "hello");
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
    ed.set_cursor(6);
    ed.kill_to_start();
    assert_eq!(ed.value, "world");
    assert_eq!(ed.cursor(), 0);
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
}

#[test]
fn test_kill_word_forward() {
    let mut ed = InputEditor::new();
    ed.insert_str("hello world");
    ed.set_cursor(6);
    ed.kill_word_forward();
    assert_eq!(ed.value, "hello ");
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
    assert_eq!(ed.kill_ring().len(), 2);
}

#[test]
fn test_kill_ring_max_size() {
    let mut ed = InputEditor::new();
    for i in 0..15 {
        ed.insert_str(&format!("word{} ", i));
        ed.kill_word_back();
    }
    assert!(ed.kill_ring().len() <= 10);
}

// ============================================================================
// Undo/Redo
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
    ed.insert_char('x');
    assert!(!ed.can_redo());
}

#[test]
fn test_undo_empty_is_noop() {
    let mut ed = InputEditor::new();
    ed.undo();
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
// Command History
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
    ed.history_up();
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
    ed.history_down();
    assert!(ed.value.is_empty());
}

#[test]
fn test_history_stashes_current_input() {
    let mut ed = InputEditor::new();
    ed.push_history("old");
    ed.insert_str("new typing");
    ed.history_up(); // stashes "new typing", shows "old"
    assert_eq!(ed.value, "old");
    ed.history_down(); // restores stashed input
    assert_eq!(ed.value, "new typing");
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
// Edit Mode
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
// Vim Mode
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
    assert_eq!(ed.cursor(), 1);
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
    assert!(!ed.vim_motion('h'));
}

// ============================================================================
// Transpose
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
    ed.transpose_chars();
    assert_eq!(ed.value, "acb");
}

#[test]
fn test_transpose_at_start_is_noop() {
    let mut ed = InputEditor::new();
    ed.insert_str("abc");
    ed.move_home();
    ed.transpose_chars();
    assert_eq!(ed.value, "abc");
}

// ============================================================================
// Unicode Support
// ============================================================================

#[test]
fn test_unicode_insert() {
    let mut ed = InputEditor::new();
    ed.insert_str("日本語");
    assert_eq!(ed.char_count(), 3);
    assert!(ed.len() > 3); // byte length > char count for CJK
}

#[test]
fn test_unicode_cursor_movement() {
    let mut ed = InputEditor::new();
    ed.insert_str("αβγ");
    ed.move_left();
    ed.move_left();
    assert_eq!(ed.cursor(), 2); // alpha is 2 bytes in UTF-8
}

#[test]
fn test_unicode_backspace() {
    let mut ed = InputEditor::new();
    ed.insert_str("café");
    ed.backspace();
    assert_eq!(ed.value, "caf");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_rapid_undo_redo_cycle() {
    let mut ed = InputEditor::new();
    for i in 0..20 {
        ed.insert_char(char::from(b'a' + (i % 26)));
    }
    for _ in 0..20 {
        ed.undo();
    }
    assert!(ed.is_empty());
    for _ in 0..20 {
        ed.redo();
    }
    assert_eq!(ed.char_count(), 20);
}

#[test]
fn test_empty_editor_operations() {
    let mut ed = InputEditor::new();
    ed.move_left();
    ed.move_right();
    ed.move_home();
    ed.move_end();
    ed.move_word_left();
    ed.move_word_right();
    ed.backspace();
    ed.delete();
    ed.kill_to_end();
    ed.kill_to_start();
    ed.kill_word_back();
    ed.kill_word_forward();
    ed.yank();
    ed.undo();
    ed.redo();
    ed.transpose_chars();
    // None of these should panic on empty editor
    assert!(ed.is_empty());
}

#[test]
fn test_very_long_input() {
    let mut ed = InputEditor::new();
    let long = "x".repeat(10_000);
    ed.insert_str(&long);
    assert_eq!(ed.len(), 10_000);
    assert_eq!(ed.cursor(), 10_000);
    ed.move_home();
    assert_eq!(ed.cursor(), 0);
}