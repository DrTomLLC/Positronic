// positronic-bridge/src/input/mod.rs
//
// The "Intelli-Input" editor (Roadmap Pillar II).
//
// A full terminal input editor with cursor management, selection,
// word-level navigation, kill/yank ring, undo/redo, command history,
// insert/overwrite modes, and a vim-mode state machine.
//
// All editing logic is pure Rust with zero UI dependencies.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════
// Edit Mode
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Insert,
    Overwrite,
}

impl EditMode {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    Disabled,
    Normal,
    Insert,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Selection {
    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.cursor { (self.anchor, self.cursor) } else { (self.cursor, self.anchor) }
    }
    pub fn len(&self) -> usize { let (s, e) = self.range(); e - s }
    pub fn is_empty(&self) -> bool { self.anchor == self.cursor }
}

// ═══════════════════════════════════════════════════════════════════
// Undo Entry
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct UndoEntry {
    value: String,
    cursor: usize,
}

// ═══════════════════════════════════════════════════════════════════
// InputEditor
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct InputEditor {
    pub value: String,
    cursor: usize,
    selection: Option<Selection>,
    edit_mode: EditMode,
    vim_mode: VimMode,
    kill_ring: Vec<String>,
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    max_undo: usize,
    history: Vec<String>,
    history_cursor: Option<usize>,
    history_stash: String,
}

impl InputEditor {
    pub fn new() -> Self {
        Self {
            value: String::new(), cursor: 0, selection: None,
            edit_mode: EditMode::Insert, vim_mode: VimMode::Disabled,
            kill_ring: Vec::new(), undo_stack: Vec::new(), redo_stack: Vec::new(),
            max_undo: 100, history: Vec::new(), history_cursor: None, history_stash: String::new(),
        }
    }

    // ─── Cursor queries ──────────────────────────────────────────

    pub fn cursor(&self) -> usize { self.cursor }
    pub fn set_cursor(&mut self, pos: usize) { self.cursor = pos.min(self.value.len()); self.selection = None; }
    pub fn at_start(&self) -> bool { self.cursor == 0 }
    pub fn at_end(&self) -> bool { self.cursor == self.value.len() }
    pub fn char_count(&self) -> usize { self.value.chars().count() }
    pub fn is_empty(&self) -> bool { self.value.is_empty() }
    pub fn len(&self) -> usize { self.value.len() }

