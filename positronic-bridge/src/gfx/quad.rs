// positronic-bridge/src/gfx/quad.rs
//! Colored rectangle pipeline.
//!
//! Draws filled quads (backgrounds, cursor, selection, status bar, etc.)
//! using the shapes.wgsl vertex/fragment shader. Each quad is two triangles
//! with per-vertex color.

use wgpu::{Buffer, BufferUsages, Device, Queue, RenderPass, RenderPipeline, TextureFormat};

use crate::renderer::Rgba;

// ════════════════════════════════════════════════════════════════════
// Vertex Layout
// ════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

impl QuadVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Globals Uniform
// ════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    resolution: [f32; 2],
    _pad: [f32; 2],
}

// ════════════════════════════════════════════════════════════════════
// Quad Instance (public API)
// ════════════════════════════════════════════════════════════════════

/// A single colored rectangle to draw. Coordinates in pixels.
#[derive(Debug, Clone, Copy)]
pub struct QuadInstance {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Rgba,
}

// ════════════════════════════════════════════════════════════════════
// Pipeline
// ════════════════════════════════════════════════════════════════════

pub struct QuadPipeline {
    pipeline: RenderPipeline,
    globals_buffer: Buffer,
    globals_bind_group: wgpu::BindGroup,
    vertices: Vec<QuadVertex>,
}

impl QuadPipeline {
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shapes.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shapes.wgsl").into()),
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("quad-bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("quad-bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        // wgpu 28: push_constant_ranges removed, use immediate_size instead
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad-pl"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        // wgpu 28: multiview removed, use multiview_mask instead
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
            multiview_mask: None,
        });

        Self {
            pipeline,
            globals_buffer,
            globals_bind_group,
            vertices: Vec::new(),
        }
    }

    /// Queue a colored rectangle for this frame.
    pub fn push(&mut self, quad: QuadInstance) {
        let c = [quad.color.r, quad.color.g, quad.color.b, quad.color.a];
        let x0 = quad.x;
        let y0 = quad.y;
        let x1 = quad.x + quad.w;
        let y1 = quad.y + quad.h;

        // Two triangles per quad
        self.vertices
            .push(QuadVertex { pos: [x0, y0], color: c });
        self.vertices
            .push(QuadVertex { pos: [x1, y0], color: c });
        self.vertices
            .push(QuadVertex { pos: [x0, y1], color: c });

        self.vertices
            .push(QuadVertex { pos: [x1, y0], color: c });
        self.vertices
            .push(QuadVertex { pos: [x1, y1], color: c });
        self.vertices
            .push(QuadVertex { pos: [x0, y1], color: c });
    }

    /// Clear all queued quads for the next frame.
    pub fn clear(&mut self) {
        self.vertices.clear();
    }

    /// Render all queued quads in the given render pass.
    pub fn render(
        &self,
        pass: &mut RenderPass<'_>,
        device: &Device,
        queue: &Queue,
        viewport: [u32; 2],
    ) {
        if self.vertices.is_empty() {
            return;
        }

        // Update globals uniform
        let globals = Globals {
            resolution: [viewport[0] as f32, viewport[1] as f32],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        // Create vertex buffer from current frame data
        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad-vb"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: BufferUsages::VERTEX,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.globals_bind_group, &[]);
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.draw(0..self.vertices.len() as u32, 0..1);
    }
}

// Re-export for buffer init
use wgpu::util::DeviceExt;