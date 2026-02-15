// positronic-bridge/src/input/mod.rs
//
// The "Intelli-Input" editor (Roadmap Pillar II).
//
// A full terminal input editor with cursor management, selection,
// word-level navigation, kill/yank ring, undo/redo, command history,
// insert/overwrite modes, and a vim-mode state machine.
//
// All editing logic is pure Rust with zero UI dependencies.
// The iced `view()` method at the bottom is a thin rendering wrapper.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════
// Edit Mode
// ═══════════════════════════════════════════════════════════════════

/// Whether keystrokes insert or overwrite characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Insert,
    Overwrite,
}

impl EditMode {
    /// Toggle between Insert and Overwrite.
    pub fn toggle(&self) -> Self {
        match self {
            EditMode::Insert => EditMode::Overwrite,
            EditMode::Overwrite => EditMode::Insert,
        }
    }
}

impl fmt::Display for EditMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditMode::Insert => write!(f, "INS"),
            EditMode::Overwrite => write!(f, "OVR"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Vim Mode
// ═══════════════════════════════════════════════════════════════════

/// Vim-mode state machine modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    /// Disabled — normal editor behavior.
    Disabled,
    /// Normal mode: motions, operators.
    Normal,
    /// Insert mode: typing inserts text.
    Insert,
    /// Command-line mode (after pressing `:` in Normal).
    Command,
}

impl fmt::Display for VimMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VimMode::Disabled => write!(f, "—"),
            VimMode::Normal => write!(f, "NORMAL"),
            VimMode::Insert => write!(f, "INSERT"),
            VimMode::Command => write!(f, "COMMAND"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Selection
// ═══════════════════════════════════════════════════════════════════

/// A text selection range (anchor + cursor form the extent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Where the selection started (fixed end).
    pub anchor: usize,
    /// Where the cursor currently is (moving end).
    pub cursor: usize,
}

impl Selection {
    /// Normalized start..end range (start <= end).
    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    /// Number of selected characters.
    pub fn len(&self) -> usize {
        let (start, end) = self.range();
        end - start
    }

    /// Whether the selection is empty.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }
}

// ═══════════════════════════════════════════════════════════════════
// Undo Entry
// ═══════════════════════════════════════════════════════════════════

/// A snapshot of editor state for undo/redo.
#[derive(Debug, Clone)]
struct UndoEntry {
    value: String,
    cursor: usize,
}

// ═══════════════════════════════════════════════════════════════════
// InputEditor
// ═══════════════════════════════════════════════════════════════════

/// The Intelli-Input editor: a full terminal command-line editor.
///
/// Manages all editing state independent of any UI framework.
/// The iced `view()` method is a thin rendering wrapper.
#[derive(Debug, Clone)]
pub struct InputEditor {
    /// The current text content.
    pub value: String,
    /// Byte-offset cursor position within `value`.
    cursor: usize,
    /// Current selection (None if no selection active).
    selection: Option<Selection>,
    /// Insert vs Overwrite mode.
    edit_mode: EditMode,
    /// Vim mode state.
    vim_mode: VimMode,
    /// Kill ring (clipboard for Ctrl+K/U/W/Y).
    kill_ring: Vec<String>,
    /// Undo stack.
    undo_stack: Vec<UndoEntry>,
    /// Redo stack.
    redo_stack: Vec<UndoEntry>,
    /// Maximum undo depth.
    max_undo: usize,
    /// Command history buffer.
    history: Vec<String>,
    /// Current position in history (None = editing new input).
    history_cursor: Option<usize>,
    /// Saved input when navigating history.
    history_stash: String,
}

