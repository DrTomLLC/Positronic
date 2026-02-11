use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi;
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

// --- The State Machine ---

// --- The State Machine ---

pub struct StateMachine {
    pub term: Arc<Mutex<Term<EventProxy>>>,
}

impl std::fmt::Debug for StateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateMachine")
            .field("term", &"Term<EventProxy>")
            .finish()
    }
}

impl StateMachine {
    pub fn new(cols: u16, rows: u16) -> Self {
        let size = TermSize {
            cols: cols as usize,
            rows: rows as usize,
        };

        let config = alacritty_terminal::term::Config::default();
        let proxy = EventProxy {};

        let term = Term::new(config, &size, proxy);

        Self {
            term: Arc::new(Mutex::new(term)),
        }
    }

    pub fn process_bytes(&self, bytes: &[u8]) {
        let mut term_guard = match self.term.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let mut parser: ansi::Processor = ansi::Processor::new();

        for byte in bytes {
            parser.advance(&mut *term_guard, *byte);
        }
    }

    pub fn snapshot(&self) -> Vec<Vec<(char, MyColor)>> {
        let term_guard = match self.term.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let grid = term_guard.grid();
        let cols = term_guard.columns();
        let rows = term_guard.screen_lines();

        // Create empty buffer
        let mut output = vec![vec![(' ', MyColor::Default); cols]; rows];

        // Alacritty 0.24 display_iter() iterates all visible cells in order.
        // It returns Indexed<&Cell>. We rely on the Point to know where it goes.
        for indexed in grid.display_iter() {
            let col = indexed.point.column.0;
            let line = indexed.point.line.0;

            // Safety check against resize races
            if (line as usize) < rows && col < cols {
                let cell_char = indexed.c;
                let cell_color = MyColor::from_alacritty(indexed.fg);
                output[line as usize][col] = (cell_char, cell_color);
            }
        }
        output
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        let mut term_guard = match self.term.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let size = TermSize {
            cols: cols as usize,
            rows: rows as usize,
        };

        term_guard.resize(size);
    }
}

// --- Event Proxy (Required Stub) ---

#[derive(Clone)]
pub struct EventProxy;

impl EventListener for EventProxy {
    fn send_event(&self, _event: alacritty_terminal::event::Event) {
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
                ansi::NamedColor::Foreground => MyColor::Default,
                ansi::NamedColor::Background => MyColor::Default,
                ansi::NamedColor::Cursor => MyColor::Default,
                ansi::NamedColor::DimBlack => MyColor::Black,
                ansi::NamedColor::DimRed => MyColor::Red,
                ansi::NamedColor::DimGreen => MyColor::Green,
                ansi::NamedColor::DimYellow => MyColor::Yellow,
                ansi::NamedColor::DimBlue => MyColor::Blue,
                ansi::NamedColor::DimMagenta => MyColor::Magenta,
                ansi::NamedColor::DimCyan => MyColor::Cyan,
                ansi::NamedColor::DimWhite => MyColor::White,
                ansi::NamedColor::BrightForeground => MyColor::Default,
                ansi::NamedColor::DimForeground => MyColor::Default,
            },
            ansi::Color::Spec(rgb) => MyColor::Rgb(rgb.r, rgb.g, rgb.b),
            ansi::Color::Indexed(idx) => MyColor::Indexed(idx),
        }
    }
}
