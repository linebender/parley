// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Cache key for glyph bitmaps stored in the atlas.
//!
//! [`GlyphCacheKey`] captures every parameter that affects the visual appearance
//! of a rasterized glyph — font identity, size, hinting, subpixel position,
//! COLR context color, and variable-font coordinates. Two keys that compare
//! equal produce identical bitmaps and can safely share a single atlas entry.

use core::hash::{Hash, Hasher};
#[cfg(not(feature = "std"))]
use core_maths::CoreFloat as _;
use skrifa::instance::NormalizedCoord;
use smallvec::SmallVec;
use vello_common::color::{AlphaColor, Srgb};

/// Number of horizontal subpixel quantization buckets (valid range: 1–255).
///
/// Higher values improve rendering quality at the cost of more atlas entries
/// per glyph. Common values: 1 (disabled), 2, 4 (default), 8.
pub(crate) const SUBPIXEL_BUCKETS: u8 = 4;

/// Unique identifier for a cached glyph bitmap.
///
/// Two glyphs with the same key are visually identical and can share
/// the same cached bitmap. The key includes all parameters that affect
/// the glyph's appearance.
///
/// All fields including `var_coords` are included in Hash/Eq, so glyphs
/// with different variation settings are treated as distinct cache entries.
#[derive(Clone, Debug)]
pub struct GlyphCacheKey {
    /// Unique identifier for the font blob.
    pub font_id: u64,
    /// Index within font collection (for TTC files).
    pub font_index: u32,
    /// Glyph index within the font.
    pub glyph_id: u32,
    /// Font size as f32 bits (exact match, no quantization).
    pub size_bits: u32,
    /// Whether hinting was applied.
    pub hinted: bool,
    /// Horizontal subpixel position (0 to SUBPIXEL_BUCKETS-1).
    pub subpixel_x: u8,
    /// Context color for COLR glyphs (packed RGBA). 0 for non-COLR glyphs.
    pub context_color: AlphaColor<Srgb>,
    /// Variation coordinates for variable fonts.
    pub var_coords: SmallVec<[NormalizedCoord; 4]>,
}

impl GlyphCacheKey {
    /// Creates a new cache key.
    ///
    /// `fractional_x` (the fractional pixel offset) is quantized into
    /// `SUBPIXEL_BUCKETS` buckets, so nearby positions share the same entry.
    #[inline]
    pub fn new(
        font_id: u64,
        font_index: u32,
        glyph_id: u32,
        size: f32,
        hinted: bool,
        fractional_x: f32,
        context_color: AlphaColor<Srgb>,
        var_coords: &[NormalizedCoord],
    ) -> Self {
        Self {
            font_id,
            font_index,
            glyph_id,
            size_bits: size.to_bits(),
            hinted,
            subpixel_x: quantize_subpixel(fractional_x),
            context_color,
            var_coords: SmallVec::from_slice(var_coords),
        }
    }
}

/// Manual `Hash` and `PartialEq` are required because `AlphaColor<Srgb>`
/// does not implement `Hash`/`Eq`. We pack it into a `u32` (premultiplied RGBA8)
/// so that the color participates in hashing and comparison deterministically.
impl Hash for GlyphCacheKey {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.font_id.hash(state);
        self.font_index.hash(state);
        self.glyph_id.hash(state);
        self.size_bits.hash(state);
        self.hinted.hash(state);
        self.subpixel_x.hash(state);
        let context_color = pack_color(self.context_color);
        context_color.hash(state);
        self.var_coords.hash(state);
    }
}

impl PartialEq for GlyphCacheKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.font_id == other.font_id
            && self.font_index == other.font_index
            && self.glyph_id == other.glyph_id
            && self.size_bits == other.size_bits
            && self.hinted == other.hinted
            && self.subpixel_x == other.subpixel_x
            && pack_color(self.context_color) == pack_color(other.context_color)
            && self.var_coords == other.var_coords
    }
}

impl Eq for GlyphCacheKey {}

/// Premultiply and pack an RGBA color into a `u32` for bitwise hashing/comparison.
#[inline]
pub(crate) fn pack_color(color: AlphaColor<Srgb>) -> u32 {
    color.premultiply().to_rgba8().to_u32()
}

/// Quantize a fractional pixel offset into one of [`SUBPIXEL_BUCKETS`] buckets.
#[expect(
    clippy::cast_possible_truncation,
    reason = "result is clamped to SUBPIXEL_BUCKETS-1 which fits in u8"
)]
#[inline]
fn quantize_subpixel(frac: f32) -> u8 {
    let normalized = frac.fract();
    let normalized = if normalized < 0.0 {
        normalized + 1.0
    } else {
        normalized
    };
    ((normalized * SUBPIXEL_BUCKETS as f32).round() as u8).min(SUBPIXEL_BUCKETS - 1)
}

/// Convert a quantized bucket index back to the fractional pixel offset it represents.
#[inline]
pub fn subpixel_offset(quantized: u8) -> f32 {
    quantized as f32 / SUBPIXEL_BUCKETS as f32
}

#[cfg(test)]
mod tests {
    use vello_common::color::palette::css::BLACK;

    use super::*;

    #[test]
    fn test_quantize_subpixel() {
        // Test bucket boundaries
        assert_eq!(quantize_subpixel(0.0), 0);
        assert_eq!(quantize_subpixel(0.1), 0);
        assert_eq!(quantize_subpixel(0.2), 1);
        assert_eq!(quantize_subpixel(0.25), 1);
        assert_eq!(quantize_subpixel(0.4), 2);
        assert_eq!(quantize_subpixel(0.5), 2);
        assert_eq!(quantize_subpixel(0.6), 2);
        assert_eq!(quantize_subpixel(0.7), 3);
        assert_eq!(quantize_subpixel(0.75), 3);
        assert_eq!(quantize_subpixel(0.9), 3);
        assert_eq!(quantize_subpixel(1.0), 0);
    }

    #[test]
    fn test_subpixel_offset() {
        assert_eq!(subpixel_offset(0), 0.0);
        assert_eq!(subpixel_offset(1), 0.25);
        assert_eq!(subpixel_offset(2), 0.5);
        assert_eq!(subpixel_offset(3), 0.75);
    }

    #[test]
    fn test_key_equality() {
        let key1 = GlyphCacheKey::new(1, 0, 42, 16.0, true, 0.3, BLACK, &[]);
        let key2 = GlyphCacheKey::new(1, 0, 42, 16.0, true, 0.3, BLACK, &[]);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_var_coords_in_equality() {
        // Two keys with different var_coords should NOT be equal
        let key1 = GlyphCacheKey::new(1, 0, 42, 16.0, true, 0.3, BLACK, &[]);
        let key2 = GlyphCacheKey::new(
            1,
            0,
            42,
            16.0,
            true,
            0.3,
            BLACK,
            &[NormalizedCoord::from_bits(100)],
        );
        assert_ne!(key1, key2);

        // Two keys with the same var_coords should be equal
        let key3 = GlyphCacheKey::new(
            1,
            0,
            42,
            16.0,
            true,
            0.3,
            BLACK,
            &[NormalizedCoord::from_bits(100)],
        );
        assert_eq!(key2, key3);
    }
}
