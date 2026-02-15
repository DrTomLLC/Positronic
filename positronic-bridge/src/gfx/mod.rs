//! GPU Rendering Subsystem.
//!
//! Manages the wgpu device, surface, and rendering pipelines.
//! Sub-modules:
//!   renderer — wgpu device/surface lifecycle, frame orchestration
//!   quad     — colored rectangle pipeline (backgrounds, cursor, selection)
//!   text     — glyphon-based text rendering

mod quad;
mod renderer;
pub(crate) mod text;

pub use quad::{QuadInstance, QuadPipeline};
pub use renderer::GpuState;
pub use text::TextEngine;