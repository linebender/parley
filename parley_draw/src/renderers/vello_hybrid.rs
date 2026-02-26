// Copyright 2025 the Vello Authors and the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Vello Hybrid (GPU) glyph rendering backend.
//!
//! Provides [`GpuGlyphAtlas`] and the `HybridBackend` implementation of
//! `GlyphAtlasBackend`.
//!
//! Unlike the CPU backend, no local `Pixmap` storage is allocated here — the
//! GPU renderer owns atlas textures and receives pixel data through the
//! pending-upload queue.

use super::vello_renderer;
use crate::atlas::{
    AtlasCommandRecorder, AtlasSlot, GLYPH_PADDING, GlyphAtlas, GlyphCache, GlyphCacheConfig,
    GlyphCacheKey, ImageCache, PendingBitmapUpload, PendingClearRect, RasterMetrics,
};
use crate::renderers::vello_renderer::{AtlasReplayTarget, GlyphAtlasBackend, quality_for_scale};
use crate::{GlyphCaches, HintCache, OutlineCache, kurbo, peniko};
use crate::{
    Pixmap,
    colr::{ColrPainter, ColrRenderer},
    glyph::{CachedGlyphType, ColrGlyph, GlyphBitmap, GlyphRenderer, PreparedGlyph},
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use kurbo::{Affine, BezPath, Rect};
use peniko::color::palette::css::BLACK;
use peniko::color::{AlphaColor, Srgb};
use peniko::{BlendMode, Extend, Gradient, ImageQuality, ImageSampler};
use vello_common::paint::{Image, ImageId, ImageSource, PaintType, Tint};
use vello_hybrid::Scene;

/// Glyph atlas cache for the hybrid (GPU) renderer.
///
/// Wraps the shared [`GlyphAtlas`] for cache bookkeeping and pending uploads
/// but does not allocate any local pixel storage — the GPU renderer manages
/// atlas textures itself via `Renderer::write_to_atlas`.
#[derive(Debug, Default)]
pub struct GpuGlyphAtlas {
    /// Shared allocator, LRU eviction state, and pending-command queues.
    inner: GlyphAtlas,
}

impl GpuGlyphAtlas {
    /// Creates a new hybrid glyph atlas cache with default eviction settings.
    pub fn new() -> Self {
        Self {
            inner: GlyphAtlas::new(),
        }
    }

    /// Creates a new hybrid glyph atlas cache with custom eviction settings.
    pub fn with_config(eviction_config: GlyphCacheConfig) -> Self {
        Self {
            inner: GlyphAtlas::with_config(eviction_config),
        }
    }
}

/// Thin delegation to the inner [`GlyphAtlas`]. No page-level pixel storage
/// to manage here — the GPU owns atlas textures.
impl GlyphCache for GpuGlyphAtlas {
    fn get(&mut self, key: &GlyphCacheKey) -> Option<AtlasSlot> {
        self.inner.get(key)
    }

    fn insert(
        &mut self,
        image_cache: &mut ImageCache,
        key: GlyphCacheKey,
        raster_metrics: RasterMetrics,
    ) -> Option<(u16, u16, AtlasSlot, &mut AtlasCommandRecorder)> {
        let (_page_index, x, y, atlas_slot) =
            self.inner.insert_entry(image_cache, key, raster_metrics)?;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "atlas dimensions are configured to fit in u16"
        )]
        let (atlas_w, atlas_h) = {
            let (w, h) = image_cache.atlas_manager().config().atlas_size;
            (w as u16, h as u16)
        };
        let recorder = self
            .inner
            .recorder_for_page(atlas_slot.page_index, atlas_w, atlas_h);
        Some((x, y, atlas_slot, recorder))
    }

    fn push_pending_upload(
        &mut self,
        image_id: ImageId,
        pixmap: Arc<Pixmap>,
        atlas_slot: AtlasSlot,
    ) {
        self.inner.push_pending_upload(image_id, pixmap, atlas_slot);
    }

    fn take_pending_uploads(&mut self) -> Vec<PendingBitmapUpload> {
        self.inner.take_pending_uploads()
    }

    fn take_pending_atlas_commands(&mut self) -> Vec<AtlasCommandRecorder> {
        self.inner
            .take_pending_atlas_commands()
            .into_iter()
            .flatten()
            .collect()
    }

    fn take_pending_clear_rects(&mut self) -> Vec<PendingClearRect> {
        self.inner.take_pending_clear_rects()
    }

    fn maintain(&mut self, image_cache: &mut ImageCache) {
        self.inner.maintain(image_cache);
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn cache_hits(&self) -> u64 {
        self.inner.cache_hits()
    }

    fn cache_misses(&self) -> u64 {
        self.inner.cache_misses()
    }

    fn clear_stats(&mut self) {
        self.inner.clear_stats();
    }
}

