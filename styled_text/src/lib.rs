// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Styled Text stores text with compact full-style identifiers.
//! It builds on [`attributed_text`] for range storage and segment resolution,
//! then adds interned style payloads for callers that need compact, reusable
//! style data.
//!
//! The core idea is that each resolved text segment points at a [`StyleId`].
//! The [`StyleSet`] behind that id stores complete style records, with
//! layout-affecting payloads and paint-only payloads interned separately.
//! That means a paint-only change can share layout style identity with the
//! surrounding text, which is the information a shaping or layout cache usually
//! wants.
//!
//! This is deliberately not a document model.
//! It does not own shaping, font resolution, inline boxes, cascading, or
//! renderer-specific style semantics.
//! Callers choose their own layout and paint payload types, then adapt the
//! resolved segments to Parley or another layout system.
//!
//! Unlike an API that sets individual style bits on ranges and leaves a later
//! stage to interpret those bits, `styled_text` resolves patches into complete
//! style records before lowering.
//! That keeps the core crate independent of any toolkit's style vocabulary
//! while giving downstream code a simple stream of text ranges and full styles.
//!
//! ## Concepts
//!
//! - [`StyledText`] stores the text, the resolved [`StyleId`] spans, and the
//!   shared [`StyleSet`].
//! - [`StyleSet`] interns layout payloads, paint payloads, and the joined style
//!   records that point at them.
//! - [`StylePatch`] is the small trait callers implement to say how a partial
//!   style change updates their own full style types.
//! - [`StyledTextBuilder`] appends text and applies patches in the order they
//!   were applied to the builder.
//! - [`StyledSegmentsWorkspace`] is reusable scratch storage for iterating
//!   resolved styled segments without reallocating every time.
//!
//! ## Building styled text
//!
//! ```no_run
//! use styled_text::{StylePatch, StyledSegmentsWorkspace, StyledTextBuilder};
//!
//! #[derive(Clone, Debug, PartialEq, Default)]
//! struct LayoutStyle {
//!     font_size: f32,
//! }
//!
//! #[derive(Clone, Debug, PartialEq, Default)]
//! struct PaintStyle {
//!     rgba: [u8; 4],
//! }
//!
//! #[derive(Clone, Debug, Default)]
//! struct TextStyleChange {
//!     font_size: Option<f32>,
//!     rgba: Option<[u8; 4]>,
//! }
//!
//! impl StylePatch<LayoutStyle, PaintStyle> for TextStyleChange {
//!     fn apply_to(&self, layout: &mut LayoutStyle, paint: &mut PaintStyle) {
//!         if let Some(font_size) = self.font_size {
//!             layout.font_size = font_size;
//!         }
//!         if let Some(rgba) = self.rgba {
//!             paint.rgba = rgba;
//!         }
//!     }
//! }
//!
//! let mut text = StyledTextBuilder::new(
//!     LayoutStyle { font_size: 16.0 },
//!     PaintStyle { rgba: [0, 0, 0, 255] },
//! );
//! text.push("Hello ");
//! let styled_range = text.push_with(
//!     "styled",
//!     TextStyleChange {
//!         font_size: Some(28.0),
//!         ..TextStyleChange::default()
//!     },
//! );
//! text.apply(
//!     styled_range,
//!     TextStyleChange {
//!         rgba: Some([220, 40, 40, 255]),
//!         ..TextStyleChange::default()
//!     },
//! );
//! text.push(" text");
//! let styled = text.finish();
//!
//! let mut workspace = StyledSegmentsWorkspace::new();
//! for segment in workspace.segments(&styled) {
//!     let style = styled.style_set().segment_style(segment.style());
//!     // Feed segment.range() and style into layout or painting code.
//! }
//! ```
//!
//! ## Features
//!
//! - `std` (enabled by default): Enables the `std` feature of
//!   [`attributed_text`].

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

extern crate alloc;

mod segments;
mod style_set;
mod text;
mod text_builder;

pub use attributed_text::{Error, TextChunk, TextRange, TextStorage};
pub use segments::{StyledSegment, StyledSegmentsWorkspace};
pub use style_set::{
    LayoutStyleId, PaintStyleId, SegmentStyle, StyleId, StyleRecord, StyleSet, StyleSetBuilder,
};
pub use text::StyledText;
pub use text_builder::{StylePatch, StyledTextBuilder};
