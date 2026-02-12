// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Glyph bitmap atlas cache for efficient text rendering.
//!
//! This module provides a glyph bitmap atlas cache that:
//! - Rasterizes glyphs once and reuses the bitmaps for subsequent draws
//! - Packs glyph bitmaps into shared atlas images using guillotiere
//! - Uses stable glyph keys for reliable cache hits
//! - Supports multiple atlas pages for scalability
//! - Implements simple age-based eviction
//!
//! The cache is split into a common trait ([`GlyphCache`]) with shared logic
//! in [`GlyphAtlas`], and concrete implementations in the renderer
//! modules:
//! - [`CpuGlyphAtlas`](crate::renderers::vello_cpu::CpuGlyphAtlas) — owns `Pixmap`
//!   storage for CPU rendering
//! - [`GpuGlyphAtlas`](crate::renderers::vello_hybrid::GpuGlyphAtlas) — no
//!   local pixel storage (GPU manages textures)

pub(crate) mod cache;
pub mod commands;
pub(crate) mod key;
mod region;

#[cfg(feature = "std")]
pub use cache::GlyphCacheStats;
pub use cache::{
    AtlasConfig, GLYPH_PADDING, GlyphCache, GlyphAtlas, ImageCache, MAX_GLYPH_SIZE,
    PendingBitmapUpload,
};
pub use commands::{AtlasCommand, AtlasCommandRecorder, AtlasPaint};
pub use key::GlyphCacheKey;
pub use region::{AtlasSlot, RasterMetrics};