/// Convenience alias: all glyph caches needed by the hybrid (GPU) renderer.
pub type GpuGlyphCaches = GlyphCaches<GpuGlyphAtlas>;

impl GpuGlyphCaches {
    /// Creates a new `GpuGlyphCaches` instance with custom eviction settings.
    pub fn with_config(eviction_config: GlyphCacheConfig) -> Self {
        Self {
            outline_cache: OutlineCache::default(),
            hinting_cache: HintCache::default(),
            underline_exclusions: Vec::new(),
            glyph_atlas: GpuGlyphAtlas::with_config(eviction_config),
        }
    }
}

/// Bridges Parley's [`GlyphRenderer`] trait to the shared
/// [`vello_renderer`] cache orchestration for the hybrid backend.
impl GlyphRenderer<GpuGlyphAtlas> for Scene {
    fn fill_glyph(
        &mut self,
        prepared_glyph: PreparedGlyph<'_>,
        glyph_atlas: &mut GpuGlyphAtlas,
        image_cache: &mut ImageCache,
    ) {
        vello_renderer::fill_glyph::<HybridBackend>(self, prepared_glyph, glyph_atlas, image_cache);
    }

    fn stroke_glyph(
        &mut self,
        prepared_glyph: PreparedGlyph<'_>,
        glyph_atlas: &mut GpuGlyphAtlas,
        image_cache: &mut ImageCache,
    ) {
        vello_renderer::stroke_glyph::<HybridBackend>(
            self,
            prepared_glyph,
            glyph_atlas,
            image_cache,
        );
    }

    fn render_cached_glyph(
        &mut self,
        cached_slot: AtlasSlot,
        transform: Affine,
        glyph_type: CachedGlyphType,
    ) {
        match glyph_type {
            CachedGlyphType::Outline => {
                let tint = self.get_context_color();
                vello_renderer::render_outline_glyph_from_atlas::<HybridBackend>(
                    self,
                    cached_slot,
                    transform,
                    tint,
                );
            }
            CachedGlyphType::Bitmap => {
                vello_renderer::render_bitmap_glyph_from_atlas::<HybridBackend>(
                    self,
                    cached_slot,
                    transform,
                );
            }
            CachedGlyphType::Colr(area) => {
                vello_renderer::render_colr_glyph_from_atlas::<HybridBackend>(
                    self,
                    cached_slot,
                    transform,
                    area,
                );
            }
        }
    }

    fn fill_rect(&mut self, rect: Rect) {
        self.fill_rect(&rect);
    }

    fn get_context_color(&self) -> AlphaColor<Srgb> {
        // Non-solid paints (gradients, images) have no single color to
        // extract, so fall back to black — the CSS default for `currentColor`.
        let paint = self.paint().clone();
        match paint {
            PaintType::Solid(s) => s,
            _ => BLACK,
        }
    }
}

/// Zero-sized marker that selects the Vello Hybrid rendering backend
/// in generic [`GlyphAtlasBackend`] code.
pub(crate) struct HybridBackend;

impl GlyphAtlasBackend for HybridBackend {
    type Renderer = Scene;
    type Cache = GpuGlyphAtlas;

    fn render_from_atlas(
        renderer: &mut Scene,
        atlas_slot: AtlasSlot,
        rect_transform: Affine,
        area: Rect,
        quality: ImageQuality,
        paint_transform: Affine,
        tint: Option<Tint>,
    ) {
        // Use the image_cache-assigned ImageId (not page_index). The GPU renderer
        // resolves this through image_cache.get() to obtain atlas layer + offset.
        let image = Image {
            image: ImageSource::opaque_id(atlas_slot.image_id),
            sampler: ImageSampler {
                x_extend: Extend::Pad,
                y_extend: Extend::Pad,
                quality,
                alpha: 1.0,
            },
        };

        let state = renderer.take_current_state();

        renderer.set_tint(tint);
        renderer.set_transform(rect_transform);
        renderer.set_paint(image);
        renderer.set_paint_transform(paint_transform);
        renderer.fill_rect(&area);

        renderer.reset_tint();
        renderer.restore_state(state);
    }

    fn paint_transform(_atlas_slot: &AtlasSlot) -> Affine {
        // The image cache resolves the slot to the allocation origin, but the
        // actual glyph bitmap is inset by GLYPH_PADDING on each side (a guard
        // band required by Extend::Pad sampling). Shift paint to compensate.
        let padding = GLYPH_PADDING as f64;
        Affine::translate((-padding, -padding))
    }

    fn render_outline_to_atlas(
        path: &Arc<BezPath>,
        subpixel_offset: f32,
        recorder: &mut AtlasCommandRecorder,
        dst_x: u16,
        dst_y: u16,
        raster_metrics: RasterMetrics,
    ) {
        let outline_transform =
            Affine::scale_non_uniform(1.0, -1.0).then_translate(kurbo::Vec2::new(
                dst_x as f64 - raster_metrics.bearing_x as f64 + subpixel_offset as f64,
                dst_y as f64 - raster_metrics.bearing_y as f64,
            ));
        recorder.set_transform(outline_transform);
        recorder.set_paint(BLACK);
        recorder.fill_path(path);
    }

