// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! CSS-inspired text style vocabulary and resolution.
//!
//! This crate defines:
//! - A closed set of inline and paragraph style properties (the vocabulary)
//! - CSS-like reset semantics via [`Specified`]
//! - Resolution of specified styles into *computed* (absolute) styles
//!
//! It is `no_std` + `alloc` friendly and is intentionally independent of any shaping/layout engine.
//!
//! ## Scope
//!
//! This crate focuses on portable, engine-agnostic style semantics (fonts, metrics, bidi, and
//! OpenType settings). It intentionally does **not** model paint/brush types (color/gradients),
//! and it currently does not expose detailed decoration geometry (underline/strikethrough
//! thickness/offset) or decoration paints. Those are expected to live in wrapper attributes or in
//! an engine-lowering layer.
//!
//! The primary integration pattern is:
//! - Author spans using [`InlineStyle`] declarations
//! - Resolve to [`ComputedInlineStyle`] runs (e.g. via a higher-level crate)
//! - Lower computed runs to a layout engine such as Parley
//!
//! ## Model
//!
//! This crate is structured similarly to CSS:
//!
//! - **Specified values** are expressed as declaration lists ([`InlineStyle`], [`ParagraphStyle`]).
//! - Declarations store values wrapped in [`Specified`], enabling `inherit`/`initial` behavior.
//! - **Computed values** are absolute and engine-ready ([`ComputedInlineStyle`],
//!   [`ComputedParagraphStyle`]).
//!
//! Some inline declarations (such as OpenType feature/variation settings) can fail to parse when
//! provided as raw CSS-like source strings. In those cases, inline resolution returns an error.
//!
//! ## OpenType Settings
//!
//! OpenType feature and variation settings are represented as [`Settings`]. For convenience, they
//! can be authored either as a parsed list ([`Settings::List`]) or as a CSS-like source string
//! ([`Settings::Source`]). When using `Source`, parsing happens during inline style resolution and
//! invalid input will return a [`ResolveStyleError`].
//!
//! Resolution is performed relative to three computed styles:
//!
//! - `parent`: the inherited style context
//! - `initial`: the style used for [`Specified::Initial`]
//! - `root`: the style used for root-relative units such as `rem`
//!
//! The crate does not define how you obtain `parent`/`initial`; that is typically provided by a
//! higher-level layer (e.g. a styled text model, a document model, or a UI toolkit).
//!
//! ## Conflict Handling
//!
//! Styles are lists of declarations rather than “one field per property”. When multiple
//! declarations of the same property are present, the **last** declaration in the list wins.
//! When multiple overlapping spans apply to the same text, the higher-level layer is expected to
//! define an application order (commonly: span application order, last writer wins).
//!
//! ## Relative Values
//!
//! Some specified values are relative (for example [`FontSize::Em`], [`Spacing::Em`],
//! [`LineHeight::Em`], and their root-relative forms like `rem`). These are resolved against
//! *computed* context:
//!
//! - `font-size: Em(x)` is resolved against the **parent** computed font size.
//! - `font-size: Rem(x)` is resolved against the **root** computed font size.
//! - properties like `letter-spacing` and `word-spacing` are resolved against the **computed**
//!   font size for the same resolved style.
//!
//! This gives deterministic results for overlapping declarations and matches common CSS-like
//! systems.
//!
//! ## References
//!
//! This crate is inspired by (but not identical to) these specifications:
//!
//! - CSS Fonts: <https://www.w3.org/TR/css-fonts-4/>
//! - CSS Text: <https://www.w3.org/TR/css-text-3/> and <https://www.w3.org/TR/css-text-4/>
//! - Unicode Bidirectional Algorithm (UAX #9): <https://www.unicode.org/reports/tr9/>
//!
//! ## Example
//!
//! ```
//! use text_style::{
//!     BaseDirection, ComputedInlineStyle, ComputedParagraphStyle, FontSize, InlineResolveContext,
//!     InlineStyle, ParagraphResolveContext, ParagraphStyle, Specified,
//! };
//!
//! let base_inline = ComputedInlineStyle::default();
//! let base_paragraph = ComputedParagraphStyle::default();
//!
//! // "font-size: 1.25em; text-decoration-line: underline"
//! let inline = InlineStyle::new()
//!     .font_size(Specified::Value(FontSize::Em(1.25)))
//!     .underline(Specified::Value(true));
//! let inline_ctx = InlineResolveContext::new(&base_inline, &base_inline, &base_inline);
//! let computed_inline = inline.resolve(inline_ctx).unwrap();
//! assert_eq!(computed_inline.font_size_px(), base_inline.font_size_px() * 1.25);
//!
//! // "direction: rtl"
//! let paragraph = ParagraphStyle::new().base_direction(Specified::Value(BaseDirection::Rtl));
//! let paragraph_ctx = ParagraphResolveContext::new(&base_paragraph, &base_paragraph, &base_paragraph);
//! let computed_paragraph = paragraph.resolve(paragraph_ctx);
//! assert_eq!(computed_paragraph.base_direction(), BaseDirection::Rtl);
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
#![no_std]

extern crate alloc;

mod bidi;
mod computed;
mod context;
mod declarations;
mod error;
mod font;
mod paragraph;
mod parse;
mod resolve;
mod settings;
mod specified;
#[cfg(test)]
mod tests;
mod values;

pub use bidi::{BidiControl, BidiDirection, BidiOverride};
pub use computed::{
    ComputedInlineStyle, ComputedLineHeight, ComputedParagraphStyle, FontWeight, FontWidth,
};
pub use context::{InlineResolveContext, ParagraphResolveContext};
pub use declarations::{InlineDeclaration, ParagraphDeclaration};
pub use error::ResolveStyleError;
pub use font::{FontFamily, FontStack, GenericFamily};
pub use paragraph::{BaseDirection, OverflowWrap, ParagraphStyle, TextWrapMode, WordBreak};
pub use parse::ParseSettingsError;
pub use settings::{Setting, Settings, Tag};
pub use specified::Specified;
pub use values::{FontSize, FontStyle, LineHeight, Spacing};

pub use declarations::InlineStyle;
