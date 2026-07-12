// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Data structures to store the result of shaping.

use crate::{FontInstance, itemize::TextRange};

// TODO: The data here will probably get a shape roughly as follows.
//
// pub struct ShapedText {
//     runs: Vec<ShapedRun>,
//     clusters: Vec<ClusterData>,
//     glyphs: Vec<Glyph>,
//     coords: Vec<i16>,
//     fonts: Vec<FontInstance>,
//     features: Vec<FontFeature>,
//     ...
// }
//
// pub struct ShapedRun {
//     /// Font size.
//     pub font_size: f32,
//     /// The range of text this run corresponds to.
//     pub text_range: TextRange,
//     /// This run's glyphs, as a range into [`ShapedText::glyphs`].
//     pub glyph_range: Range<usize>,
//     /// The normalized variation coords of this run, as a range into [`ShapedText::coords`].
//     pub coords_range: Range<usize>,
//     /// This run's font, as an index into [`ShapedText::fonts`].
//     pub font_index: Range<usize>,
//     /// The bidi level of the run.
//     pub bidi_level: u8,
//     ...
// }

/// A shaped run. This is a run of glyphs within an [`Item`][`crate::itemize::Item`] that can be
/// rendered with a single font.
///
/// This struct will change shape, as it's currently provided in the callback of
/// [`crate::Shaper::shape_item`], but will become an encoding of a run within a larger
/// `parley_core::ShapedText`.
#[derive(Clone, Debug)]
pub struct ShapedRun<'a> {
    /// The range within the original text this run corresponds to.
    pub range: TextRange,

    /// The glyphs of the run.
    pub glyph_buffer: &'a harfrust::GlyphBuffer,

    /// The font this run's glyphs come from.
    pub font: FontInstance,

    /// Normalized font variation coordinates.
    pub coords: &'a [harfrust::NormalizedCoord],
}
