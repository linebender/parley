// Copyright 2025 the Vello Authors and the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared glyph rendering logic for all backends.
//!
//! This module contains the backend-agnostic parts of glyph rendering:
//! utility functions, the [`GlyphAtlasBackend`] trait, the
//! [`AtlasReplayTarget`] trait, and generic cache orchestration functions
//! ([`fill_glyph`], [`stroke_glyph`]).

use crate::atlas::commands::{AtlasCommand, AtlasCommandRecorder, AtlasPaint};
use crate::atlas::key::subpixel_offset;
use crate::atlas::{AtlasSlot, GlyphCache, GlyphCacheKey, ImageCache, RasterMetrics};
use crate::glyph::{ColrGlyph, GlyphBitmap, GlyphRenderer, GlyphType, PreparedGlyph};
use crate::{kurbo, peniko};
use kurbo::{Affine, BezPath, Rect, Shape};
use peniko::color::{AlphaColor, Srgb};
use peniko::{BlendMode, Gradient, ImageQuality};

#[cfg(not(feature = "std"))]
use peniko::kurbo::common::FloatFuncs as _;

// ---------------------------------------------------------------------------
// AtlasReplayTarget — the replay interface for atlas commands
// ---------------------------------------------------------------------------

/// Trait for types that can execute atlas draw commands.
///
/// Both the actual renderers (`RenderContext`, `Scene`) implement this trait
/// so that recorded [`AtlasCommand`]s can be replayed into them at render time.
pub trait AtlasReplayTarget {
    /// Set the current transform.
    fn set_transform(&mut self, t: Affine);
    /// Set the current paint to a solid colour.
    fn set_paint_solid(&mut self, color: AlphaColor<Srgb>);
    /// Set the current paint to a gradient.
    fn set_paint_gradient(&mut self, gradient: Gradient);
    /// Set the paint transform.
    fn set_paint_transform(&mut self, t: Affine);
    /// Fill a path with the current paint and transform.
    fn fill_path(&mut self, path: &BezPath);
    /// Fill a rectangle with the current paint and transform.
    fn fill_rect(&mut self, rect: &Rect);
    /// Push a clip layer defined by a path.
    fn push_clip_layer(&mut self, clip: &BezPath);
    /// Push a blend/compositing layer.
    fn push_blend_layer(&mut self, blend_mode: BlendMode);
    /// Pop the most recent clip or blend layer.
    fn pop_layer(&mut self);
}

/// Replay recorded atlas commands into a target that implements [`AtlasReplayTarget`].
pub fn replay_atlas_commands(commands: &[AtlasCommand], target: &mut impl AtlasReplayTarget) {
    for cmd in commands {
        match cmd {
            AtlasCommand::SetTransform(t) => target.set_transform(*t),
            AtlasCommand::SetPaint(AtlasPaint::Solid(c)) => target.set_paint_solid(*c),
            AtlasCommand::SetPaint(AtlasPaint::Gradient(g)) => target.set_paint_gradient(g.clone()),
            AtlasCommand::SetPaintTransform(t) => target.set_paint_transform(*t),
            AtlasCommand::FillPath(p) => target.fill_path(p),
            AtlasCommand::FillRect(r) => target.fill_rect(r),
            AtlasCommand::PushClipLayer(c) => target.push_clip_layer(c),
            AtlasCommand::PushBlendLayer(m) => target.push_blend_layer(*m),
            AtlasCommand::PopLayer => target.pop_layer(),
        }
    }
}

// ---------------------------------------------------------------------------
// CacheOutcome
// ---------------------------------------------------------------------------

/// Result of attempting to render a glyph through the bitmap cache.
enum CacheOutcome {
    /// Glyph was rasterized, inserted into cache, and rendered from atlas.
    Inserted,
    /// Glyph has non-axis-aligned transform (rotated/skewed).
    NotAxisAligned,
    /// Atlas is full or allocation failed.
    AtlasFull,
}

// ---------------------------------------------------------------------------
// GlyphAtlasBackend
// ---------------------------------------------------------------------------

