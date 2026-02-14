use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte::ansi;

use std::ops::Index;
use std::sync::{Arc, Mutex};

// --- Helper Types for Dimensions ---

#[derive(Clone, Copy, Debug)]
struct TermSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

// --- Event Proxy (Required Stub) ---

#[derive(Clone)]
pub struct EventProxy;

impl EventListener for EventProxy {
    fn send_event(&self, _event: Event) {
        // Headless: No events needed
    }
}

// --- Color Handling ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyColor {
    Default,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Rgb(u8, u8, u8),
    Indexed(u8),
}

impl MyColor {
    fn from_alacritty(fg: ansi::Color) -> Self {
        match fg {
            ansi::Color::Named(name) => match name {
                ansi::NamedColor::Black => MyColor::Black,
                ansi::NamedColor::Red => MyColor::Red,
                ansi::NamedColor::Green => MyColor::Green,
                ansi::NamedColor::Yellow => MyColor::Yellow,
                ansi::NamedColor::Blue => MyColor::Blue,
                ansi::NamedColor::Magenta => MyColor::Magenta,
                ansi::NamedColor::Cyan => MyColor::Cyan,
                ansi::NamedColor::White => MyColor::White,

                ansi::NamedColor::BrightBlack => MyColor::BrightBlack,
                ansi::NamedColor::BrightRed => MyColor::BrightRed,
                ansi::NamedColor::BrightGreen => MyColor::BrightGreen,
                ansi::NamedColor::BrightYellow => MyColor::BrightYellow,
                ansi::NamedColor::BrightBlue => MyColor::BrightBlue,
                ansi::NamedColor::BrightMagenta => MyColor::BrightMagenta,
                ansi::NamedColor::BrightCyan => MyColor::BrightCyan,
                ansi::NamedColor::BrightWhite => MyColor::BrightWhite,

                // Treat these as "use default" for snapshotting.
                ansi::NamedColor::Foreground
                | ansi::NamedColor::Background
                | ansi::NamedColor::Cursor
                | ansi::NamedColor::BrightForeground
                | ansi::NamedColor::DimForeground => MyColor::Default,

                // Dim colors: map to their base equivalents for now.
                ansi::NamedColor::DimBlack => MyColor::Black,
                ansi::NamedColor::DimRed => MyColor::Red,
                ansi::NamedColor::DimGreen => MyColor::Green,
                ansi::NamedColor::DimYellow => MyColor::Yellow,
                ansi::NamedColor::DimBlue => MyColor::Blue,
                ansi::NamedColor::DimMagenta => MyColor::Magenta,
                ansi::NamedColor::DimCyan => MyColor::Cyan,
                ansi::NamedColor::DimWhite => MyColor::White,
            },
            ansi::Color::Spec(rgb) => MyColor::Rgb(rgb.r, rgb.g, rgb.b),
            ansi::Color::Indexed(idx) => MyColor::Indexed(idx),
        }
    }
}

// --- Snapshot (flat storage, 2D semantics for tests) ---

pub type SnapshotCell = (char, MyColor);

#[derive(Debug, Clone)]
pub struct Snapshot {
    cols: usize,
    rows: usize,
    pub cells: Vec<SnapshotCell>, // row-major: row * cols + col
}

impl Snapshot {
    pub fn new(cols: usize, rows: usize) -> Self {
        let len = cols.saturating_mul(rows);
        Self {
            cols,
            rows,
            cells: vec![(' ', MyColor::Default); len],
        }
    }

    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Matches the old `Vec<Vec<_>>` semantics your tests expect:
    /// `snapshot.len()` == number of rows.
    #[inline]
    pub fn len(&self) -> usize {
        self.rows
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows == 0 || self.cols == 0
    }

    #[inline]
    fn ensure_size(&mut self, cols: usize, rows: usize) {
        if self.cols == cols && self.rows == rows {
            return;
        }

        self.cols = cols;
        self.rows = rows;

        let len = cols.saturating_mul(rows);
        self.cells.resize(len, (' ', MyColor::Default));
    }

    #[inline]
    fn clear(&mut self) {
        self.cells.fill((' ', MyColor::Default));
    }
}

impl Index<usize> for Snapshot {
    type Output = [SnapshotCell];

    #[inline]
    fn index(&self, row: usize) -> &Self::Output {
        let start = row
            .checked_mul(self.cols)
            .expect("row index overflow computing snapshot slice start");
        let end = start + self.cols;
        &self.cells[start..end]
    }
}

pub struct SnapshotRows<'a> {
    snap: &'a Snapshot,
    row: usize,
}

impl<'a> Iterator for SnapshotRows<'a> {
    type Item = &'a [SnapshotCell];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.snap.rows {
            return None;
        }
        let r = self.row;
        self.row += 1;
        Some(&self.snap[r])
    }
}

impl<'a> IntoIterator for &'a Snapshot {
    type Item = &'a [SnapshotCell];
    type IntoIter = SnapshotRows<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        SnapshotRows { snap: self, row: 0 }
    }
}

// --- The State Machine ---

struct Inner {
    term: Term<EventProxy>,
    parser: ansi::Processor,
}

pub struct StateMachine {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for StateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateMachine")
            .field("inner", &"Mutex<Inner>")
            .finish()
    }
}

impl StateMachine {
    pub fn new(cols: u16, rows: u16) -> Self {
        let size = TermSize {
            cols: cols as usize,
            rows: rows as usize,
        };

        let config = TermConfig::default();
        let term = Term::new(config, &size, EventProxy);
        let parser = ansi::Processor::new();

        Self {
            inner: Arc::new(Mutex::new(Inner { term, parser })),
        }
    }

    #[inline]
    fn lock_inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Feed raw PTY/serial bytes into the terminal emulator.
    /// Keeps parser state across calls (important for escape sequences split across chunks).
    pub fn process_bytes(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        let mut inner = self.lock_inner();

        // Split the mutable borrow safely.
        let Inner { term, parser } = &mut *inner;

        /*for &byte in bytes {
            parser.advance(term, byte);
        }*/
        for &byte in bytes.iter() {
            parser.advance(term, &[byte]);
        }
    }

    /// Allocate a fresh snapshot.
    /// If you want to reuse memory, prefer `snapshot_into`.
    pub fn snapshot(&self) -> Snapshot {
        let mut out = Snapshot::new(0, 0);
        self.snapshot_into(&mut out);
        out
    }

    /// Fill an existing snapshot buffer (reuses allocation when dimensions are unchanged).
    pub fn snapshot_into(&self, out: &mut Snapshot) {
        let inner = self.lock_inner();

        let cols = inner.term.columns();
        let rows = inner.term.screen_lines();

        out.ensure_size(cols, rows);
        out.clear();

        let grid = inner.term.grid();

        for indexed in grid.display_iter() {
            let col = indexed.point.column.0;

            // IMPORTANT: Line is often i32. Guard negatives.
            let line_i32 = indexed.point.line.0;
            if line_i32 < 0 {
                continue;
            }
            let line = line_i32 as usize;

            if line >= rows || col >= cols {
                continue;
            }

            let cell = indexed.cell;
            let idx = line * cols + col;

            out.cells[idx] = (cell.c, MyColor::from_alacritty(cell.fg));
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        let mut inner = self.lock_inner();

        let size = TermSize {
            cols: cols as usize,
            rows: rows as usize,
        };

        inner.term.resize(size);
    }
}
