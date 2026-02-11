use alacritty_terminal::event::{EventListener, WindowSize};
use alacritty_terminal::grid::Grid;
use alacritty_terminal::index::{Direction, Point};
use alacritty_terminal::term::{Term, TermMode, cell::Cell};
use alacritty_terminal::vte::ansi;
use std::sync::{Arc, Mutex};
use std::io::Sink;

/// The "Brain" of the terminal.
/// It parses ANSI escape codes and maintains the grid state (what char is where).
pub struct StateMachine {
    // Alacritty's internal Terminal state.
    // We wrap it in Arc<Mutex> because the PTY reader thread writes to it,
    // and the UI thread reads from it.
    pub term: Arc<Mutex<Term<EventProxy>>>,
}

impl StateMachine {
    pub fn resize(&self, cols: u16, rows: u16) {
        let mut term = self.term.lock().unwrap();
        let size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width: 1,
            cell_height: 1,
        };
        term.resize(size);
    }
    
    pub fn new(cols: u16, rows: u16) -> Self {
        let size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width: 1,  // Not used for headless
            cell_height: 1, // Not used for headless
        };
        
        let config = alacritty_terminal::term::Config::default();
        let proxy = EventProxy {}; // We ignore internal Alacritty events for now

        let term = Term::new(&config, &size, proxy);

        Self {
            term: Arc::new(Mutex::new(term)),
        }
    }

    /// Process raw bytes from the PTY and update the Grid.
    /// This is the "Tick" function called by the Reader thread.
    pub fn process_bytes(&self, bytes: &[u8]) {
        let mut term = self.term.lock().unwrap();
        let mut parser = ansi::Processor::new();

        // Alacritty's parser takes a Writer for the "Response" (e.g. asking "Who are you?")
        // We dump that into a Sink for now, but really it should go back to the PTY.
        let mut sink = Sink::default(); 

        for byte in bytes {
            parser.advance(&mut *term, *byte, &mut sink);
        }
    }

    /// Returns a snapshot of the current visible grid (for the UI to render).
    /// This converts Alacritty's complex internal grid into a simple Vec of Strings/Cells.
    pub fn snapshot(&self) -> Vec<Vec<(char, MyColor)>> {
        let term = self.term.lock().unwrap();
        let grid = term.grid();
        
        let mut output = Vec::new();

        // Iterate over visible rows
        for row in grid.display_iter() {
            let mut row_vec = Vec::new();
            for cell in row {
                // Simplified color extraction (you'll need a real mapping later)
                let color = MyColor::from_alacritty(cell.fg); 
                row_vec.push((cell.c, color));
            }
            output.push(row_vec);
        }
        output
    }
}

// --- Helper Types ---

/// A dummy event listener required by Alacritty.
#[derive(Clone)]
pub struct EventProxy;

impl EventListener for EventProxy {
    fn send_event(&self, _event: alacritty_terminal::event::Event) {
        // We don't care about window events here, the Bridge handles them.
    }
}

/// A simplified color struct for your UI to consume.
#[derive(Debug, Clone, Copy)]
pub enum MyColor {
    Default,
    Red,
    Green,
    // ... add others
}

impl MyColor {
    fn from_alacritty(fg: ansi::Color) -> Self {
        // TODO: Map Alacritty's 256 colors to our simple enum
        // This is a placeholder
        MyColor::Default
    }
}