/// Abstracts the differences between CPU and Hybrid rendering backends.
///
/// Backend-specific operations — atlas image construction, paint transforms,
/// and outline/COLR rendering into the [`AtlasCommandRecorder`] — are defined
/// as trait methods.  The shared cache orchestration logic lives in the
/// generic free functions [`fill_glyph`] and [`stroke_glyph`].
pub(crate) trait GlyphAtlasBackend {
    /// The renderer type for this backend (e.g. `RenderContext` or `Scene`).
    type Renderer;

    /// The glyph bitmap cache type for this backend.
    type Cache: GlyphCache;

    // ---- Atlas rendering -------------------------------------------------

    /// Render a cached glyph from the atlas.
    ///
    /// Constructs a backend-specific `Image` from the atlas slot and
    /// renders it into the scene.
    fn render_from_atlas(
        renderer: &mut Self::Renderer,
        atlas_slot: AtlasSlot,
        rect_transform: Affine,
        area: Rect,
        quality: ImageQuality,
        paint_transform: Affine,
    );

    /// Compute the paint transform for outline glyphs rendered from the atlas.
    fn outline_paint_transform(atlas_slot: &AtlasSlot) -> Affine;

    /// Compute the paint transform for bitmap glyphs rendered from the atlas.
    fn bitmap_paint_transform(atlas_slot: &AtlasSlot) -> Affine;

    /// Compute the paint transform for COLR glyphs rendered from the atlas.
    fn colr_paint_transform(atlas_slot: &AtlasSlot) -> Affine;

    // ---- Atlas population ------------------------------------------------

    /// Record outline glyph draw commands into the atlas command recorder.
    fn render_outline_to_atlas(
        path: &BezPath,
        subpixel_bucket: f32,
        recorder: &mut AtlasCommandRecorder,
        dst_x: u16,
        dst_y: u16,
        meta: RasterMetrics,
    );

    /// Record COLR glyph draw commands into the atlas command recorder.
    fn render_colr_to_atlas(
        glyph: &ColrGlyph<'_>,
        context_color: AlphaColor<Srgb>,
        recorder: &mut AtlasCommandRecorder,
        dst_x: u16,
        dst_y: u16,
    );

    /// Handle bitmap atlas insertion bookkeeping.
    ///
    /// Both backends queue the bitmap for later processing:
    /// - CPU backend: queues for deferred copy to atlas pixmap
    /// - Hybrid backend: queues for GPU upload
    fn queue_bitmap_upload_to_atlas(
        glyph: &GlyphBitmap,
        bitmap_cache: &mut Self::Cache,
        atlas_slot: AtlasSlot,
    );

    // ---- Direct rendering (uncached) -------------------------------------

    /// Render an outline glyph directly (not using cache).
    fn render_outline_directly(renderer: &mut Self::Renderer, path: &BezPath, transform: Affine);

    /// Render a bitmap glyph directly (not using cache).
    fn render_bitmap_directly(renderer: &mut Self::Renderer, glyph: GlyphBitmap, transform: Affine);

    /// Render a COLR glyph directly (not using cache).
    fn render_colr_directly(
        renderer: &mut Self::Renderer,
        glyph: &ColrGlyph<'_>,
        transform: Affine,
        context_color: AlphaColor<Srgb>,
    );
}

// ---------------------------------------------------------------------------
// Cache orchestration (fill / stroke)
// ---------------------------------------------------------------------------

