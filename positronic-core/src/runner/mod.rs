use crate::airlock::Airlock;
use crate::pty_manager::PtyManager;
use crate::vault::Vault;

use anyhow::Result;
use positronic_hive::HiveNode;
use positronic_io::HardwareMonitor;
use positronic_neural::cortex::NeuralClient;
use positronic_script::wasm_host::WasmHost;

use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct Runner {
    pty: Arc<Mutex<PtyManager>>,
    #[allow(dead_code)]
    airlock: Arc<Airlock>,
    #[allow(dead_code)]
    neural: Arc<NeuralClient>,
    #[allow(dead_code)]
    vault: Vault,
    #[allow(dead_code)]
    wasm_host: Arc<WasmHost>,
    #[allow(dead_code)]
    hive: Arc<HiveNode>,
    #[allow(dead_code)]
    io: Arc<HardwareMonitor>,
}

impl Runner {
    pub fn new(
        pty: Arc<Mutex<PtyManager>>,
        airlock: Arc<Airlock>,
        neural: Arc<NeuralClient>,
        vault: Vault,
        wasm_host: Arc<WasmHost>,
        hive: Arc<HiveNode>,
        io: Arc<HardwareMonitor>,
    ) -> Self {
        Self {
            pty,
            airlock,
            neural,
            vault,
            wasm_host,
            hive,
            io,
        }
    }

    /// Execute user input by forwarding it into the PTY.
    pub async fn execute(&self, data: &str) -> Result<()> {
        // Normalize incoming UI text:
        // - accept \r\n or \n
        // - strip trailing newlines (PTY write_line adds one)
        let mut normalized = data.replace("\r\n", "\n");
        while normalized.ends_with('\n') {
            normalized.pop();
        }

        let mut pty = self.pty.lock().await;

        // If user hit Enter on an empty line, still send a newline to get prompt/output.
        if normalized.trim().is_empty() {
            let _ = pty.write_line("");
            return Ok(());
        }

        pty.write_line(&normalized)?;
        Ok(())
    }
}
