// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Glyph bitmap cache key.

use crate::renderers::vello_renderer::pack_color;
use core::hash::{Hash, Hasher};
use skrifa::instance::NormalizedCoord;
use smallvec::SmallVec;
use vello_common::color::{AlphaColor, Srgb};

/// Number of subpixel quantization buckets (1-255).
/// More buckets = better quality but more cache entries.
/// Common values: 1 (disabled), 2, 4 (default), 8.
pub(crate) const SUBPIXEL_BUCKETS: u8 = 4;

/// Unique identifier for a cached glyph bitmap.
///
/// Two glyphs with the same key are visually identical and can share
/// the same cached bitmap. The key includes all parameters that affect
/// the glyph's appearance.
///
/// Note: `var_coords` is NOT included in Hash/Eq - it's used for
/// cache partition selection instead.
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
    /// NOT included in Hash/Eq - used for partition selection.
    pub var_coords: SmallVec<[NormalizedCoord; 4]>,
}

impl GlyphCacheKey {
    /// Creates a new key for outline glyphs.
    ///
    /// # Arguments
    /// * `font_id` - Unique identifier for the font blob
    /// * `font_index` - Index within font collection
    /// * `glyph_id` - Glyph index within the font
    /// * `size` - Font size in pixels per em
    /// * `hinted` - Whether hinting is applied
    /// * `fractional_x` - Fractional x position (0.0 to 1.0)
    /// * `var_coords` - Variation coordinates for variable fonts
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

// Hash only the rendering-affecting fields, NOT var_coords
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
        // var_coords intentionally NOT hashed
    }
}

// Eq only the rendering-affecting fields, NOT var_coords
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
        // var_coords intentionally NOT compared
    }
}

impl Eq for GlyphCacheKey {}

/// Quantize fractional position to [`SUBPIXEL_BUCKETS`] buckets.
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

/// Returns the subpixel offset value for a quantized bucket.
#[inline]
pub(crate) fn subpixel_offset(quantized: u8) -> f32 {
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
    fn test_var_coords_not_in_equality() {
        // Two keys with different var_coords should still be equal
        // (var_coords is for partition selection, not equality)
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
        assert_eq!(key1, key2);
    }
}