impl InputEditor {
    /// Create a new empty editor.
    pub fn new() -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            selection: None,
            edit_mode: EditMode::Insert,
            vim_mode: VimMode::Disabled,
            kill_ring: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo: 100,
            history: Vec::new(),
            history_cursor: None,
            history_stash: String::new(),
        }
    }

    // ───────────────────────────────────────────────────────────────
    // Cursor queries
    // ───────────────────────────────────────────────────────────────

    /// Current cursor byte offset.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Set cursor to an absolute position, clamped to valid range.
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.value.len());
        self.selection = None;
    }

    /// Whether the cursor is at the start of the buffer.
    pub fn at_start(&self) -> bool {
        self.cursor == 0
    }

    /// Whether the cursor is at the end of the buffer.
    pub fn at_end(&self) -> bool {
        self.cursor == self.value.len()
    }

    /// Number of characters (not bytes) in the buffer.
    pub fn char_count(&self) -> usize {
        self.value.chars().count()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Current line length in bytes.
    pub fn len(&self) -> usize {
        self.value.len()
    }

    // ───────────────────────────────────────────────────────────────
    // Character-level cursor movement
    // ───────────────────────────────────────────────────────────────

    /// Move cursor one character to the left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            // Walk back to previous char boundary
            let mut pos = self.cursor - 1;
            while pos > 0 && !self.value.is_char_boundary(pos) {
                pos -= 1;
            }
            self.cursor = pos;
        }
        self.selection = None;
    }

    /// Move cursor one character to the right.
    pub fn move_right(&mut self) {
        if self.cursor < self.value.len() {
            // Walk forward to next char boundary
            let mut pos = self.cursor + 1;
            while pos < self.value.len() && !self.value.is_char_boundary(pos) {
                pos += 1;
            }
            self.cursor = pos;
        }
        self.selection = None;
    }

    /// Move cursor to the start (Home).
    pub fn move_home(&mut self) {
        self.cursor = 0;
        self.selection = None;
    }

    /// Move cursor to the end (End).
    pub fn move_end(&mut self) {
        self.cursor = self.value.len();
        self.selection = None;
    }

    // ───────────────────────────────────────────────────────────────
    // Word-level cursor movement
    // ───────────────────────────────────────────────────────────────

    /// Move cursor one word to the left (Ctrl+Left).
    pub fn move_word_left(&mut self) {
        self.cursor = self.find_word_boundary_left();
        self.selection = None;
    }

    /// Move cursor one word to the right (Ctrl+Right).
    pub fn move_word_right(&mut self) {
        self.cursor = self.find_word_boundary_right();
        self.selection = None;
    }

    /// Find the byte offset of the previous word boundary.
    fn find_word_boundary_left(&self) -> usize {
        if self.cursor == 0 {
            return 0;
        }
        let bytes = self.value.as_bytes();
        let mut pos = self.cursor;

        // Skip whitespace going left
        while pos > 0 && bytes[pos - 1].is_ascii_whitespace() {
            pos -= 1;
        }
        // Skip word chars going left
        while pos > 0 && !bytes[pos - 1].is_ascii_whitespace() {
            pos -= 1;
        }
        pos
    }

    /// Find the byte offset of the next word boundary.
    fn find_word_boundary_right(&self) -> usize {
        let len = self.value.len();
        if self.cursor >= len {
            return len;
        }
        let bytes = self.value.as_bytes();
        let mut pos = self.cursor;

        // Skip current word chars
        while pos < len && !bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        // Skip whitespace
        while pos < len && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        pos
    }

    // ───────────────────────────────────────────────────────────────
    // Selection
    // ───────────────────────────────────────────────────────────────

    /// Get the current selection, if any.
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// Start or extend selection one character left (Shift+Left).
    pub fn select_left(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        if self.cursor > 0 {
            let mut pos = self.cursor - 1;
            while pos > 0 && !self.value.is_char_boundary(pos) {
                pos -= 1;
            }
            self.cursor = pos;
        }
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    /// Start or extend selection one character right (Shift+Right).
    pub fn select_right(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        if self.cursor < self.value.len() {
            let mut pos = self.cursor + 1;
            while pos < self.value.len() && !self.value.is_char_boundary(pos) {
                pos += 1;
            }
            self.cursor = pos;
        }
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    /// Select to start of line (Shift+Home).
    pub fn select_home(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        self.cursor = 0;
        self.selection = Some(Selection { anchor, cursor: 0 });
        self.collapse_empty_selection();
    }

    /// Select to end of line (Shift+End).
    pub fn select_end(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        self.cursor = self.value.len();
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    /// Select one word left (Ctrl+Shift+Left).
    pub fn select_word_left(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        self.cursor = self.find_word_boundary_left();
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    /// Select one word right (Ctrl+Shift+Right).
    pub fn select_word_right(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        self.cursor = self.find_word_boundary_right();
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    /// Select all text (Ctrl+A).
    pub fn select_all(&mut self) {
        if self.value.is_empty() {
            self.selection = None;
        } else {
            self.selection = Some(Selection { anchor: 0, cursor: self.value.len() });
            self.cursor = self.value.len();
        }
    }

    /// Clear the selection without deleting text.
    pub fn deselect(&mut self) {
        self.selection = None;
    }

    /// Get the selected text, if any.
    pub fn selected_text(&self) -> Option<&str> {
        self.selection.map(|sel| {
            let (start, end) = sel.range();
            &self.value[start..end]
        })
    }

    /// If selection collapsed to zero width, remove it.
    fn collapse_empty_selection(&mut self) {
        if let Some(sel) = &self.selection {
            if sel.is_empty() {
                self.selection = None;
            }
        }
    }

    // ───────────────────────────────────────────────────────────────
    // Text insertion and deletion
    // ───────────────────────────────────────────────────────────────

    /// Insert a character at the cursor (handles selection replacement).
    pub fn insert_char(&mut self, ch: char) {
        self.save_undo();
        self.delete_selection_internal();

        if self.edit_mode == EditMode::Overwrite && self.cursor < self.value.len() {
            // Remove the character under the cursor first
            let end = next_char_boundary(&self.value, self.cursor);
            self.value.replace_range(self.cursor..end, "");
        }

        self.value.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.redo_stack.clear();
    }

    /// Insert a string at the cursor (handles selection replacement).
    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.save_undo();
        self.delete_selection_internal();
        self.value.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.redo_stack.clear();
    }

    /// Delete the character before the cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.selection.is_some() {
            self.save_undo();
            self.delete_selection_internal();
            self.redo_stack.clear();
            return;
        }
        if self.cursor == 0 {
            return;
        }
        self.save_undo();
        let prev = prev_char_boundary(&self.value, self.cursor);
        self.value.replace_range(prev..self.cursor, "");
        self.cursor = prev;
        self.redo_stack.clear();
    }

    /// Delete the character after the cursor (Delete key).
    pub fn delete(&mut self) {
        if self.selection.is_some() {
            self.save_undo();
            self.delete_selection_internal();
            self.redo_stack.clear();
            return;
        }
        if self.cursor >= self.value.len() {
            return;
        }
        self.save_undo();
        let next = next_char_boundary(&self.value, self.cursor);
        self.value.replace_range(self.cursor..next, "");
        self.redo_stack.clear();
    }

    /// Delete the selected text (internal, does not save undo).
    fn delete_selection_internal(&mut self) {
        if let Some(sel) = self.selection.take() {
            let (start, end) = sel.range();
            self.value.replace_range(start..end, "");
            self.cursor = start;
        }
    }

    /// Delete the selected text and return it, or None if nothing selected.
    pub fn cut_selection(&mut self) -> Option<String> {
        if let Some(sel) = self.selection {
            let (start, end) = sel.range();
            let cut = self.value[start..end].to_string();
            self.save_undo();
            self.value.replace_range(start..end, "");
            self.cursor = start;
            self.selection = None;
            self.redo_stack.clear();
            Some(cut)
        } else {
            None
        }
    }

    /// Set the value and place cursor at the end (e.g. history recall).
    pub fn set_value(&mut self, s: &str) {
        self.value = s.to_string();
        self.cursor = self.value.len();
        self.selection = None;
    }

    /// Clear all text.
    pub fn clear(&mut self) {
        if !self.value.is_empty() {
            self.save_undo();
            self.value.clear();
            self.cursor = 0;
            self.selection = None;
            self.redo_stack.clear();
        }
    }

    // ───────────────────────────────────────────────────────────────
    // Kill/Yank (Emacs-style line editing)
    // ───────────────────────────────────────────────────────────────

    /// Kill from cursor to end of line (Ctrl+K).
    pub fn kill_to_end(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        self.save_undo();
        let killed = self.value[self.cursor..].to_string();
        self.value.truncate(self.cursor);
        self.push_kill(killed);
        self.redo_stack.clear();
    }

    /// Kill from start of line to cursor (Ctrl+U).
    pub fn kill_to_start(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.save_undo();
        let killed = self.value[..self.cursor].to_string();
        self.value = self.value[self.cursor..].to_string();
        self.cursor = 0;
        self.push_kill(killed);
        self.redo_stack.clear();
    }

    /// Kill the word before the cursor (Ctrl+W / Alt+Backspace).
    pub fn kill_word_back(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.save_undo();
        let boundary = self.find_word_boundary_left();
        let killed = self.value[boundary..self.cursor].to_string();
        self.value.replace_range(boundary..self.cursor, "");
        self.cursor = boundary;
        self.push_kill(killed);
        self.redo_stack.clear();
    }

    /// Kill the word after the cursor (Alt+D).
    pub fn kill_word_forward(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        self.save_undo();
        let boundary = self.find_word_boundary_right();
        let killed = self.value[self.cursor..boundary].to_string();
        self.value.replace_range(self.cursor..boundary, "");
        self.push_kill(killed);
        self.redo_stack.clear();
    }

    /// Yank (paste) the most recent kill (Ctrl+Y).
    pub fn yank(&mut self) {
        if let Some(text) = self.kill_ring.last().cloned() {
            self.insert_str(&text);
        }
    }

    /// Get the kill ring contents.
    pub fn kill_ring(&self) -> &[String] {
        &self.kill_ring
    }

    /// Push to the kill ring (keeps last 10 entries).
    fn push_kill(&mut self, text: String) {
        self.kill_ring.push(text);
        if self.kill_ring.len() > 10 {
            self.kill_ring.remove(0);
        }
    }

    // ───────────────────────────────────────────────────────────────
    // Undo / Redo
    // ───────────────────────────────────────────────────────────────

    /// Undo the last edit.
    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            self.redo_stack.push(UndoEntry {
                value: self.value.clone(),
                cursor: self.cursor,
            });
            self.value = entry.value;
            self.cursor = entry.cursor;
            self.selection = None;
        }
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            self.undo_stack.push(UndoEntry {
                value: self.value.clone(),
                cursor: self.cursor,
            });
            self.value = entry.value;
            self.cursor = entry.cursor;
            self.selection = None;
        }
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Save current state to undo stack.
    fn save_undo(&mut self) {
        self.undo_stack.push(UndoEntry {
            value: self.value.clone(),
            cursor: self.cursor,
        });
        if self.undo_stack.len() > self.max_undo {
            self.undo_stack.remove(0);
        }
    }

    // ───────────────────────────────────────────────────────────────
    // Command History
    // ───────────────────────────────────────────────────────────────

    /// Push a command to history.
    pub fn push_history(&mut self, command: &str) {
        if command.trim().is_empty() {
            return;
        }
        // Avoid consecutive duplicates
        if self.history.last().map(|s| s.as_str()) == Some(command) {
            return;
        }
        self.history.push(command.to_string());
        self.history_cursor = None;
        self.history_stash.clear();
    }

    /// Navigate up in history (older commands).
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_cursor {
            None => {
                // First press: stash current input, jump to most recent
                self.history_stash = self.value.clone();
                let idx = self.history.len() - 1;
                self.history_cursor = Some(idx);
                self.set_value(&self.history[idx].clone());
            }
            Some(idx) if idx > 0 => {
                let new_idx = idx - 1;
                self.history_cursor = Some(new_idx);
                self.set_value(&self.history[new_idx].clone());
            }
            _ => {} // Already at oldest
        }
    }

    /// Navigate down in history (newer commands).
    pub fn history_down(&mut self) {
        match self.history_cursor {
            Some(idx) => {
                if idx + 1 < self.history.len() {
                    let new_idx = idx + 1;
                    self.history_cursor = Some(new_idx);
                    self.set_value(&self.history[new_idx].clone());
                } else {
                    // Back to the stashed input
                    self.history_cursor = None;
                    let stash = self.history_stash.clone();
                    self.set_value(&stash);
                    self.history_stash.clear();
                }
            }
            None => {} // Already at bottom
        }
    }

    /// Get the history buffer.
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Get the current history cursor position.
    pub fn history_position(&self) -> Option<usize> {
        self.history_cursor
    }

    /// Clear command history.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_cursor = None;
        self.history_stash.clear();
    }

    // ───────────────────────────────────────────────────────────────
    // Edit Mode
    // ───────────────────────────────────────────────────────────────

    /// Get the current edit mode.
    pub fn edit_mode(&self) -> EditMode {
        self.edit_mode
    }

    /// Toggle between Insert and Overwrite mode.
    pub fn toggle_edit_mode(&mut self) {
        self.edit_mode = self.edit_mode.toggle();
    }

    /// Set edit mode directly.
    pub fn set_edit_mode(&mut self, mode: EditMode) {
        self.edit_mode = mode;
    }

    // ───────────────────────────────────────────────────────────────
    // Vim Mode
    // ───────────────────────────────────────────────────────────────

    /// Get the current vim mode.
    pub fn vim_mode(&self) -> VimMode {
        self.vim_mode
    }

    /// Enable vim mode (enters Normal mode).
    pub fn enable_vim(&mut self) {
        self.vim_mode = VimMode::Normal;
    }

    /// Disable vim mode (back to standard editing).
    pub fn disable_vim(&mut self) {
        self.vim_mode = VimMode::Disabled;
    }

    /// Set vim mode directly.
    pub fn set_vim_mode(&mut self, mode: VimMode) {
        self.vim_mode = mode;
    }

    /// Process a vim normal-mode motion command.
    /// Returns true if the command was handled.
    pub fn vim_motion(&mut self, ch: char) -> bool {
        if self.vim_mode != VimMode::Normal {
            return false;
        }
        match ch {
            'h' => { self.move_left(); true }
            'l' => { self.move_right(); true }
            '0' => { self.move_home(); true }
            '$' => { self.move_end(); true }
            'w' => { self.move_word_right(); true }
            'b' => { self.move_word_left(); true }
            'i' => { self.vim_mode = VimMode::Insert; true }
            'a' => {
                self.move_right();
                self.vim_mode = VimMode::Insert;
                true
            }
            'A' => {
                self.move_end();
                self.vim_mode = VimMode::Insert;
                true
            }
            'I' => {
                self.move_home();
                self.vim_mode = VimMode::Insert;
                true
            }
            'x' => { self.delete(); true }
            'X' => { self.backspace(); true }
            'D' => { self.kill_to_end(); true }
            'C' => {
                self.kill_to_end();
                self.vim_mode = VimMode::Insert;
                true
            }
            'u' => { self.undo(); true }
            ':' => { self.vim_mode = VimMode::Command; true }
            _ => false,
        }
    }

    /// Exit vim insert mode back to normal mode (Escape).
    pub fn vim_escape(&mut self) {
        match self.vim_mode {
            VimMode::Insert | VimMode::Command => {
                self.vim_mode = VimMode::Normal;
                // Vim convention: cursor moves left one on Escape from Insert
                if self.cursor > 0 {
                    self.move_left();
                }
            }
            _ => {}
        }
    }

    // ───────────────────────────────────────────────────────────────
    // Composite operations for the UI layer
    // ───────────────────────────────────────────────────────────────

    /// Submit the current input: pushes to history, clears buffer.
    /// Returns the submitted text.
    pub fn submit(&mut self) -> String {
        let text = self.value.clone();
        self.push_history(&text);
        self.value.clear();
        self.cursor = 0;
        self.selection = None;
        self.history_cursor = None;
        self.history_stash.clear();
        text
    }

    /// Swap the two characters before the cursor (Ctrl+T / transpose).
    pub fn transpose_chars(&mut self) {
        if self.value.len() < 2 || self.cursor == 0 {
            return;
        }
        self.save_undo();

        // If at end, transpose the last two chars
        let pos = if self.cursor >= self.value.len() {
            prev_char_boundary(&self.value, self.value.len())
        } else {
            self.cursor
        };
        let prev = prev_char_boundary(&self.value, pos);

        if prev == pos {
            return;
        }

        let ch_a: String = self.value[prev..pos].to_string();
        let next_end = next_char_boundary(&self.value, pos);
        let ch_b: String = self.value[pos..next_end].to_string();

        let mut new = String::with_capacity(self.value.len());
        new.push_str(&self.value[..prev]);
        new.push_str(&ch_b);
        new.push_str(&ch_a);
        new.push_str(&self.value[next_end..]);
        self.value = new;
        self.cursor = next_end;
        self.redo_stack.clear();
    }

    /// Get a text representation of the buffer with a cursor marker (for debugging).
    pub fn debug_display(&self) -> String {
        let mut s = String::new();
        s.push_str(&self.value[..self.cursor]);
        s.push('|');
        s.push_str(&self.value[self.cursor..]);
        s
    }
}

