// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley Core provides low level APIs for implementing text layout.
//!
//! It provides Unicode analysis and shaping of text. The intended workflow is to analyze and shape
//! a paragraph of text, followed by the caller cutting the paragraph into fragments, calling
//! Parley Core to reshape across the boundary and measure the resulting fragments.
//!
//! ## The pipeline
//!
//! 1. Font-independent **analysis** ([`Analyzer`] -> [`Analysis`]) produces per-character
//!    [`CharInfo`] and maximal [`Item`] runs of constant script, bidi level, language and
//!    [orientation](RunOrientation).
//! 2. Font-dependent **shaping** ([`ShapeContext`] -> [`ShapedText`]) resolves runs into
//!    positioned [`Glyph`]s and a cluster <-> text map. Reshape using
//!    [`ShapeContext::apply_break`] and [`ShapeContext::apply_concat`].
//!
//! Behavior like white-space processing, text alignment/justification, and text transformation are
//! concerns left to the caller.
//!
//! ## Features
//!
//! - `std` (enabled by default): use the standard library.
//! - `libm`: use `libm` for floating-point math on `no_std` targets.
//! - `complex-scripts`: dictionary-based breaking for CJK/Thai/Khmer/Lao/Myanmar.
//!
//! ## Example
//!
//! Analyze, shape, greedily break, reshape the breaks, and lay out every line.
//!
//! ```
//! use parley_core::{
//!     Analysis, AnalysisOptions, Analyzer, Boundary, ItemizeOptions, ShapeContext, ShapeInput,
//!     ShapedText, reorder_visual,
//! };
//!
//! let mut analyzer = Analyzer::new();
//! let mut shape_cx = ShapeContext::new();
//! let mut analysis = Analysis::new();
//! let mut shaped = ShapedText::new();
//!
//! let mut fonts = fontique::Collection::new(fontique::CollectionOptions::default());
//! let mut source = fontique::SourceCache::new(fontique::SourceCacheOptions::default());
//!
//! let text = "The quick brown fox jumps over the lazy dog.";
//!
//! // 1. Font-independent analysis.
//! analyzer.analyze(text, &AnalysisOptions::default(), &mut analysis);
//!
//! // 2. Itemize + shape.
//! let mut query = fonts.query(&mut source);
//! query.set_families([fontique::QueryFamily::Generic(
//!     fontique::GenericFamily::SansSerif,
//! )]);
//! // If you have style spans, call `itemize_with` instead of `items`.
//! for item in analysis.items(text, &ItemizeOptions::default()) {
//!     shape_cx.shape_run(
//!         &ShapeInput {
//!             text,
//!             analysis: &analysis,
//!             text_range: item.text_range.clone(),
//!             char_range: item.char_range.clone(),
//!             script: item.script,
//!             language: item.language,
//!             level: item.level,
//!             orientation: item.orientation,
//!             attributes: fontique::Attributes::default(),
//!             font_size: 16.0,
//!             features: &[],
//!             variations: &[],
//!             letter_spacing: 0.0,
//!             word_spacing: 0.0,
//!         },
//!         &mut query,
//!         &mut shaped,
//!     );
//! }
//!
//! // 3. Greedy line breaking over the shaped clusters.
//! let breaks = greedy_breaks(&shaped, 200.0);
//!
//! // 4. Apply each break, reshaping to sever cursive joins and ligatures. This tends to shrink
//! //    the lines, but to be fully correct you could remeasure and backtrack.
//! for &pos in &breaks {
//!     shape_cx.apply_break(text, &analysis, &mut shaped, pos, &mut query);
//! }
//!
//! // 5. Lay out every line. A run may straddle a break, so each line clips runs to its range.
//! let starts = core::iter::once(0).chain(breaks.iter().copied());
//! let ends = breaks.iter().copied().chain(core::iter::once(text.len()));
//! let mut pen_y = 0.0_f32;
//! for (start, end) in starts.zip(ends) {
//!     let mut runs: Vec<_> = shaped
//!         .runs()
//!         .filter(|r| r.text_range().start < end && r.text_range().end > start)
//!         .collect();
//!     reorder_visual(&mut runs, |r| r.bidi_level());
//!     let ascent = runs.iter().map(|r| r.metrics().ascent).fold(0.0_f32, f32::max);
//!     let descent = runs.iter().map(|r| r.metrics().descent).fold(0.0_f32, f32::max);
//!     let leading = runs.iter().map(|r| r.metrics().leading).fold(0.0_f32, f32::max);
//!     let mut pen_x = 0.0_f32;
//!     for run in &runs {
//!         for cluster in run.clusters() {
//!             if !(start..end).contains(&cluster.text_range().start) {
//!                 continue;
//!             }
//!             for glyph in cluster.glyphs() {
//!                 let _ = (glyph.id, run.font(), pen_x + glyph.x, pen_y + ascent + glyph.y);
//!             }
//!             pen_x += cluster.advance();
//!         }
//!     }
//!     pen_y += ascent + descent + leading;
//! }
//!
//! /// First-fit greedy breaking; returns the byte offset where each new line starts.
//! fn greedy_breaks(shaped: &ShapedText, max_advance: f32) -> Vec<usize> {
//!     let mut breaks = Vec::new();
//!     let mut line_advance = 0.0_f32;
//!     let mut last_opp: Option<(usize, f32)> = None;
//!     for run in shaped.runs() {
//!         for cluster in run.clusters() {
//!             let at = cluster.text_range().start;
//!             match cluster.boundary() {
//!                 Boundary::Mandatory => {
//!                     breaks.push(at);
//!                     line_advance = 0.0;
//!                     last_opp = None;
//!                 }
//!                 Boundary::Line => last_opp = Some((at, line_advance)),
//!                 _ => {}
//!             }
//!             line_advance += cluster.advance();
//!             if line_advance > max_advance {
//!                 if let Some((pos, before)) = last_opp.take() {
//!                     breaks.push(pos);
//!                     line_advance -= before;
//!                 }
//!                 // No opportunity => over-long word; you could emit an emergency break here.
//!             }
//!         }
//!     }
//!     breaks
//! }
//! ```

// LINEBENDER LINT SET - lib.rs - v3
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]
#![expect(clippy::cast_possible_truncation, reason = "Deferred")]
#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod analysis;
pub mod analyzer;
pub mod itemize;
pub mod shape;

mod bidi;
mod common;
mod convert;
mod shaped_text;
mod util;

pub use analysis::CharInfo;
pub use analyzer::{Analysis, AnalysisOptions, Analyzer};
pub use bidi::{BidiLevel, BidiResolver, needs_bidi_resolution, reorder_visual};
pub use common::{
    Boundary, Direction, NormalizedCoord, RunMetrics, RunOrientation, TextOrientation, Whitespace,
    WritingMode,
};
pub use itemize::{Item, ItemKind, ItemizeOptions, Itemizer};
pub use shape::{ShapeContext, ShapeInput};
pub use shaped_text::{
    Cluster, ClusterGlyphs, Glyph, InlineBox, ReshapeRanges, Run, RunKind, ShapedText,
};
