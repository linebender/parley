// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use parlance::FontFeature;

use crate::{FontInstance, Glyph, itemize::TextRange, shape::ClusterData};

// #[derive(Clone, Debug)]
// pub struct ShapedText {
//     // TODO: implement a `ShapedRun` inside `parley_core`, at which point we'll store them here.
//     // runs: Vec<ShapedRun>,
//     pub(crate) clusters: Vec<ClusterData>,
//     pub(crate) glyphs: Vec<Glyph>,
//     pub(crate) coords: Vec<i16>,
//     pub(crate) features: Vec<FontFeature>,
// }

// #[derive(Clone, Debug)]
// pub struct ShapedRun {
//     pub font_index: FontInstance,
//     /// Font size.
//     pub font_size: f32,
//     /// The range of text this run corresponds to.
//     pub text_range: TextRange,
//     /// Bidi level for the run.
//     pub bidi_level: u8,
//
// }

/// This struct will change shape, as it's currently provided in the callback of
/// [`crate::ShapeContext::shape_item`], but will become an encoding of a run within a larger
/// `parley_core::ShapedText`.
pub struct ShapedRun<'a> {
    pub range: TextRange,
    pub font: FontInstance,
    pub glyph_buffer: &'a harfrust::GlyphBuffer,
    pub coords: &'a [harfrust::NormalizedCoord],
}