/// Fill a prepared glyph, using the bitmap cache when possible and falling
/// back to direct rendering otherwise.
pub(crate) fn fill_glyph<B: GlyphAtlasBackend>(
    renderer: &mut B::Renderer,
    prepared_glyph: PreparedGlyph<'_>,
    bitmap_cache: &mut B::Cache,
    image_cache: &mut ImageCache,
) where
    B::Renderer: GlyphRenderer<B::Cache>,
{
    let cache_key = prepared_glyph.cache_key;
    let transform = prepared_glyph.transform;

    match prepared_glyph.glyph_type {
        GlyphType::Outline(glyph) => {
            // Try cached rendering first
            if let Some(ref key) = cache_key {
                if let CacheOutcome::Inserted = insert_outline_to_cache::<B>(
                    renderer,
                    glyph.path,
                    transform,
                    key,
                    bitmap_cache,
                    image_cache,
                ) {
                    return;
                }
            }

            B::render_outline_directly(renderer, glyph.path, transform);
        }
        GlyphType::Bitmap(glyph) => {
            // Try cached rendering first
            if let Some(ref key) = cache_key {
                if let CacheOutcome::Inserted = insert_bitmap_to_cache::<B>(
                    renderer,
                    &glyph,
                    transform,
                    key,
                    bitmap_cache,
                    image_cache,
                ) {
                    return;
                }
            }

            B::render_bitmap_directly(renderer, glyph, transform);
        }
        GlyphType::Colr(glyph) => {
            if let Some(key) = cache_key {
                if let CacheOutcome::Inserted = insert_colr_to_cache::<B>(
                    renderer,
                    &glyph,
                    transform,
                    key,
                    bitmap_cache,
                    image_cache,
                ) {
                    return;
                }
            }

            let context_color = renderer.get_context_color();
            B::render_colr_directly(renderer, &glyph, transform, context_color);
        }
    }
}

/// Stroke a prepared glyph, using the bitmap cache when possible and falling
/// back to direct rendering otherwise.
pub(crate) fn stroke_glyph<B: GlyphAtlasBackend>(
    renderer: &mut B::Renderer,
    prepared_glyph: PreparedGlyph<'_>,
    bitmap_cache: &mut B::Cache,
    image_cache: &mut ImageCache,
) where
    B::Renderer: GlyphRenderer<B::Cache>,
{
    match prepared_glyph.glyph_type {
        GlyphType::Outline(glyph) => {
            let cache_key = prepared_glyph.cache_key;
            let transform = prepared_glyph.transform;

            // Try cached rendering first
            if let Some(ref key) = cache_key {
                if let CacheOutcome::Inserted = insert_outline_to_cache::<B>(
                    renderer,
                    glyph.path,
                    transform,
                    key,
                    bitmap_cache,
                    image_cache,
                ) {
                    return;
                }
            }

            B::render_outline_directly(renderer, glyph.path, transform);
        }
        GlyphType::Bitmap(_) | GlyphType::Colr(_) => {
            // The definitions of COLR and bitmap glyphs can't meaningfully support being stroked.
            // (COLR's imaging model only has fills)
            fill_glyph::<B>(renderer, prepared_glyph, bitmap_cache, image_cache);
        }
    }
}

// ---------------------------------------------------------------------------
// Cache insertion helpers
// ---------------------------------------------------------------------------

/// Try to render an outline glyph using the bitmap cache.
///
/// On cache miss, allocates atlas space (the insert returns the per-page
/// command recorder) and delegates rasterisation to the backend via
/// [`GlyphAtlasBackend::render_outline_to_atlas`].
fn insert_outline_to_cache<B: GlyphAtlasBackend>(
    renderer: &mut B::Renderer,
    path: &BezPath,
    transform: Affine,
    cache_key: &GlyphCacheKey,
    bitmap_cache: &mut B::Cache,
    image_cache: &mut ImageCache,
) -> CacheOutcome {
    if !is_axis_aligned(&transform) {
        return CacheOutcome::NotAxisAligned;
    }

    let bounds = path.bounding_box();
    let meta = calculate_glyph_meta(&bounds);

    // Cache miss - allocate atlas space and record commands
    let subpixel_bucket = subpixel_offset(cache_key.subpixel_x);

    let Some((dst_x, dst_y, atlas_slot, recorder)) =
        bitmap_cache.insert(image_cache, cache_key.clone(), meta)
    else {
        return CacheOutcome::AtlasFull;
    };

    B::render_outline_to_atlas(path, subpixel_bucket, recorder, dst_x, dst_y, meta);

    render_outline_glyph_from_atlas::<B>(renderer, atlas_slot, transform);
    CacheOutcome::Inserted
}

