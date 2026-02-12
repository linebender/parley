// Copyright 2025 the Vello Authors and the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Glyph renderer implementation using Vello Hybrid.

use super::vello_renderer;
use crate::atlas::{
    AtlasCommandRecorder, AtlasSlot, GLYPH_PADDING, GlyphAtlas, GlyphCache, GlyphCacheKey,
    ImageCache, PendingBitmapUpload, RasterMetrics,
};
use crate::renderers::vello_renderer::{AtlasReplayTarget, GlyphAtlasBackend, quality_for_scale};
use crate::{
    Pixmap,
    colr::{ColrPainter, ColrRenderer},
    glyph::{CachedGlyphType, ColrGlyph, GlyphBitmap, GlyphRenderer, PreparedGlyph},
};
use crate::{kurbo, peniko};
use alloc::sync::Arc;
use alloc::vec::Vec;
use kurbo::{Affine, BezPath, Rect};
use peniko::color::palette::css::BLACK;
use peniko::color::{AlphaColor, Srgb};
use peniko::{BlendMode, Extend, Gradient, ImageQuality, ImageSampler};
use vello_common::paint::{Image, ImageId, ImageSource, PaintType};
use vello_hybrid::Scene;

/// Glyph bitmap cache for the hybrid (GPU) renderer.
///
/// Uses the shared [`GlyphAtlas`] for cache entries and pending
/// uploads, but does not allocate any local `Pixmap` storage — the GPU
/// renderer manages atlas textures itself.
#[derive(Debug, Default)]
pub struct GpuGlyphAtlas {
    /// Shared cache data.
    core: GlyphAtlas,
}

impl GpuGlyphAtlas {
    /// Creates a new hybrid glyph bitmap cache.
    pub fn new() -> Self {
        Self {
            core: GlyphAtlas::new(),
        }
    }
}

impl GlyphCache for GpuGlyphAtlas {
    fn get(&mut self, key: &GlyphCacheKey) -> Option<AtlasSlot> {
        self.core.get(key)
    }

    fn insert(
        &mut self,
        image_cache: &mut ImageCache,
        key: GlyphCacheKey,
        meta: RasterMetrics,
    ) -> Option<(u16, u16, AtlasSlot, &mut AtlasCommandRecorder)> {
        let (_atlas_idx, x, y, atlas_slot) = self.core.insert_entry(image_cache, key, meta)?;
        let (atlas_w, atlas_h) = image_cache.atlas_manager().config().atlas_size;
        let recorder =
            self.core
                .recorder_for_page(atlas_slot.page_index, atlas_w as u16, atlas_h as u16);
        Some((x, y, atlas_slot, recorder))
    }

    fn push_pending_upload(
        &mut self,
        image_id: ImageId,
        pixmap: Arc<Pixmap>,
        atlas_slot: AtlasSlot,
    ) {
        self.core.push_pending_upload(image_id, pixmap, atlas_slot);
    }

    fn take_pending_uploads(&mut self) -> Vec<PendingBitmapUpload> {
        self.core.take_pending_uploads()
    }

    fn take_pending_atlas_commands(&mut self) -> Vec<AtlasCommandRecorder> {
        self.core.take_pending_atlas_commands().into_iter().flatten().collect()
    }

    fn tick(&mut self) {
        self.core.tick();
    }

    fn maintain(&mut self, image_cache: &mut ImageCache) {
        self.core.maintain(image_cache);
    }

    fn clear(&mut self) {
        self.core.clear();
    }

    fn len(&self) -> usize {
        self.core.len()
    }

    fn is_empty(&self) -> bool {
        self.core.is_empty()
    }

    fn cache_hits(&self) -> u64 {
        self.core.cache_hits()
    }

    fn cache_misses(&self) -> u64 {
        self.core.cache_misses()
    }

    fn clear_stats(&mut self) {
        self.core.clear_stats();
    }
}

/// Convenient type alias for hybrid (GPU) rendering glyph caches.
pub type GpuGlyphCaches = crate::glyph::GlyphCaches<GpuGlyphAtlas>;

impl GlyphRenderer<GpuGlyphAtlas> for Scene {
    fn fill_glyph(
        &mut self,
        prepared_glyph: PreparedGlyph<'_>,
        bitmap_cache: &mut GpuGlyphAtlas,
        image_cache: &mut ImageCache,
    ) {
        vello_renderer::fill_glyph::<HybridBackend>(
            self,
            prepared_glyph,
            bitmap_cache,
            image_cache,
        );
    }

