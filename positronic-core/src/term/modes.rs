/// Lightweight CSI mode tracker (chunk-safe enough for our gate).
///
/// We only care about a small set of DEC private modes:
/// - Alt screen: ?47 / ?1047 / ?1049
/// - Mouse: ?1000/?1002/?1003/?1006/?1015
/// - Bracketed paste: ?2004
/// - App cursor: ?1
#[derive(Debug, Clone, Copy, Default)]
pub struct ModeSnapshot {
    pub alt_screen: bool,
    pub mouse_reporting: bool,
    pub bracketed_paste: bool,
    pub app_cursor: bool,
}

impl ModeSnapshot {
    /// If false, Intelli-Input + overlays should back off and use raw PTY passthrough.
    pub fn intelli_safe(&self) -> bool {
        !self.alt_screen && !self.mouse_reporting
    }
}

#[derive(Debug, Default, Clone)]
pub struct ModeTracker {
    snap: ModeSnapshot,

    // tiny CSI parser state
    state: ParseState,
    cur_num: Option<u16>,
    nums: Vec<u16>,
    private: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    Ground,
    Esc,
    Csi,
}

impl Default for ParseState {
    fn default() -> Self {
        ParseState::Ground
    }
}

impl ModeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> ModeSnapshot {
        self.snap
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            match self.state {
                ParseState::Ground => {
                    if b == 0x1b {
                        self.state = ParseState::Esc;
                    }
                }
                ParseState::Esc => {
                    if b == b'[' {
                        self.state = ParseState::Csi;
                        self.private = false;
                        self.cur_num = None;
                        self.nums.clear();
                    } else {
                        self.state = ParseState::Ground;
                    }
                }
                ParseState::Csi => {
                    // private prefix
                    if self.nums.is_empty() && self.cur_num.is_none() && !self.private && b == b'?' {
                        self.private = true;
                        continue;
                    }

                    if b.is_ascii_digit() {
                        let d = (b - b'0') as u16;
                        self.cur_num = Some(self.cur_num.unwrap_or(0).saturating_mul(10).saturating_add(d));
                        continue;
                    }

                    if b == b';' {
                        if let Some(n) = self.cur_num.take() {
                            self.nums.push(n);
                        } else {
                            self.nums.push(0);
                        }
                        continue;
                    }

                    // final byte
                    if let Some(n) = self.cur_num.take() {
                        self.nums.push(n);
                    }

                    if self.private && (b == b'h' || b == b'l') {
                        let set = b == b'h';
                        self.apply_private_modes(set);
                    }

                    self.state = ParseState::Ground;
                }
            }
        }
    }

    fn apply_private_modes(&mut self, set: bool) {
        for &m in &self.nums {
            match m {
                47 | 1047 | 1049 => self.snap.alt_screen = set,
                1000 | 1002 | 1003 | 1006 | 1015 => self.snap.mouse_reporting = set,
                2004 => self.snap.bracketed_paste = set,
                1 => self.snap.app_cursor = set,
                _ => {}
            }
        }
    }
}
