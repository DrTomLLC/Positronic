//! Block model + recorder.
//!
//! A "block" is an atomic terminal interaction:
//! - command
//! - output (raw text / ansi)
//! - exit code
//! - cwd
//! - timing
//!
//! Recorder glues together OSC semantic markers + PTY output.

pub mod model;
pub mod recorder;

pub use model::{BlockId, TerminalBlockV2};
pub use recorder::{BlockRecorder, RecorderEvent};