    fn stroke_glyph(
        &mut self,
        prepared_glyph: PreparedGlyph<'_>,
        bitmap_cache: &mut GpuGlyphAtlas,
        image_cache: &mut ImageCache,
    ) {
        vello_renderer::stroke_glyph::<HybridBackend>(
            self,
            prepared_glyph,
            bitmap_cache,
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
                vello_renderer::render_outline_glyph_from_atlas::<HybridBackend>(
                    self,
                    cached_slot,
                    transform,
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

    fn get_context_color(&self) -> AlphaColor<Srgb> {
        // The context color defaults to black since we can't access the
        // current paint from vello_hybrid's Scene API.
        let paint = self.paint().clone();
        match paint {
            PaintType::Solid(s) => s,
            _ => BLACK,
        }
    }
}

/// Marker type for the Vello Hybrid rendering backend.
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
    ) {
        // Use the actual allocated ImageId (not page_index). The GPU renderer resolves
        // this through image_cache.get() which knows the atlas offset.
        let image = Image {
            image: ImageSource::OpaqueId(atlas_slot.image_id),
            sampler: ImageSampler {
                x_extend: Extend::Pad,
                y_extend: Extend::Pad,
                quality,
                alpha: 1.0,
            },
        };

        let state = renderer.take_current_state();
        renderer.set_transform(rect_transform);
        renderer.set_paint(image);
        renderer.set_paint_transform(paint_transform);
        renderer.fill_rect(&area);
        renderer.restore_state(state);
    }

    fn outline_paint_transform(_atlas_slot: &AtlasSlot) -> Affine {
        let padding = GLYPH_PADDING as f64;
        Affine::translate((-padding, -padding))
    }

    fn bitmap_paint_transform(_atlas_slot: &AtlasSlot) -> Affine {
        Affine::IDENTITY
    }

    fn colr_paint_transform(_atlas_slot: &AtlasSlot) -> Affine {
        let padding = GLYPH_PADDING as f64;
        Affine::translate((-padding, -padding))
    }

    fn render_outline_to_atlas(
        path: &BezPath,
        subpixel_bucket: f32,
        recorder: &mut AtlasCommandRecorder,
        dst_x: u16,
        dst_y: u16,
        meta: RasterMetrics,
    ) {
        let outline_transform =
            Affine::scale_non_uniform(1.0, -1.0).then_translate(kurbo::Vec2::new(
                dst_x as f64 - meta.bearing_x as f64 + subpixel_bucket as f64,
                dst_y as f64 - meta.bearing_y as f64,
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
        bitmap_cache: &mut GpuGlyphAtlas,
        atlas_slot: AtlasSlot,
    ) {
        // Queue the bitmap for GPU upload. The application will drain this
        // and call Renderer::write_to_atlas before the main render pass.
        bitmap_cache.push_pending_upload(
            atlas_slot.image_id,
            Arc::clone(&glyph.pixmap),
            atlas_slot,
        );
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

impl ColrRenderer for Scene {
    fn push_clip_layer(&mut self, clip: &BezPath) {
        self.push_layer(Some(clip), None, None, None, None);
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
        Scene::set_paint_transform(self, affine);
    }

    fn pop_layer(&mut self) {
        Scene::pop_layer(self);
    }
}

impl AtlasReplayTarget for Scene {
    fn set_transform(&mut self, t: Affine) {
        Scene::set_transform(self, t);
    }

    fn set_paint_solid(&mut self, color: AlphaColor<Srgb>) {
        self.set_paint(color);
    }

    fn set_paint_gradient(&mut self, gradient: Gradient) {
        self.set_paint(gradient);
    }

    fn set_paint_transform(&mut self, t: Affine) {
        Scene::set_paint_transform(self, t);
    }

    fn fill_path(&mut self, path: &BezPath) {
        Scene::fill_path(self, path);
    }

    fn fill_rect(&mut self, rect: &Rect) {
        Scene::fill_rect(self, rect);
    }

    fn push_clip_layer(&mut self, clip: &BezPath) {
        self.push_layer(Some(clip), None, None, None, None);
    }

    fn push_blend_layer(&mut self, blend_mode: BlendMode) {
        self.push_layer(None, Some(blend_mode), None, None, None);
    }

    fn pop_layer(&mut self) {
        Scene::pop_layer(self);
    }
}
