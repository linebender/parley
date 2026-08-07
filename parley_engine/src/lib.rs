// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley Engine provides low level APIs for implementing text layout.
//!
//! ## Features
//!
//! - `std` (enabled by default): This is currently unused and is provided for forward compatibility.
//!
//! ## Typical shaping pipeline
//!
//! A caller normally reuses an [`Analyzer`], [`Analysis`], and [`Shaper`] while
//! laying out text. The analysis must be produced before itemization and
//! shaping, and the same source string must be passed to each stage:
//!
//! ```text
//! analyzer.analyze(text, &options, &mut analysis);
//! for item in analysis.itemize(text, split_after) {
//!     shaper.shape_item(text, &analysis, &item, &shape_options, select_font, &mut shaped_text);
//! }
//! ```
//!
//! [`Analysis::itemize`] divides text into runs with compatible bidi and
//! script properties. [`Shaper::shape_item`] then turns each run into glyphs;
//! the `select_font` callback chooses a [`FontInstance`] for each character
//! cluster. Higher-level users should generally prefer `parley`'s
//! [`LayoutContext`](https://docs.rs/parley/latest/parley/struct.LayoutContext.html),
//! which manages these stages and adds style resolution and line layout.

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
#![no_std]
#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

mod analysis;
mod analyzer;
pub mod bidi;
pub mod break_overrides;
mod glyph;
pub mod itemize;
mod lru_cache;
pub mod shape;

pub use linebender_resource_handle::FontData;
pub use parlance::BaseDirection;

pub use analysis::{Analysis, AnalysisDataSources, Boundary, CharInfo};
pub use analyzer::{AnalysisOptions, Analyzer};
pub use glyph::Glyph;
pub use shape::atom::{Atom, Atoms, Grapheme, Graphemes, ShapedSlice};
pub use shape::shaped_text::{FontMetrics, NormalizedCoord, ShapedRun, ShapedText};
pub use shape::shaper::{FontInstance, ShapeOptions, Shaper};
