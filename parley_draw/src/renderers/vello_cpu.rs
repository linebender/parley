// Copyright 2025 the Vello Authors and the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Vello CPU glyph rendering backend.
//!
//! Provides [`CpuGlyphAtlas`] (atlas backed by per-page [`Pixmap`]s) and the
//! `CpuBackend` implementation of `GlyphAtlasBackend` that rasterises
//! glyphs into CPU-accessible pixel buffers.
//!
//! The key difference from the hybrid backend is that atlas pages are owned as
//! [`Arc<Pixmap>`]s here, so the CPU renderer can read pixels directly without
//! any GPU upload step.

use super::vello_renderer;
#[cfg(debug_assertions)]
use crate::atlas::GlyphCacheStats;
#[cfg(all(debug_assertions, feature = "png"))]
use crate::atlas::cache::save_pixmap_to_png;
use crate::atlas::{
    AtlasCommandRecorder, AtlasSlot, GlyphAtlas, GlyphCache, GlyphCacheConfig, GlyphCacheKey,
    ImageCache, PendingBitmapUpload, PendingClearRect, RasterMetrics,
};
use crate::glyph::{HintCache, OutlineCache};
use crate::renderers::vello_renderer::{AtlasReplayTarget, GlyphAtlasBackend, quality_for_scale};
use crate::{GlyphCaches, kurbo, peniko};
use crate::{
    Pixmap,
    colr::{ColrPainter, ColrRenderer},
    glyph::{CachedGlyphType, ColrGlyph, GlyphBitmap, GlyphRenderer, PreparedGlyph},
};
#[cfg(all(debug_assertions, feature = "png"))]
use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use kurbo::{Affine, BezPath, Rect};
use peniko::Extend;
use peniko::color::{AlphaColor, Srgb};
use peniko::{BlendMode, Gradient};
use vello_common::paint::{ImageId, Tint};
use vello_cpu::peniko::{ImageQuality, ImageSampler};
use vello_cpu::{Image, ImageSource, PaintType, RenderContext, color::palette::css::BLACK};

/// CPU-side glyph atlas backed by per-page [`Pixmap`]s.
///
/// Wraps the shared [`GlyphAtlas`] allocator and adds owned pixel storage
/// so that glyphs can be rasterized directly into CPU-accessible memory.
pub struct CpuGlyphAtlas {
    /// Shared cache data.
    inner: GlyphAtlas,
    /// One `Pixmap` per atlas page, grown on demand. Wrapped in `Arc` so
    /// callers can cheaply share a page with a render context.
    pixmaps: Vec<Arc<Pixmap>>,
    /// Width of each atlas page in pixels.
    page_width: u16,
    /// Height of each atlas page in pixels.
    page_height: u16,
}

impl CpuGlyphAtlas {
    /// Creates a new atlas with the given page dimensions and default eviction settings.
    pub fn new(page_width: u16, page_height: u16) -> Self {
        Self::with_config(page_width, page_height, GlyphCacheConfig::default())
    }

    /// Creates a new atlas with custom page dimensions and eviction settings.
    pub fn with_config(
        page_width: u16,
        page_height: u16,
        eviction_config: GlyphCacheConfig,
    ) -> Self {
        Self {
            inner: GlyphAtlas::with_config(eviction_config),
            pixmaps: Vec::new(),
            page_width,
            page_height,
        }
    }

    /// Returns a reference to the `Arc<Pixmap>` for `page_index`, allowing
    /// cheap `Arc::clone` when registering the page with a render context.
    pub fn page_pixmap(&self, page_index: usize) -> Option<&Arc<Pixmap>> {
        self.pixmaps.get(page_index)
    }

    /// Returns a mutable reference to the pixmap for `page_index`.
    pub fn page_pixmap_mut(&mut self, page_index: usize) -> Option<&mut Pixmap> {
        self.pixmaps.get_mut(page_index).and_then(Arc::get_mut)
    }

    /// Returns the number of atlas pages currently allocated.
    #[inline]
    pub fn page_count(&self) -> usize {
        self.pixmaps.len()
    }
}

