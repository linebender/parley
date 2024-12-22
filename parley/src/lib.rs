// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parley is a library for rich text layout.
//!
//! Some key types are:
//! - [`FontContext`] and [`LayoutContext`] are resources which should be shared globally (or at coarse-grained boundaries).
//!   - [`FontContext`] is database of fonts.
//!   - [`LayoutContext`] is scratch space that allows for reuse of allocations between layouts.
//! - [`RangedBuilder`] and [`TreeBuilder`] which are builders for creating a [`Layout`].
//!     - [`RangedBuilder`] allows styles to be specified as a flat `Vec` of spans
//!     - [`TreeBuilder`] allows styles to be specified as a tree of spans
//!
//!   They are constructed using the [`ranged_builder`](LayoutContext::ranged_builder) and [`tree_builder`](LayoutContext::ranged_builder) methods on [`LayoutContext`].
//! - [`Layout`] which represents styled paragraph(s) of text and can perform shaping, line-breaking, bidi-reordering, and alignment of that text.
//!
//!   `Layout` supports re-linebreaking and re-aligning many times (in case the width at which wrapping should occur changes). But if the text content or
//!   the styles applied to that content change then a new `Layout` must be created using a new `RangedBuilder` or `TreeBuilder`.
//!
//! ## Usage Example
//!
//! See the [examples](https://github.com/linebender/parley/tree/main/examples) directory for more complete usage examples that include rendering.
//!
//! ```rust
//! use parley::{
//!    Alignment, FontContext, FontWeight, InlineBox, Layout, LayoutContext, PositionedLayoutItem,
//!    StyleProperty,
//! };
//!
//! // Create a FontContext (font database) and LayoutContext (scratch space).
//! // These are both intended to be constructed rarely (perhaps even once per app):
//! let mut font_cx = FontContext::new();
//! let mut layout_cx = LayoutContext::new();
//!
//! // Create a `RangedBuilder` or a `TreeBuilder`, which are used to construct a `Layout`.
//! const DISPLAY_SCALE : f32 = 1.0;
//! const TEXT : &str = "Lorem Ipsum...";
//! let mut builder = layout_cx.ranged_builder(&mut font_cx, &TEXT, DISPLAY_SCALE);
//!
//! // Set default styles that apply to the entire layout
//! builder.push_default(StyleProperty::LineHeight(1.3));
//! builder.push_default(StyleProperty::FontSize(16.0));
//!
//! // Set a style that applies to the first 4 characters
//! builder.push(StyleProperty::FontWeight(FontWeight::new(600.0)), 0..4);
//!
//! // Add a box to be laid out inline with the text
//! builder.push_inline_box(InlineBox { id: 0, index: 5, width: 50.0, height: 50.0 });
//!
//! // Build the builder into a Layout
//! let mut layout: Layout<()> = builder.build(&TEXT);
//!
//! // Run line-breaking and alignment on the Layout
//! const MAX_WIDTH : Option<f32> = Some(100.0);
//! layout.break_all_lines(MAX_WIDTH);
//! layout.align(MAX_WIDTH, Alignment::Start);
//!
//! // Inspect computed layout (see examples for more details)
//! let width = layout.width();
//! let height = layout.height();
//! for line in layout.lines() {
//!     for item in line.items() {
//!         match item {
//!             PositionedLayoutItem::GlyphRun(glyph_run) => {
//!                 // Render the glyph run
//!             }
//!             PositionedLayoutItem::InlineBox(inline_box) => {
//!                 // Render the inline box
//!             }
//!         };
//!     }
//! }
//! ```

// LINEBENDER LINT SET - lib.rs - v1
// See https://linebender.org/wiki/canonical-lints/
// These lints aren't included in Cargo.toml because they
// shouldn't apply to examples and tests
#![warn(unused_crate_dependencies)]
#![warn(clippy::print_stdout, clippy::print_stderr)]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(missing_docs, reason = "We have many as-yet undocumented items.")]
#![expect(
    missing_debug_implementations,
    single_use_lifetimes,
    unnameable_types,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::cast_possible_truncation,
    clippy::exhaustive_enums,
    clippy::missing_assert_message,
    clippy::missing_panics_doc,
    clippy::shadow_unrelated,
    reason = "Deferred"
)]

#[cfg(not(any(feature = "std", feature = "libm")))]
compile_error!("parley requires either the `std` or `libm` feature to be enabled");

extern crate alloc;

pub use fontique;
pub use swash;

pub(crate) mod algos;

pub mod inputs;
pub mod outputs;

mod bidi;
mod inline_box;
mod resolve;
mod shape;
mod swash_convert;
mod util;

pub mod layout;

#[cfg(test)]
mod tests;

pub use peniko::kurbo::Rect;
pub use peniko::Font;

// TODO - Remove
pub use inputs::builder::{RangedBuilder, TreeBuilder};
pub use inputs::context::LayoutContext;
pub use inputs::font::FontContext;
pub use inputs::style::*;

pub use inline_box::InlineBox;
#[doc(inline)]
pub use layout::Layout;

pub use layout::editor::{PlainEditor, PlainEditorDriver};

pub use layout::*;
