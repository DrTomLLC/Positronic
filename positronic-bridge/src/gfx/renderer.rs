//! wgpu device/surface lifecycle and per-frame orchestration.
//!
//! GpuState owns the device, queue, surface, and config. It creates the
//! swapchain texture each frame and hands it to the quad + text pipelines.

use std::sync::Arc;
use wgpu::{
    Backends, Device, DeviceDescriptor, Instance, InstanceDescriptor, PowerPreference, Queue,
    RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat, TextureUsages,
    TextureViewDescriptor,
};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use super::quad::QuadPipeline;
use super::text::TextEngine;
use crate::renderer::Rgba;

/// Owns all GPU state. Created once per window.
pub struct GpuState {
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
    pub format: TextureFormat,

    // Pipelines
    pub quads: QuadPipeline,
    pub text: TextEngine,
}

impl GpuState {
    /// Initialize wgpu with the given window. Blocks until the adapter is ready.
    pub fn new(window: Arc<dyn Window>) -> anyhow::Result<Self> {
        let size = window.outer_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
            .or_else(|_| anyhow::anyhow!("No suitable GPU adapter found"))?;

        tracing::info!(
            "GPU adapter: {} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let (device, queue) = pollster::block_on(adapter.request_device(
            &DeviceDescriptor {
                label: Some("positronic-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
        ))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let quads = QuadPipeline::new(&device, format);
        let text = TextEngine::new(&device, &queue, format);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: PhysicalSize::new(width, height),
            format,
            quads,
            text,
        })
    }

    /// Handle window resize.
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Render a frame. Returns Ok(true) if a frame was presented, Ok(false) if skipped.
    pub fn render_frame(
        &mut self,
        clear_color: Rgba,
        draw_fn: impl FnOnce(&mut QuadPipeline, &mut TextEngine, &Device, &Queue, [u32; 2]),
    ) -> anyhow::Result<bool> {
        let output = match self.surface.get_current_texture() {
            Ok(tex) => tex,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                return Err(anyhow::anyhow!("GPU out of memory"));
            }
            Err(e) => {
                tracing::warn!("Surface error: {:?}", e);
                return Ok(false);
            }
        };

        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        let viewport = [self.config.width, self.config.height];

        // Let the caller populate quad + text data
        draw_fn(
            &mut self.quads,
            &mut self.text,
            &self.device,
            &self.queue,
            viewport,
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-encoder"),
            });

        // Main render pass
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color.r as f64,
                            g: clear_color.g as f64,
                            b: clear_color.b as f64,
                            a: clear_color.a as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Draw quads (backgrounds, cursor, selection)
            self.quads
                .render(&mut pass, &self.device, &self.queue, viewport);
        }

        // Text pass (glyphon renders after quads so text is on top)
        self.text.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            self.format,
            viewport,
        );

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Clear transient data for next frame
        self.quads.clear();

        Ok(true)
    }
}