impl Default for CpuGlyphAtlas {
    fn default() -> Self {
        Self::new(256, 256)
    }
}

impl Debug for CpuGlyphAtlas {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CpuGlyphAtlas")
            .field("inner", &self.inner)
            .field("page_count", &self.pixmaps.len())
            .field("page_width", &self.page_width)
            .field("page_height", &self.page_height)
            .finish()
    }
}

/// Delegates to the inner [`GlyphAtlas`], additionally growing the `pixmaps`
/// vector when `insert` opens a new atlas page.
impl GlyphCache for CpuGlyphAtlas {
    fn get(&mut self, key: &GlyphCacheKey) -> Option<AtlasSlot> {
        self.inner.get(key)
    }

    fn insert(
        &mut self,
        image_cache: &mut ImageCache,
        key: GlyphCacheKey,
        raster_metrics: RasterMetrics,
    ) -> Option<(u16, u16, AtlasSlot, &mut AtlasCommandRecorder)> {
        let (page_index, x, y, atlas_slot) =
            self.inner.insert_entry(image_cache, key, raster_metrics)?;

        // Create a new pixmap if the allocator opened a new atlas page
        if self.pixmaps.len() <= page_index {
            debug_assert_eq!(
                self.pixmaps.len(),
                page_index,
                "atlas page indices must be contiguous"
            );
            self.pixmaps
                .push(Arc::new(Pixmap::new(self.page_width, self.page_height)));
        }

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
        self.pixmaps.clear();
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

#[cfg(all(debug_assertions, feature = "png"))]
impl CpuGlyphAtlas {
    /// Save every atlas page as `{path_prefix}_atlas_page_{index}.png`.
    pub fn save_atlas_pages_to(&self, path_prefix: &str) {
        for (i, pixmap) in self.pixmaps.iter().enumerate() {
            let path = format!("{path_prefix}_atlas_page_{i}.png");
            let _ = save_pixmap_to_png(pixmap, std::path::Path::new(&path));
        }
    }
}

#[cfg(all(debug_assertions, feature = "png"))]
impl CpuGlyphAtlas {
    /// Save every atlas page under `examples/_output/vello_cpu_atlas_page_{index}.png`.
    pub fn save_atlas_pages(&self) {
        for (i, pixmap) in self.pixmaps.iter().enumerate() {
            let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.pop(); // up from parley_draw to workspace root
            path.push("examples");
            path.push("_output");
            let _ = std::fs::create_dir_all(&path);
            path.push(format!("vello_cpu_atlas_page_{i}.png"));
            let _ = save_pixmap_to_png(pixmap, &path);
        }
    }
}

#[cfg(debug_assertions)]
impl CpuGlyphAtlas {
    /// Get detailed statistics about cached glyphs.
    pub fn stats(&self) -> GlyphCacheStats {
        self.inner.stats(self.pixmaps.len())
    }

    /// Log detailed atlas statistics at info level.
    pub fn log_atlas_stats(&self) {
        self.inner.log_atlas_stats(self.pixmaps.len());
    }

    /// Returns all cached glyph keys (for debugging).
    pub fn all_keys(&self) -> Vec<&GlyphCacheKey> {
        self.inner.all_keys()
    }

    /// Log all cached keys grouped by glyph ID at info level.
    pub fn log_keys_grouped(&self) {
        self.inner.log_keys_grouped();
    }
}

/// Convenience alias: all glyph caches needed by the CPU renderer.
pub type CpuGlyphCaches = GlyphCaches<CpuGlyphAtlas>;

impl CpuGlyphCaches {
    /// Creates a new `CpuGlyphCaches` instance with the given atlas page size
    /// and default eviction settings.
    pub fn new(page_width: u16, page_height: u16) -> Self {
        Self::with_config(page_width, page_height, GlyphCacheConfig::default())
    }

    /// Creates a new `CpuGlyphCaches` instance with custom page size and eviction settings.
    pub fn with_config(
        page_width: u16,
        page_height: u16,
        eviction_config: GlyphCacheConfig,
    ) -> Self {
        Self {
            outline_cache: OutlineCache::default(),
            hinting_cache: HintCache::default(),
            underline_exclusions: Vec::new(),
            glyph_atlas: CpuGlyphAtlas::with_config(page_width, page_height, eviction_config),
        }
    }

    /// Save all atlas pages to PNG files for debugging.
    ///
    /// Files are saved to `examples/_output/vello_cpu_atlas_page_{index}.png`.
    /// Only available in debug builds with the `png` feature.
    #[cfg(all(debug_assertions, feature = "png"))]
    pub fn save_atlas_pages(&self) {
        self.glyph_atlas.save_atlas_pages();
    }

    /// Save all atlas pages to PNG files with a custom path prefix.
    ///
    /// Files are saved as `{path_prefix}_atlas_page_{index}.png`.
    /// Only available with the `png` feature in debug builds.
    #[cfg(all(debug_assertions, feature = "png"))]
    pub fn save_atlas_pages_to(&self, path_prefix: &str) {
        self.glyph_atlas.save_atlas_pages_to(path_prefix);
    }
}

/// Bridges Parley's [`GlyphRenderer`] trait to the shared
/// [`vello_renderer`] cache orchestration for the CPU backend.
impl GlyphRenderer<CpuGlyphAtlas> for RenderContext {
    fn fill_glyph(
        &mut self,
        prepared_glyph: PreparedGlyph<'_>,
        glyph_atlas: &mut CpuGlyphAtlas,
        image_cache: &mut ImageCache,
    ) {
        vello_renderer::fill_glyph::<CpuBackend>(self, prepared_glyph, glyph_atlas, image_cache);
    }

    fn stroke_glyph(
        &mut self,
        prepared_glyph: PreparedGlyph<'_>,
        glyph_atlas: &mut CpuGlyphAtlas,
        image_cache: &mut ImageCache,
    ) {
        vello_renderer::stroke_glyph::<CpuBackend>(self, prepared_glyph, glyph_atlas, image_cache);
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
                vello_renderer::render_outline_glyph_from_atlas::<CpuBackend>(
                    self,
                    cached_slot,
                    transform,
                    tint,
                );
            }
            CachedGlyphType::Bitmap => {
                vello_renderer::render_bitmap_glyph_from_atlas::<CpuBackend>(
                    self,
                    cached_slot,
                    transform,
                );
            }
            CachedGlyphType::Colr(area) => {
                vello_renderer::render_colr_glyph_from_atlas::<CpuBackend>(
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

/// Zero-sized marker that selects the Vello CPU rendering backend
/// in generic [`GlyphAtlasBackend`] code.
pub(crate) struct CpuBackend;

impl GlyphAtlasBackend for CpuBackend {
    type Renderer = RenderContext;
    type Cache = CpuGlyphAtlas;

    fn render_from_atlas(
        renderer: &mut RenderContext,
        atlas_slot: AtlasSlot,
        rect_transform: Affine,
        area: Rect,
        quality: ImageQuality,
        paint_transform: Affine,
        tint: Option<Tint>,
    ) {
        // CPU backend uses page_index as the opaque ImageId (one pixmap per page),
        // resolved later via register_image(). The hybrid backend uses the
        // image_cache-assigned ImageId instead.
        // TODO: use the actual allocated ImageId similar to the hybrid?
        let image = Image {
            image: ImageSource::opaque_id(ImageId::new(atlas_slot.page_index)),
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

    fn paint_transform(atlas_slot: &AtlasSlot) -> Affine {
        // The CPU backend uses full-page pixmaps, so the paint origin must be
        // shifted to the glyph's slot within the page. Contrast with the hybrid
        // backend, which gets per-allocation origins from the image cache.
        Affine::translate((-(atlas_slot.x as f64), -(atlas_slot.y as f64)))
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
        glyph_atlas: &mut CpuGlyphAtlas,
        atlas_slot: AtlasSlot,
    ) {
        // Deferred: the copy to atlas pixmap happens before render_to_pixmap().
        glyph_atlas.push_pending_upload(atlas_slot.image_id, Arc::clone(&glyph.pixmap), atlas_slot);
    }

    fn render_outline_directly(renderer: &mut RenderContext, path: &BezPath, transform: Affine) {
        let state = renderer.take_current_state();
        renderer.set_transform(transform);
        renderer.fill_path(path);
        renderer.restore_state(state);
    }

    fn render_bitmap_directly(renderer: &mut RenderContext, glyph: GlyphBitmap, transform: Affine) {
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
        renderer.set_paint(image);
        renderer.set_transform(transform);
        renderer.fill_rect(&glyph.area);
        renderer.restore_state(state);
    }

    fn render_colr_directly(
        renderer: &mut RenderContext,
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

/// Maps COLR paint operations to Vello CPU draw calls.
impl ColrRenderer for RenderContext {
    fn push_clip_layer(&mut self, clip: BezPath) {
        Self::push_clip_layer(self, &clip);
    }

    fn push_blend_layer(&mut self, blend_mode: BlendMode) {
        Self::push_blend_layer(self, blend_mode);
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
/// to be replayed into a CPU [`RenderContext`].
impl AtlasReplayTarget for RenderContext {
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
        Self::push_clip_layer(self, clip);
    }

    fn push_blend_layer(&mut self, blend_mode: BlendMode) {
        Self::push_blend_layer(self, blend_mode);
    }

    fn pop_layer(&mut self) {
        Self::pop_layer(self);
    }
}

/// Debug utilities for visualizing glyph bounds during rasterization.
#[cfg(feature = "debug_glyph_bounds")]
#[allow(
    dead_code,
    unreachable_pub,
    clippy::trivially_copy_pass_by_ref,
    reason = "debug-only utilities called manually during development"
)]
mod debug {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use crate::atlas::RasterMetrics;
    use crate::kurbo::{Affine, Rect};
    use crate::peniko;
    use vello_cpu::RenderContext;

    static COLOR_INDEX: AtomicUsize = AtomicUsize::new(0);

    /// Twelve semi-transparent rotating colours for distinguishing adjacent
    /// glyph bounds. Each call to [`fill_glyph_bounds`] advances the index.
    const COLORS: [peniko::Color; 12] = [
        peniko::Color::new([1.0, 0.0, 0.0, 0.5]), // Red
        peniko::Color::new([0.0, 1.0, 0.0, 0.5]), // Green
        peniko::Color::new([0.0, 0.0, 1.0, 0.5]), // Blue
        peniko::Color::new([1.0, 1.0, 0.0, 0.5]), // Yellow
        peniko::Color::new([1.0, 0.0, 1.0, 0.5]), // Magenta
        peniko::Color::new([0.0, 1.0, 1.0, 0.5]), // Cyan
        peniko::Color::new([1.0, 0.5, 0.0, 0.5]), // Orange
        peniko::Color::new([0.5, 0.0, 1.0, 0.5]), // Purple
        peniko::Color::new([0.0, 1.0, 0.5, 0.5]), // Mint
        peniko::Color::new([1.0, 0.5, 0.5, 0.5]), // Pink
        peniko::Color::new([0.5, 1.0, 0.5, 0.5]), // Light green
        peniko::Color::new([0.5, 0.5, 1.0, 0.5]), // Light blue
    ];

    /// Fill the glyph's pixel bounds with the next rotating debug colour.
    /// Call before rendering the actual glyph content to visualise the extent.
    pub fn fill_glyph_bounds(renderer: &mut RenderContext, raster_metrics: &RasterMetrics) {
        let idx = COLOR_INDEX.fetch_add(1, Ordering::Relaxed) % COLORS.len();
        renderer.set_transform(Affine::IDENTITY);
        renderer.set_paint(COLORS[idx]);
        renderer.fill_rect(&Rect::new(
            0.0,
            0.0,
            raster_metrics.width as f64,
            raster_metrics.height as f64,
        ));
    }
}
