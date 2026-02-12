// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Atlas slot and rasterization data structures.

use vello_common::paint::ImageId;

/// Location and metrics of a cached glyph within an atlas page.
#[derive(Clone, Copy, Debug)]
pub struct AtlasSlot {
    /// The image ID for this glyph in the [`ImageCache`].
    ///
    /// Used for deallocation and for looking up the atlas page/offset.
    ///
    /// [`ImageCache`]: vello_common::image_cache::ImageCache
    pub image_id: ImageId,

    /// Which atlas page contains this glyph.
    pub page_index: u32,

    /// X position in atlas (pixels).
    pub x: u16,

    /// Y position in atlas (pixels).
    pub y: u16,

    /// Width of glyph bitmap (pixels).
    pub width: u16,

    /// Height of glyph bitmap (pixels).
    pub height: u16,

    /// Horizontal bearing (offset from origin to left edge of glyph).
    /// This is used to position the glyph correctly when blitting.
    pub bearing_x: i16,

    /// Vertical bearing (offset from origin to top edge of glyph).
    /// This is used to position the glyph correctly when blitting.
    pub bearing_y: i16,
}

/// Metadata for a rasterized glyph (no pixel data).
///
/// Used with scratch buffer rendering to avoid per-glyph heap allocations.
/// The actual pixel data lives in a reusable scratch buffer.
#[derive(Clone, Copy, Debug)]
pub struct RasterMetrics {
    /// Width of the rasterized glyph in pixels.
    pub width: u16,
    /// Height of the rasterized glyph in pixels.
    pub height: u16,
    /// Horizontal bearing (offset from glyph origin to left edge).
    pub bearing_x: i16,
    /// Vertical bearing (offset from glyph origin to top edge).
    pub bearing_y: i16,
}
