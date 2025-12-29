// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Span styling and document structure built on [`attributed_text`].
//!
//! - [`style`] defines a closed style vocabulary.
//! - [`resolve`] resolves specified styles to computed styles.
//! - [`attributed_text`] stores generic attributes on byte ranges.
//! - `styled_text` combines them:
//!   - [`StyledText`]: a single layout block (maps cleanly to one Parley `Layout`)
//!   - [`StyledDocument`]: a flat sequence of blocks with semantic kinds (headings, list items…)
//!
//! ## Scope
//!
//! This crate provides span application, inline style run resolution, and a lightweight block
//! model. It does not itself lower styles to Parley APIs, and it does not define paint/brush types
//! (those are expected to live in wrapper attributes and an engine-lowering layer).
//!
//! ## Design Intent
//!
//! `StyledText` is intended to be a durable attributed-text model:
//! - it can be parsed from markup and retained as an application’s in-memory representation
//! - it can be used transiently as a “layout input packet” if you already have your own model
//! - it aims to be a reasonable interchange format for rich text (for example copy/paste)
//!
//! The mutation/editing story is still evolving. Short-term APIs focus on span application and
//! layout-facing iteration; richer mutation patterns (inserts/deletes with span adjustment, etc.)
//! are expected to be added over time.
//!
//! ## Indices
//!
//! All ranges are expressed as **byte indices** into UTF-8 text, and must be on UTF-8 character
//! boundaries (as required by [`attributed_text`]).
//!
//! ## Overlaps
//!
//! When spans overlap, inline style resolution applies spans in the order they were added (last
//! writer wins). Higher-level semantic attributes can be carried by wrapper types via
//! [`HasInlineStyle`].
//!
//! ## Example: Styled spans
//!
//! ```
//! use styled_text::StyledText;
//! use styled_text::style::{FontSize, InlineStyle, Specified};
//! use styled_text::resolve::{ComputedInlineStyle, ComputedParagraphStyle};
//!
//! let base_inline = ComputedInlineStyle::default();
//! let base_paragraph = ComputedParagraphStyle::default();
//! let mut text = StyledText::new("Hello world!", base_inline, base_paragraph);
//!
//! // Make "world!" 1.5x larger.
//! let world = 6..12;
//! let style = InlineStyle::new().font_size(Specified::Value(FontSize::Em(1.5)));
//! text.apply_span(text.range(world).unwrap(), style);
//!
//! let runs: Vec<_> = text
//!     .resolved_inline_runs_coalesced()
//!     .collect();
//! assert_eq!(runs.len(), 2);
//! assert_eq!(runs[1].range, 6..12);
//! ```
//!
//! ## Example: Wrapper attributes for semantics
//!
//! ```
//! # extern crate alloc;
//! use alloc::sync::Arc;
//! use styled_text::{HasInlineStyle, StyledText};
//! use styled_text::style::InlineStyle;
//! use styled_text::resolve::{ComputedInlineStyle, ComputedParagraphStyle};
//!
//! #[derive(Debug, Clone)]
//! struct Attr {
//!     style: InlineStyle,
//!     href: Option<Arc<str>>,
//! }
//!
//! impl HasInlineStyle for Attr {
//!     fn inline_style(&self) -> &InlineStyle {
//!         &self.style
//!     }
//! }
//!
//! let base_inline = ComputedInlineStyle::default();
//! let base_paragraph = ComputedParagraphStyle::default();
//! let mut text = StyledText::new("Click me", base_inline, base_paragraph);
//! text.apply_span(
//!     text.range(0..8).unwrap(),
//!     Attr {
//!         style: InlineStyle::new(),
//!         href: Some(Arc::from("https://example.invalid")),
//!     },
//! );
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

pub mod resolve;
pub mod style;

mod block;
mod document;
mod runs;
mod text;
mod traits;

#[cfg(test)]
mod tests;

pub use block::{Block, BlockKind};
pub use document::StyledDocument;
pub use runs::{CoalescedInlineRuns, InlineStyleRun, ResolvedInlineRuns};
pub use text::StyledText;
pub use traits::HasInlineStyle;

pub use resolve::{
    ComputedInlineStyle, ComputedLineHeight, ComputedParagraphStyle, InlineResolveContext,
    ParagraphResolveContext, ResolveStyleExt,
};
pub use style::{
    BaseDirection, BidiControl, BidiDirection, BidiOverride, FontFamily, FontFamilyName,
    FontFeature, FontFeatures, FontSize, FontStyle, FontVariation, FontVariations, FontWeight,
    FontWidth, GenericFamily, InlineDeclaration, InlineStyle, LineHeight, OverflowWrap,
    ParagraphDeclaration, ParagraphStyle, ParseFontFamilyError, ParseFontFamilyErrorKind,
    ParseLanguageError, ParseSettingsError, ParseSettingsErrorKind, Spacing, Specified, Tag,
    TextWrapMode, WordBreak,
};
