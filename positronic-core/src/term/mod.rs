//! Terminal-side parsing helpers that sit *next to* the emulator.
//!
//! - `osc`: streaming OSC parser (OSC 7 cwd, OSC 133 prompt markers, etc.)
//! - `modes`: lightweight CSI mode tracker (alt-screen, mouse reporting, bracketed paste)
//! - `semantic`: prompt/command state derived from OSC markers

pub mod modes;
pub mod osc;
pub mod semantic;
