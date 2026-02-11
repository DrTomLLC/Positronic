//! # Positronic IO
//!
//! The Hardware Bridge.
//! Bypasses PTY for high-frequency Serial/USB communication.
//! Critical for "Oscilloscope Mode" and Embedded Development.

use serialport::SerialPort;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc; // Requires 'serialport' crate

/// High-frequency data point for the Oscilloscope
#[derive(Debug, Clone, Copy)]
pub struct SensorSample {
    pub timestamp: f64,
    pub value: f32,
    pub channel: u8,
}

/// Events from the Hardware Layer
#[derive(Debug, Clone)]
pub enum HardwareEvent {
    DeviceConnected(String),
    DeviceDisconnected(String),
    DataBatch(Vec<SensorSample>),
    SerialOutput(String),
    Error(String),
}

/// Configuration for a Serial Connection
#[derive(Debug, Clone)]
pub struct SerialConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub flow_control: bool,
}

/// Commands sent to the IO Thread
enum IOCommand {
    Connect(SerialConfig),
    Disconnect(String),
    Scan,
    Stop,
}

/// The Hardware Monitor Engine
pub struct HardwareMonitor {
    /// Active ports are tracked by name for UI status
    active_ports: Arc<Mutex<Vec<String>>>,
    /// Command channel to the I/O thread
    cmd_tx: mpsc::Sender<IOCommand>,
}

impl std::fmt::Debug for HardwareMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HardwareMonitor").finish()
    }
}

impl HardwareMonitor {
    /// Start the IO Engine in a background thread
    pub fn start() -> (Self, mpsc::Receiver<HardwareEvent>) {
        let (event_tx, event_rx) = mpsc::channel(1024);
        let (cmd_tx, mut cmd_rx) = mpsc::channel(32);

        let monitor = Self {
            active_ports: Arc::new(Mutex::new(Vec::new())),
            cmd_tx,
        };

        // The Dedicated IO Thread
        tokio::spawn(async move {
            tracing::info!("IO Thread Started");
            // Map of open ports: PortName -> Box<dyn SerialPort>
            // We can't keep them easily in a simple loop without a select! over files.
            // For this architecture, we spawn a *Reader Task* per port.

            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    IOCommand::Connect(config) => {
                        let port_name = config.port_name.clone();
                        match serialport::new(&port_name, config.baud_rate)
                            .timeout(Duration::from_millis(10))
                            .open()
                        {
                            Ok(port) => {
                                let _ = event_tx
                                    .send(HardwareEvent::DeviceConnected(port_name.clone()))
                                    .await;
                                // Spawn a dedicated reader for this port
                                let tx_clone = event_tx.clone();
                                let mut owned_port = port; // Move ownership

                                tokio::task::spawn_blocking(move || {
                                    let mut buffer: Vec<u8> = vec![0; 1024];
                                    loop {
                                        match owned_port.read(&mut buffer) {
                                            Ok(bytes_read) if bytes_read > 0 => {
                                                let s =
                                                    String::from_utf8_lossy(&buffer[..bytes_read])
                                                        .to_string();
                                                // Send via blocking calls or channel
                                                let _ = tx_clone
                                                    .blocking_send(HardwareEvent::SerialOutput(s));
                                            }
                                            Ok(_) => {} // Zero bytes
                                            Err(ref e)
                                                if e.kind() == std::io::ErrorKind::TimedOut => {}
                                            Err(_) => break, // Port closed/error
                                        }
                                        std::thread::sleep(Duration::from_millis(1));
                                    }
                                });
                            }
                            Err(e) => {
                                let _ = event_tx
                                    .send(HardwareEvent::Error(format!(
                                        "Failed to open {}: {}",
                                        port_name, e
                                    )))
                                    .await;
                            }
                        }
                    }
                    IOCommand::Disconnect(port) => {
                        // In a real impl, we'd signal the reader thread to stop via a cancellation token map.
                        let _ = event_tx.send(HardwareEvent::DeviceDisconnected(port)).await;
                    }
                    IOCommand::Scan => {
                        match serialport::available_ports() {
                            Ok(ports) => {
                                for p in ports {
                                    // Advertise available ports
                                    let _ = event_tx
                                        .send(HardwareEvent::DeviceConnected(p.port_name))
                                        .await;
                                }
                            }
                            Err(e) => {
                                let _ = event_tx
                                    .send(HardwareEvent::Error(format!("Scan failed: {}", e)))
                                    .await;
                            }
                        }
                    }
                    IOCommand::Stop => break,
                }
            }
        });

        (monitor, event_rx)
    }

    pub async fn connect(&self, port: &str, baud: u32) -> anyhow::Result<()> {
        let config = SerialConfig {
            port_name: port.to_string(),
            baud_rate: baud,
            data_bits: 8,
            flow_control: false,
        };
        self.cmd_tx
            .send(IOCommand::Connect(config))
            .await
            .map_err(|_| anyhow::anyhow!("IO Thread Dead"))
    }

    pub async fn scan_ports(&self) -> anyhow::Result<()> {
        self.cmd_tx
            .send(IOCommand::Scan)
            .await
            .map_err(|_| anyhow::anyhow!("IO Thread Dead"))
    }
}