impl Default for InputEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InputEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Char boundary helpers
// ═══════════════════════════════════════════════════════════════════

/// Find the next char boundary after `pos`.
fn next_char_boundary(s: &str, pos: usize) -> usize {
    let mut end = pos + 1;
    while end < s.len() && !s.is_char_boundary(end) {
        end += 1;
    }
    end.min(s.len())
}

/// Find the previous char boundary before `pos`.
fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut start = pos - 1;
    while start > 0 && !s.is_char_boundary(start) {
        start -= 1;
    }
    start
}

// ═══════════════════════════════════════════════════════════════════
// Inline unit tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor() {
        let ed = InputEditor::new();
        assert!(ed.is_empty());
        assert_eq!(ed.cursor(), 0);
        assert!(ed.at_start());
        assert!(ed.at_end());
    }

    #[test]
    fn test_insert_and_cursor() {
        let mut ed = InputEditor::new();
        ed.insert_char('a');
        ed.insert_char('b');
        ed.insert_char('c');
        assert_eq!(ed.value, "abc");
        assert_eq!(ed.cursor(), 3);
        assert!(ed.at_end());
    }

    #[test]
    fn test_backspace() {
        let mut ed = InputEditor::new();
        ed.insert_str("hello");
        ed.backspace();
        assert_eq!(ed.value, "hell");
        assert_eq!(ed.cursor(), 4);
    }

    #[test]
    fn test_move_and_delete() {
        let mut ed = InputEditor::new();
        ed.insert_str("abc");
        ed.move_home();
        ed.delete();
        assert_eq!(ed.value, "bc");
    }
}