// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The hyphen rendered at a soft-hyphen line break.

use crate::{FontInstance, Glyph, NormalizedCoord};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::raw::types::F2Dot14;

/// The glyph a font renders for the hyphen at a line break taken at a soft hyphen (U+00AD).
///
/// This is the font's glyph for U+2010 HYPHEN, or, if the font has none, for U+002D
/// HYPHEN-MINUS. Returns `None` if the font maps neither. The glyph's advance is scaled to
/// `font_size` at the given variation `coords`; its offset is zero.
pub fn hyphen_glyph(
    font: &FontInstance,
    font_size: f32,
    coords: &[NormalizedCoord],
) -> Option<Glyph> {
    let font_ref = skrifa::FontRef::from_index(font.font.data.as_ref(), font.font.index).ok()?;
    let charmap = font_ref.charmap();
    let id = charmap.map('\u{2010}').or_else(|| charmap.map('\u{2D}'))?;
    // Variable fonts need the location for the advance; up to 64 axes is plenty in practice.
    let mut location = [F2Dot14::default(); 64];
    let n = coords.len().min(location.len());
    for (dst, src) in location.iter_mut().zip(coords) {
        *dst = F2Dot14::from_bits(src.to_bits());
    }
    let metrics = font_ref.glyph_metrics(Size::new(font_size), LocationRef::new(&location[..n]));
    let advance = metrics.advance_width(id)?;
    Some(Glyph {
        id: id.to_u32(),
        x: 0.,
        y: 0.,
        advance,
    })
}