    // ─── Character-level cursor movement ─────────────────────────

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            let mut pos = self.cursor - 1;
            while pos > 0 && !self.value.is_char_boundary(pos) { pos -= 1; }
            self.cursor = pos;
        }
        self.selection = None;
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.value.len() {
            let mut pos = self.cursor + 1;
            while pos < self.value.len() && !self.value.is_char_boundary(pos) { pos += 1; }
            self.cursor = pos;
        }
        self.selection = None;
    }

    pub fn move_home(&mut self) { self.cursor = 0; self.selection = None; }
    pub fn move_end(&mut self) { self.cursor = self.value.len(); self.selection = None; }

    // ─── Word-level cursor movement ──────────────────────────────

    pub fn move_word_left(&mut self) { self.cursor = self.find_word_boundary_left(); self.selection = None; }
    pub fn move_word_right(&mut self) { self.cursor = self.find_word_boundary_right(); self.selection = None; }

    fn find_word_boundary_left(&self) -> usize {
        if self.cursor == 0 { return 0; }
        let bytes = self.value.as_bytes();
        let mut pos = self.cursor;
        while pos > 0 && bytes[pos - 1].is_ascii_whitespace() { pos -= 1; }
        while pos > 0 && !bytes[pos - 1].is_ascii_whitespace() { pos -= 1; }
        pos
    }

    fn find_word_boundary_right(&self) -> usize {
        let len = self.value.len();
        if self.cursor >= len { return len; }
        let bytes = self.value.as_bytes();
        let mut pos = self.cursor;
        while pos < len && !bytes[pos].is_ascii_whitespace() { pos += 1; }
        while pos < len && bytes[pos].is_ascii_whitespace() { pos += 1; }
        pos
    }

    // ─── Selection ───────────────────────────────────────────────

    pub fn selection(&self) -> Option<Selection> { self.selection }

    pub fn select_left(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        if self.cursor > 0 {
            let mut pos = self.cursor - 1;
            while pos > 0 && !self.value.is_char_boundary(pos) { pos -= 1; }
            self.cursor = pos;
        }
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    pub fn select_right(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        if self.cursor < self.value.len() {
            let mut pos = self.cursor + 1;
            while pos < self.value.len() && !self.value.is_char_boundary(pos) { pos += 1; }
            self.cursor = pos;
        }
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    pub fn select_home(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        self.cursor = 0;
        self.selection = Some(Selection { anchor, cursor: 0 });
        self.collapse_empty_selection();
    }

    pub fn select_end(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        self.cursor = self.value.len();
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    pub fn select_word_left(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        self.cursor = self.find_word_boundary_left();
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    pub fn select_word_right(&mut self) {
        let anchor = self.selection.map(|s| s.anchor).unwrap_or(self.cursor);
        self.cursor = self.find_word_boundary_right();
        self.selection = Some(Selection { anchor, cursor: self.cursor });
        self.collapse_empty_selection();
    }

    pub fn select_all(&mut self) {
        if self.value.is_empty() { self.selection = None; } else {
            self.selection = Some(Selection { anchor: 0, cursor: self.value.len() });
            self.cursor = self.value.len();
        }
    }

    pub fn deselect(&mut self) { self.selection = None; }

    pub fn selected_text(&self) -> Option<&str> {
        self.selection.map(|sel| { let (s, e) = sel.range(); &self.value[s..e] })
    }

    fn collapse_empty_selection(&mut self) {
        if let Some(sel) = &self.selection { if sel.is_empty() { self.selection = None; } }
    }

    // ─── Text insertion and deletion ─────────────────────────────

    pub fn insert_char(&mut self, ch: char) {
        self.save_undo();
        self.delete_selection_internal();
        if self.edit_mode == EditMode::Overwrite && self.cursor < self.value.len() {
            let end = next_char_boundary(&self.value, self.cursor);
            self.value.replace_range(self.cursor..end, "");
        }
        self.value.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.redo_stack.clear();
    }

    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() { return; }
        self.save_undo();
        self.delete_selection_internal();
        self.value.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.redo_stack.clear();
    }

    pub fn backspace(&mut self) {
        if self.selection.is_some() { self.save_undo(); self.delete_selection_internal(); self.redo_stack.clear(); return; }
        if self.cursor == 0 { return; }
        self.save_undo();
        let prev = prev_char_boundary(&self.value, self.cursor);
        self.value.replace_range(prev..self.cursor, "");
        self.cursor = prev;
        self.redo_stack.clear();
    }

    pub fn delete(&mut self) {
        if self.selection.is_some() { self.save_undo(); self.delete_selection_internal(); self.redo_stack.clear(); return; }
        if self.cursor >= self.value.len() { return; }
        self.save_undo();
        let next = next_char_boundary(&self.value, self.cursor);
        self.value.replace_range(self.cursor..next, "");
        self.redo_stack.clear();
    }

    fn delete_selection_internal(&mut self) {
        if let Some(sel) = self.selection.take() {
            let (start, end) = sel.range();
            self.value.replace_range(start..end, "");
            self.cursor = start;
        }
    }

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
        } else { None }
    }

    pub fn set_value(&mut self, s: &str) {
        self.value = s.to_string();
        self.cursor = self.value.len();
        self.selection = None;
    }

    pub fn clear(&mut self) {
        if !self.value.is_empty() {
            self.save_undo();
            self.value.clear();
            self.cursor = 0;
            self.selection = None;
            self.redo_stack.clear();
        }
    }

    // ─── Kill/Yank ───────────────────────────────────────────────

    pub fn kill_to_end(&mut self) {
        if self.cursor >= self.value.len() { return; }
        self.save_undo();
        let killed = self.value[self.cursor..].to_string();
        self.value.truncate(self.cursor);
        self.push_kill(killed);
        self.redo_stack.clear();
    }

    pub fn kill_to_start(&mut self) {
        if self.cursor == 0 { return; }
        self.save_undo();
        let killed = self.value[..self.cursor].to_string();
        self.value = self.value[self.cursor..].to_string();
        self.cursor = 0;
        self.push_kill(killed);
        self.redo_stack.clear();
    }

    pub fn kill_word_back(&mut self) {
        if self.cursor == 0 { return; }
        self.save_undo();
        let boundary = self.find_word_boundary_left();
        let killed = self.value[boundary..self.cursor].to_string();
        self.value.replace_range(boundary..self.cursor, "");
        self.cursor = boundary;
        self.push_kill(killed);
        self.redo_stack.clear();
    }

    pub fn kill_word_forward(&mut self) {
        if self.cursor >= self.value.len() { return; }
        self.save_undo();
        let boundary = self.find_word_boundary_right();
        let killed = self.value[self.cursor..boundary].to_string();
        self.value.replace_range(self.cursor..boundary, "");
        self.push_kill(killed);
        self.redo_stack.clear();
    }

    pub fn yank(&mut self) {
        if let Some(text) = self.kill_ring.last().cloned() { self.insert_str(&text); }
    }

    pub fn kill_ring(&self) -> &[String] { &self.kill_ring }

    fn push_kill(&mut self, text: String) {
        self.kill_ring.push(text);
        if self.kill_ring.len() > 10 { self.kill_ring.remove(0); }
    }

    // ─── Undo / Redo ─────────────────────────────────────────────

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            self.redo_stack.push(UndoEntry { value: self.value.clone(), cursor: self.cursor });
            self.value = entry.value;
            self.cursor = entry.cursor;
            self.selection = None;
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            self.undo_stack.push(UndoEntry { value: self.value.clone(), cursor: self.cursor });
            self.value = entry.value;
            self.cursor = entry.cursor;
            self.selection = None;
        }
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    fn save_undo(&mut self) {
        self.undo_stack.push(UndoEntry { value: self.value.clone(), cursor: self.cursor });
        if self.undo_stack.len() > self.max_undo { self.undo_stack.remove(0); }
    }

    // ─── Command History ─────────────────────────────────────────

    pub fn push_history(&mut self, command: &str) {
        if command.trim().is_empty() { return; }
        if self.history.last().map(|s| s.as_str()) == Some(command) { return; }
        self.history.push(command.to_string());
        self.history_cursor = None;
        self.history_stash.clear();
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() { return; }
        match self.history_cursor {
            None => {
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
            _ => {}
        }
    }

    pub fn history_down(&mut self) {
        match self.history_cursor {
            Some(idx) => {
                if idx + 1 < self.history.len() {
                    let new_idx = idx + 1;
                    self.history_cursor = Some(new_idx);
                    self.set_value(&self.history[new_idx].clone());
                } else {
                    self.history_cursor = None;
                    let stash = self.history_stash.clone();
                    self.set_value(&stash);
                    self.history_stash.clear();
                }
            }
            None => {}
        }
    }

    pub fn history(&self) -> &[String] { &self.history }
    pub fn history_position(&self) -> Option<usize> { self.history_cursor }
    pub fn clear_history(&mut self) { self.history.clear(); self.history_cursor = None; self.history_stash.clear(); }

    // ─── Edit Mode ───────────────────────────────────────────────

    pub fn edit_mode(&self) -> EditMode { self.edit_mode }
    pub fn toggle_edit_mode(&mut self) { self.edit_mode = self.edit_mode.toggle(); }
    pub fn set_edit_mode(&mut self, mode: EditMode) { self.edit_mode = mode; }

    // ─── Vim Mode ────────────────────────────────────────────────

    pub fn vim_mode(&self) -> VimMode { self.vim_mode }
    pub fn enable_vim(&mut self) { self.vim_mode = VimMode::Normal; }
    pub fn disable_vim(&mut self) { self.vim_mode = VimMode::Disabled; }
    pub fn set_vim_mode(&mut self, mode: VimMode) { self.vim_mode = mode; }

    pub fn vim_motion(&mut self, ch: char) -> bool {
        if self.vim_mode != VimMode::Normal { return false; }
        match ch {
            'h' => { self.move_left(); true }
            'l' => { self.move_right(); true }
            '0' => { self.move_home(); true }
            '$' => { self.move_end(); true }
            'w' => { self.move_word_right(); true }
            'b' => { self.move_word_left(); true }
            'i' => { self.vim_mode = VimMode::Insert; true }
            'a' => { self.move_right(); self.vim_mode = VimMode::Insert; true }
            'A' => { self.move_end(); self.vim_mode = VimMode::Insert; true }
            'I' => { self.move_home(); self.vim_mode = VimMode::Insert; true }
            'x' => { self.delete(); true }
            'X' => { self.backspace(); true }
            'D' => { self.kill_to_end(); true }
            'C' => { self.kill_to_end(); self.vim_mode = VimMode::Insert; true }
            'u' => { self.undo(); true }
            ':' => { self.vim_mode = VimMode::Command; true }
            _ => false,
        }
    }

    pub fn vim_escape(&mut self) {
        match self.vim_mode {
            VimMode::Insert | VimMode::Command => {
                self.vim_mode = VimMode::Normal;
                if self.cursor > 0 { self.move_left(); }
            }
            _ => {}
        }
    }

    // ─── Composite operations ────────────────────────────────────

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

    pub fn transpose_chars(&mut self) {
        if self.value.len() < 2 || self.cursor == 0 { return; }
        self.save_undo();
        let pos = if self.cursor >= self.value.len() {
            prev_char_boundary(&self.value, self.value.len())
        } else { self.cursor };
        let prev = prev_char_boundary(&self.value, pos);
        if prev == pos { return; }
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

    pub fn debug_display(&self) -> String {
        let mut s = String::new();
        s.push_str(&self.value[..self.cursor]);
        s.push('|');
        s.push_str(&self.value[self.cursor..]);
        s
    }
}

impl Default for InputEditor {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for InputEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.value) }
}

// ═══════════════════════════════════════════════════════════════════
// Char boundary helpers
// ═══════════════════════════════════════════════════════════════════

fn next_char_boundary(s: &str, pos: usize) -> usize {
    let mut end = pos + 1;
    while end < s.len() && !s.is_char_boundary(end) { end += 1; }
    end.min(s.len())
}

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 { return 0; }
    let mut start = pos - 1;
    while start > 0 && !s.is_char_boundary(start) { start -= 1; }
    start
}

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