//! Glyphon-based text rendering engine.
//!
//! Manages the font system, text buffer layout, and GPU rendering.
//! Each frame, the caller pushes ColoredSpan data and screen regions,
//! then render() uploads glyphs and draws them.

use glyphon::{
    Attrs, Buffer as GlyphonBuffer, Cache, Color as GColor, Family, FontSystem, Metrics,
    Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer,
};
use wgpu::{CommandEncoder, Device, MultisampleState, Queue, TextureFormat, TextureView};

use crate::renderer::{ColoredSpan, Rgba};

/// Font metrics for the terminal monospace font.
const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 18.0;

/// A region of text to render on screen.
pub struct TextRegion {
    pub spans: Vec<ColoredSpan>,
    pub bounds: TextBounds,
    pub left: f32,
    pub top: f32,
    pub scale: f32,
    pub default_color: Rgba,
}

/// The text rendering engine. Wraps glyphon's font system and atlas.
pub struct TextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    renderer: TextRenderer,
    regions: Vec<TextRegion>,
}

impl TextEngine {
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let renderer = TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);

        Self {
            font_system,
            swash_cache,
            atlas,
            renderer,
            regions: Vec::new(),
        }
    }

    /// Queue a text region for rendering this frame.
    pub fn push_region(&mut self, region: TextRegion) {
        self.regions.push(region);
    }

    /// Clear all queued regions.
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Get the monospace cell size (width, height) at current font size.
    pub fn cell_size(&mut self) -> (f32, f32) {
        // Create a temp buffer to measure 'M'
        let mut buffer = GlyphonBuffer::new(&mut self.font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        buffer.set_size(&mut self.font_system, Some(1000.0), Some(LINE_HEIGHT));
        buffer.set_text(&mut self.font_system, "M", &Attrs::new().family(Family::Monospace), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Measure the glyph advance
        let width = buffer
            .layout_runs()
            .next()
            .and_then(|run| run.glyphs.first())
            .map(|g| g.w)
            .unwrap_or(8.0);

        (width, LINE_HEIGHT)
    }

    /// Render all queued text regions. Called once per frame after the main render pass.
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        format: TextureFormat,
        viewport: [u32; 2],
    ) {
        if self.regions.is_empty() {
            return;
        }

        let resolution = Resolution {
            width: viewport[0],
            height: viewport[1],
        };

        // Build glyphon buffers for each region
        let mut text_areas: Vec<TextArea<'_>> = Vec::new();
        let mut buffers: Vec<GlyphonBuffer> = Vec::new();

        for region in &self.regions {
            let mut buffer =
                GlyphonBuffer::new(&mut self.font_system, Metrics::new(FONT_SIZE * region.scale, LINE_HEIGHT * region.scale));

            let bounds_w = (region.bounds.right - region.bounds.left) as f32;
            let bounds_h = (region.bounds.bottom - region.bounds.top) as f32;
            buffer.set_size(&mut self.font_system, Some(bounds_w), Some(bounds_h));

            // Build the full text with default attrs, then we'll set rich text
            let full_text: String = region.spans.iter().map(|s| s.text.as_str()).collect();

            // For now, set as single text with default color. Rich text (per-span color)
            // requires building attrs spans. We'll do a simple version first.
            let default_color = region.default_color.to_glyphon();
            let attrs = Attrs::new().family(Family::Monospace).color(default_color);

            // Build rich text spans for per-span coloring
            let mut attrs_spans: Vec<(&str, Attrs<'_>)> = Vec::new();
            for span in &region.spans {
                let span_attrs = attrs.color(span.color.to_glyphon());
                attrs_spans.push((&span.text, span_attrs));
            }

            buffer.set_rich_text(&mut self.font_system, attrs_spans, attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(&mut self.font_system, false);

            buffers.push(buffer);
        }

        // Build TextArea references
        for (i, region) in self.regions.iter().enumerate() {
            text_areas.push(TextArea {
                buffer: &buffers[i],
                left: region.left,
                top: region.top,
                scale: region.scale,
                bounds: region.bounds,
                default_color: region.default_color.to_glyphon(),
                custom_glyphs: &[],
            });
        }

        // Prepare and render
        if let Err(e) = self.renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            /* &Viewport */,
            &mut self.swash_cache,
            /* &mut SwashCache */
        ) {
            tracing::warn!("Text prepare failed: {:?}", e);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear — quads already drawn
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if let Err(e) = self.renderer.render(&self.atlas, &mut pass) {
                tracing::warn!("Text render failed: {:?}", e);
            }
        }

        // Trim atlas after render
        self.atlas.trim();

        // Clear for next frame
        self.regions.clear();
    }
}