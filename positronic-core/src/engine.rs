use crate::pty_manager::PtyManager;
use crate::state_machine::StateMachine;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

/// The main entry point for the Positronic Core.
/// The UI holds one instance of this.
pub struct PositronicEngine {
    pub pty: Arc<Mutex<PtyManager>>,
    pub state: Arc<StateMachine>,
    // Channel to notify UI that the screen changed (needs redraw)
    redraw_notifier: mpsc::Sender<()>, 
}

impl PositronicEngine {
    /// Starts the engine: Spawns PTY, starts background threads, returns the instance.
    pub async fn start(cols: u16, rows: u16, redraw_tx: mpsc::Sender<()>) -> Result<Self> {
        // 1. Create the PTY
        let mut pty = PtyManager::new(cols, rows)?;
        
        // 2. Create the State Machine (Headless Terminal)
        let state = Arc::new(StateMachine::new(cols, rows));
        
        // 3. Start the PTY Reader Thread
        let mut reader_rx = pty.start_reader()?;
        
        let engine = Self {
            pty: Arc::new(Mutex::new(pty)),
            state: state.clone(),
            redraw_notifier: redraw_tx,
        };

        // 4. Spawn the "Pump" Task (Connects PTY -> State Machine)
        let state_clone = state.clone();
        let notifier = engine.redraw_notifier.clone();
        
        tokio::spawn(async move {
            while let Some(bytes) = reader_rx.recv().await {
                // A. Parse the bytes into the grid
                state_clone.process_bytes(&bytes);
                
                // B. Notify UI to redraw
                // We ignore error (it means UI closed)
                let _ = notifier.send(()).await;
            }
        });

        Ok(engine)
    }

    /// User typed something in the UI -> Send to Shell
    pub async fn send_input(&self, data: &str) -> Result<()> {
        let mut pty = self.pty.lock().await;
        pty.write(data)
    }

    /// UI Window Resized -> Resize PTY and Grid
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        // Resize PTY (OS level)
        let mut pty = self.pty.lock().await;
        pty.resize(cols, rows)?;
        
        // Resize Grid (Alacritty level)
        self.state.resize(cols, rows);
        
        Ok(())
    }
}
