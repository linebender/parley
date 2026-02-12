// Copyright 2025 the Vello Authors and the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Glyph renderer implementation using Vello CPU.

use super::vello_renderer;
use crate::atlas::cache::save_pixmap_to_png;
use crate::atlas::{
    AtlasCommandRecorder, AtlasSlot, GlyphAtlas, GlyphCache, GlyphCacheKey, ImageCache,
    PendingBitmapUpload, RasterMetrics,
};
use crate::renderers::vello_renderer::{AtlasReplayTarget, GlyphAtlasBackend, quality_for_scale};
use crate::{
    Pixmap,
    colr::{ColrPainter, ColrRenderer},
    glyph::{CachedGlyphType, ColrGlyph, GlyphBitmap, GlyphRenderer, PreparedGlyph},
};
use crate::{kurbo, peniko};
use alloc::sync::Arc;
use core::fmt::{Debug, Formatter};
use kurbo::{Affine, BezPath, Rect};
use peniko::Extend;
use peniko::color::{AlphaColor, Srgb};
use peniko::{BlendMode, Gradient};
use vello_common::paint::ImageId;
use vello_cpu::peniko::{ImageQuality, ImageSampler};
use vello_cpu::{Image, ImageSource, PaintType, RenderContext, color::palette::css::BLACK};

/// Default atlas page size (1024x1024 = 4MB for RGBA).
const DEFAULT_PAGE_SIZE: u16 = 256;

/// Glyph bitmap cache for the CPU renderer.
///
/// Extends the shared [`GlyphAtlas`] with per-page `Pixmap` storage
/// so that glyphs can be rasterized directly into CPU-accessible atlas pages.
pub struct CpuGlyphAtlas {
    /// Shared cache data.
    core: GlyphAtlas,
    /// Pixel storage, one per atlas page. Grown on demand when the allocator
    /// creates a new atlas. Wrapped in `Arc` for cheap sharing with renderers.
    pixmaps: Vec<Arc<Pixmap>>,
    /// Width of each atlas page in pixels.
    page_width: u16,
    /// Height of each atlas page in pixels.
    page_height: u16,
}

impl CpuGlyphAtlas {
    /// Creates a new CPU glyph bitmap cache with default settings.
    pub fn new() -> Self {
        Self::with_page_size(DEFAULT_PAGE_SIZE, DEFAULT_PAGE_SIZE)
    }

    /// Creates a new CPU glyph bitmap cache with custom page size.
    pub fn with_page_size(page_width: u16, page_height: u16) -> Self {
        Self {
            core: GlyphAtlas::new(),
            pixmaps: Vec::new(),
            page_width,
            page_height,
        }
    }

    /// Get a shared reference to the pixmap `Arc` for a specific atlas page.
    ///
    /// This is useful for registering atlas pages with a render context
    /// via a cheap `Arc::clone`.
    pub fn page_pixmap(&self, page_index: usize) -> Option<&Arc<Pixmap>> {
        self.pixmaps.get(page_index)
    }

    /// Get a mutable reference to the pixmap for a specific atlas page.
    ///
    /// Returns `None` if the `Arc` has other owners (i.e. was shared via
    /// [`page_pixmap`](Self::page_pixmap) clone).
    pub fn page_pixmap_mut(&mut self, page_index: usize) -> Option<&mut Pixmap> {
        self.pixmaps.get_mut(page_index).and_then(Arc::get_mut)
    }

    /// Get the number of atlas pages currently allocated.
    #[inline]
    pub fn page_count(&self) -> usize {
        self.pixmaps.len()
    }
}

impl Default for CpuGlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for CpuGlyphAtlas {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CpuGlyphAtlas")
            .field("core", &self.core)
            .field("page_count", &self.pixmaps.len())
            .field("page_width", &self.page_width)
            .field("page_height", &self.page_height)
            .finish()
    }
}

impl GlyphCache for CpuGlyphAtlas {
    fn get(&mut self, key: &GlyphCacheKey) -> Option<AtlasSlot> {
        self.core.get(key)
    }