/// Try to render a bitmap glyph from cache.
///
/// On cache miss, delegates atlas population to the backend via
/// [`GlyphAtlasBackend::queue_bitmap_upload_to_atlas`].
fn insert_bitmap_to_cache<B: GlyphAtlasBackend>(
    renderer: &mut B::Renderer,
    glyph: &GlyphBitmap,
    transform: Affine,
    cache_key: &GlyphCacheKey,
    bitmap_cache: &mut B::Cache,
    image_cache: &mut ImageCache,
) -> CacheOutcome {
    let width = glyph.pixmap.width();
    let height = glyph.pixmap.height();

    let meta = RasterMetrics {
        width,
        height,
        bearing_x: 0,
        bearing_y: 0,
    };

    // Bitmap glyphs don't use the command recorder — they queue pixel data
    // for direct upload instead.
    let Some((_dst_x, _dst_y, atlas_slot, _)) =
        bitmap_cache.insert(image_cache, cache_key.clone(), meta)
    else {
        return CacheOutcome::AtlasFull;
    };

    // Queue bitmap for later processing (both backends defer the actual upload/copy).
    // CPU: Copies to atlas before render_to_pixmap() call.
    // Hybrid: Uploads to GPU before render() call.
    B::queue_bitmap_upload_to_atlas(glyph, bitmap_cache, atlas_slot);

    // Render from atlas using OpaqueId - actual image resolution is delayed
    // until rasterization, so the queued upload/copy will happen before then.
    let paint_transform = B::bitmap_paint_transform(&atlas_slot);
    B::render_from_atlas(
        renderer,
        atlas_slot,
        transform,
        glyph.area,
        quality_for_scale(&transform),
        paint_transform,
    );
    CacheOutcome::Inserted
}

/// Try to render a COLR glyph using the cache.
///
/// On cache miss, allocates atlas space (the insert returns the per-page
/// command recorder) and delegates rasterisation to the backend via
/// [`GlyphAtlasBackend::render_colr_to_atlas`].
fn insert_colr_to_cache<B: GlyphAtlasBackend>(
    renderer: &mut B::Renderer,
    glyph: &ColrGlyph<'_>,
    transform: Affine,
    cache_key: GlyphCacheKey,
    bitmap_cache: &mut B::Cache,
    image_cache: &mut ImageCache,
) -> CacheOutcome {
    let width = glyph.pix_width;
    let height = glyph.pix_height;

    let meta = RasterMetrics {
        width,
        height,
        bearing_x: 0,
        bearing_y: 0,
    };

    let area = glyph.area;

    let Some((dst_x, dst_y, atlas_slot, recorder)) =
        bitmap_cache.insert(image_cache, cache_key.clone(), meta)
    else {
        return CacheOutcome::AtlasFull;
    };

    B::render_colr_to_atlas(glyph, cache_key.context_color, recorder, dst_x, dst_y);

    let paint_transform = B::colr_paint_transform(&atlas_slot);

    // Use the original fractional area to preserve sub-pixel accuracy
    B::render_from_atlas(
        renderer,
        atlas_slot,
        transform,
        area,
        quality_for_skew(&transform),
        paint_transform,
    );
    CacheOutcome::Inserted
}

// ---------------------------------------------------------------------------
// Atlas rendering helpers (from cached regions)
// ---------------------------------------------------------------------------

/// Render an outline glyph from atlas using bearing-based positioning.
///
/// Outline glyphs use the atlas slot's bearing values to compute the final position,
/// and always use low quality sampling (no scaling needed).
#[inline]
pub(crate) fn render_outline_glyph_from_atlas<B: GlyphAtlasBackend>(
    renderer: &mut B::Renderer,
    atlas_slot: AtlasSlot,
    transform: Affine,
) {
    let [_, _, _, _, tx, ty] = transform.as_coeffs();
    let rect_transform = Affine::translate((
        tx.floor() + atlas_slot.bearing_x as f64,
        ty.floor() + atlas_slot.bearing_y as f64,
    ));
    let area = Rect::new(0.0, 0.0, atlas_slot.width as f64, atlas_slot.height as f64);
    let paint_transform = B::outline_paint_transform(&atlas_slot);
    B::render_from_atlas(
        renderer,
        atlas_slot,
        rect_transform,
        area,
        ImageQuality::Low,
        paint_transform,
    );
}