    fn render_colr_to_atlas(
        glyph: &ColrGlyph<'_>,
        context_color: AlphaColor<Srgb>,
        recorder: &mut AtlasCommandRecorder,
        dst_x: u16,
        dst_y: u16,
    ) {
        recorder.set_transform(Affine::translate((dst_x as f64, dst_y as f64)));

        let mut colr_painter = ColrPainter::new(glyph, context_color, recorder);
        colr_painter.paint();
    }

    fn queue_bitmap_upload_to_atlas(
        glyph: &GlyphBitmap,
        glyph_atlas: &mut GpuGlyphAtlas,
        atlas_slot: AtlasSlot,
    ) {
        // Queue for GPU upload; the application drains pending uploads and calls
        // Renderer::write_to_atlas before the main render pass.
        glyph_atlas.push_pending_upload(atlas_slot.image_id, Arc::clone(&glyph.pixmap), atlas_slot);
    }

    fn render_outline_directly(renderer: &mut Scene, path: &BezPath, transform: Affine) {
        let state = renderer.take_current_state();
        renderer.set_transform(transform);
        renderer.fill_path(path);
        renderer.restore_state(state);
    }

    fn render_bitmap_directly(renderer: &mut Scene, glyph: GlyphBitmap, transform: Affine) {
        let image = Image {
            image: ImageSource::Pixmap(glyph.pixmap),
            sampler: ImageSampler {
                x_extend: Extend::Pad,
                y_extend: Extend::Pad,
                quality: quality_for_scale(&transform),
                alpha: 1.0,
            },
        };

        let state = renderer.take_current_state();
        renderer.set_transform(transform);
        renderer.set_paint(image);
        renderer.fill_rect(&glyph.area);
        renderer.restore_state(state);
    }

    fn render_colr_directly(
        renderer: &mut Scene,
        glyph: &ColrGlyph<'_>,
        transform: Affine,
        context_color: AlphaColor<Srgb>,
    ) {
        let state = renderer.take_current_state();
        renderer.set_transform(transform);

        let mut colr_painter = ColrPainter::new(glyph, context_color, renderer);
        colr_painter.paint();

        renderer.restore_state(state);
    }
}

/// Maps COLR paint operations to Vello Hybrid draw calls.
///
/// `fill_solid` and `fill_gradient` fill the entire surface because COLR
/// compositing relies on clip layers to restrict the painted region.
impl ColrRenderer for Scene {
    fn push_clip_layer(&mut self, clip: BezPath) {
        self.push_layer(Some(&clip), None, None, None, None);
    }

    fn push_blend_layer(&mut self, blend_mode: BlendMode) {
        self.push_layer(None, Some(blend_mode), None, None, None);
    }

    fn fill_solid(&mut self, color: AlphaColor<Srgb>) {
        self.set_paint(color);
        self.fill_rect(&Rect::new(
            0.0,
            0.0,
            f64::from(self.width()),
            f64::from(self.height()),
        ));
    }

    fn fill_gradient(&mut self, gradient: Gradient) {
        self.set_paint(gradient);
        self.fill_rect(&Rect::new(
            0.0,
            0.0,
            f64::from(self.width()),
            f64::from(self.height()),
        ));
    }

    fn set_paint_transform(&mut self, affine: Affine) {
        Self::set_paint_transform(self, affine);
    }

    fn pop_layer(&mut self) {
        Self::pop_layer(self);
    }
}

/// Allows recorded [`AtlasCommand`](crate::atlas::commands::AtlasCommand)s
/// to be replayed into a hybrid [`Scene`].
impl AtlasReplayTarget for Scene {
    fn set_transform(&mut self, t: Affine) {
        Self::set_transform(self, t);
    }

    fn set_paint_solid(&mut self, color: AlphaColor<Srgb>) {
        self.set_paint(color);
    }

    fn set_paint_gradient(&mut self, gradient: Gradient) {
        self.set_paint(gradient);
    }

    fn set_paint_transform(&mut self, t: Affine) {
        Self::set_paint_transform(self, t);
    }

    fn fill_path(&mut self, path: &BezPath) {
        Self::fill_path(self, path);
    }

    fn fill_rect(&mut self, rect: &Rect) {
        Self::fill_rect(self, rect);
    }

    fn push_clip_layer(&mut self, clip: &BezPath) {
        self.push_layer(Some(clip), None, None, None, None);
    }

    fn push_blend_layer(&mut self, blend_mode: BlendMode) {
        self.push_layer(None, Some(blend_mode), None, None, None);
    }

    fn pop_layer(&mut self) {
        Self::pop_layer(self);
    }
}