    fn insert(
        &mut self,
        image_cache: &mut ImageCache,
        key: GlyphCacheKey,
        meta: RasterMetrics,
    ) -> Option<(u16, u16, AtlasSlot, &mut AtlasCommandRecorder)> {
        let (atlas_idx, x, y, atlas_slot) = self.core.insert_entry(image_cache, key, meta)?;

        // Create a new pixmap if the allocator opened a new atlas page
        if self.pixmaps.len() <= atlas_idx {
            debug_assert_eq!(self.pixmaps.len(), atlas_idx);
            self.pixmaps
                .push(Arc::new(Pixmap::new(self.page_width, self.page_height)));
        }

        let (atlas_w, atlas_h) = image_cache.atlas_manager().config().atlas_size;
        let recorder =
            self.core
                .recorder_for_page(atlas_slot.page_index, atlas_w as u16, atlas_h as u16);
        Some((x, y, atlas_slot, recorder))
    }

    fn push_pending_upload(&mut self, image_id: ImageId, pixmap: Arc<Pixmap>, atlas_slot: AtlasSlot) {
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
        self.pixmaps.clear();
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

#[cfg(feature = "png")]
impl CpuGlyphAtlas {
    /// Save all atlas pages to PNG files with a custom path prefix.
    ///
    /// Files are saved as `{path_prefix}_atlas_page_{index}.png`.
    pub fn save_atlas_pages_to(&self, path_prefix: &str) {
        for (i, pixmap) in self.pixmaps.iter().enumerate() {
            let path = format!("{path_prefix}_atlas_page_{i}.png");
            let _ = save_pixmap_to_png(pixmap, std::path::Path::new(&path));
        }
    }
}

#[cfg(all(debug_assertions, feature = "png"))]
impl CpuGlyphAtlas {
    /// Save all atlas pages to PNG files for debugging.
    ///
    /// Files are saved to `examples/_output/atlas_page_{index}.png`.
    pub fn save_atlas_pages(&self) {
        for (i, pixmap) in self.pixmaps.iter().enumerate() {
            let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.pop(); // go up from parley_draw to parley
            path.push("examples");
            path.push("_output");
            let _ = std::fs::create_dir_all(&path);
            path.push(format!("atlas_page_{i}.png"));
            let _ = save_pixmap_to_png(pixmap, &path);
        }
    }
}

#[cfg(feature = "std")]
impl CpuGlyphAtlas {
    /// Get detailed statistics about cached glyphs.
    pub fn stats(&self) -> crate::atlas::GlyphCacheStats {
        self.core.stats(self.pixmaps.len())
    }

    /// Print cache statistics to stdout.
    pub fn print_stats(&self) {
        self.core.print_stats(self.pixmaps.len());
    }

    /// Returns all cached glyph keys (for debugging).
    pub fn all_keys(&self) -> Vec<&GlyphCacheKey> {
        self.core.all_keys()
    }

    /// Print all cached keys grouped by glyph ID.
    pub fn print_keys_grouped(&self) {
        self.core.print_keys_grouped();
    }
}

/// Convenient type alias for CPU rendering glyph caches.
pub type CpuGlyphCaches = crate::glyph::GlyphCaches<CpuGlyphAtlas>;

impl CpuGlyphCaches {
    /// Creates a new `CpuGlyphCaches` instance with custom bitmap cache page size.
    pub fn with_page_size(page_width: u16, page_height: u16) -> Self {
        Self {
            outline_cache: crate::glyph::OutlineCache::default(),
            hinting_cache: crate::glyph::HintCache::default(),
            bitmap_cache: CpuGlyphAtlas::with_page_size(page_width, page_height),
        }
    }

    /// Save all atlas pages to PNG files for debugging.
    ///
    /// Files are saved to `examples/_output/atlas_page_{index}.png`.
    /// Only available in debug builds with the `png` feature.
    #[cfg(all(debug_assertions, feature = "png"))]
    pub fn save_atlas_pages(&self) {
        self.bitmap_cache.save_atlas_pages();
    }

    /// Save all atlas pages to PNG files with a custom path prefix.
    ///
    /// Files are saved as `{path_prefix}_atlas_page_{index}.png`.
    /// Only available with the `png` feature.
    #[cfg(feature = "png")]
    pub fn save_atlas_pages_to(&self, path_prefix: &str) {
        self.bitmap_cache.save_atlas_pages_to(path_prefix);
    }
}

impl GlyphRenderer<CpuGlyphAtlas> for RenderContext {
    fn fill_glyph(
        &mut self,
        prepared_glyph: PreparedGlyph<'_>,
        bitmap_cache: &mut CpuGlyphAtlas,
        image_cache: &mut ImageCache,
    ) {
        vello_renderer::fill_glyph::<CpuBackend>(
            self,
            prepared_glyph,
            bitmap_cache,
            image_cache,
        );
    }

    fn stroke_glyph(
        &mut self,
        prepared_glyph: PreparedGlyph<'_>,
        bitmap_cache: &mut CpuGlyphAtlas,
        image_cache: &mut ImageCache,
    ) {
        vello_renderer::stroke_glyph::<CpuBackend>(
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
                vello_renderer::render_outline_glyph_from_atlas::<CpuBackend>(
                    self,
                    cached_slot,
                    transform,
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

    fn get_context_color(&self) -> AlphaColor<Srgb> {
        let paint = self.paint().clone();
        match paint {
            PaintType::Solid(s) => s,
            _ => BLACK,
        }
    }
}

/// Marker type for the Vello CPU rendering backend.
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
    ) {
        let image = Image {
            image: ImageSource::OpaqueId(ImageId::new(atlas_slot.page_index)),
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

    fn outline_paint_transform(atlas_slot: &AtlasSlot) -> Affine {
        Affine::translate((-(atlas_slot.x as f64), -(atlas_slot.y as f64)))
    }

    fn bitmap_paint_transform(atlas_slot: &AtlasSlot) -> Affine {
        Affine::translate((-(atlas_slot.x as f64), -(atlas_slot.y as f64)))
    }

    fn colr_paint_transform(atlas_slot: &AtlasSlot) -> Affine {
        Affine::translate((-(atlas_slot.x as f64), -(atlas_slot.y as f64)))
    }

    fn render_outline_to_atlas(
        path: &BezPath,
        subpixel_bucket: f32,
        recorder: &mut AtlasCommandRecorder,
        dst_x: u16,
        dst_y: u16,
        meta: RasterMetrics,
    ) {
        let outline_transform = Affine::scale_non_uniform(1.0, -1.0).then_translate(
            kurbo::Vec2::new(
                dst_x as f64 - meta.bearing_x as f64 + subpixel_bucket as f64,
                dst_y as f64 - meta.bearing_y as f64,
            ),
        );
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
        bitmap_cache: &mut CpuGlyphAtlas,
        atlas_slot: AtlasSlot,
    ) {
        // Queue bitmap for deferred copy to atlas pixmap.
        // The actual copy happens before render_to_pixmap() is called.
        bitmap_cache.push_pending_upload(atlas_slot.image_id, Arc::clone(&glyph.pixmap), atlas_slot);
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

impl ColrRenderer for RenderContext {
    fn push_clip_layer(&mut self, clip: &BezPath) {
        Self::push_clip_layer(self, clip);
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
mod debug {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use crate::atlas::RasterMetrics;
    use crate::kurbo::{Affine, Rect};
    use crate::peniko;
    use vello_cpu::RenderContext;

    static COLOR_INDEX: AtomicUsize = AtomicUsize::new(0);

    /// Rotating colors for visualizing glyph bounds during rasterization.
    /// Each glyph gets the next color in sequence, making it easy to
    /// distinguish adjacent glyphs.
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

    /// Fill the glyph bounds with a rotating debug color.
    /// Call this before rendering the actual glyph content.
    pub fn fill_glyph_bounds(renderer: &mut RenderContext, meta: &RasterMetrics) {
        let idx = COLOR_INDEX.fetch_add(1, Ordering::Relaxed) % COLORS.len();
        renderer.set_transform(Affine::IDENTITY);
        renderer.set_paint(COLORS[idx]);
        renderer.fill_rect(&Rect::new(0.0, 0.0, meta.width as f64, meta.height as f64));
    }
}