/// Render a bitmap glyph from the atlas cache.
///
/// This bypasses glyph preparation and renders directly from a cached bitmap
/// in the atlas. Used for the fast path when glyphs are already cached.
#[inline]
pub(crate) fn render_bitmap_glyph_from_atlas<B: GlyphAtlasBackend>(
    renderer: &mut B::Renderer,
    atlas_slot: AtlasSlot,
    transform: Affine,
) {
    // Bitmap glyphs render at their natural size
    let area = Rect::new(0.0, 0.0, atlas_slot.width as f64, atlas_slot.height as f64);
    let paint_transform = B::bitmap_paint_transform(&atlas_slot);
    B::render_from_atlas(
        renderer,
        atlas_slot,
        transform,
        area,
        quality_for_scale(&transform),
        paint_transform,
    );
}

/// Render a COLR glyph from the atlas cache with fractional area.
///
/// This version accepts a pre-calculated fractional area to preserve
/// sub-pixel accuracy during rendering, avoiding scaling artifacts.
#[inline]
pub(crate) fn render_colr_glyph_from_atlas<B: GlyphAtlasBackend>(
    renderer: &mut B::Renderer,
    atlas_slot: AtlasSlot,
    transform: Affine,
    area: Rect,
) {
    let paint_transform = B::colr_paint_transform(&atlas_slot);
    B::render_from_atlas(
        renderer,
        atlas_slot,
        transform,
        area,
        quality_for_skew(&transform),
        paint_transform,
    );
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Calculate glyph metadata (bounds, bearings).
#[expect(
    clippy::cast_possible_truncation,
    reason = "glyph bounds fit in i32/u16/i16 at reasonable ppem values"
)]
pub(crate) fn calculate_glyph_meta(bounds: &Rect) -> RasterMetrics {
    // The path is already scaled to ppem, so we just need to account for subpixel offset
    // Add 1px margin for antialiasing
    let min_x = bounds.x0.floor() as i32 - 1;
    let max_x = bounds.x1.ceil() as i32 + 1;

    // For Y, we flip the coordinate system: font Y up -> screen Y down
    // After flipping Y, min_y becomes -max_y and max_y becomes -min_y
    let flipped_min_y = (-bounds.y1).floor() as i32 - 1;
    let flipped_max_y = (-bounds.y0).ceil() as i32 + 1;

    let width = (max_x - min_x) as u16;
    let height = (flipped_max_y - flipped_min_y) as u16;

    RasterMetrics {
        width,
        height,
        bearing_x: min_x as i16,
        bearing_y: flipped_min_y as i16,
    }
}

/// Check if a transform is axis-aligned (no rotation or skew).
#[inline]
pub(crate) fn is_axis_aligned(transform: &Affine) -> bool {
    !has_skew(transform)
}

/// Check if a transform has skew or rotation.
#[inline]
pub(crate) fn has_skew(transform: &Affine) -> bool {
    let [_, b, c, _, _, _] = transform.as_coeffs();
    b.abs() > 1e-6 || c.abs() > 1e-6
}

/// Determine image quality based on transform scale (for bitmap glyphs).
#[inline]
pub(crate) fn quality_for_scale(transform: &Affine) -> ImageQuality {
    let [a, _, _, d, _, _] = transform.as_coeffs();
    if a < 0.5 || d < 0.5 {
        ImageQuality::High
    } else {
        ImageQuality::Medium
    }
}

/// Determine image quality based on skew (for pre-rasterized content).
#[inline]
pub(crate) fn quality_for_skew(transform: &Affine) -> ImageQuality {
    if has_skew(transform) {
        ImageQuality::Medium
    } else {
        ImageQuality::Low
    }
}

/// Pack an RGBA color into a u32 for use as cache key.
#[inline]
pub(crate) fn pack_color(color: AlphaColor<Srgb>) -> u32 {
    color.premultiply().to_rgba8().to_u32()
}
