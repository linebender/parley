// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Span styling and document structure built on [`attributed_text`] and [`text_style`].
//!
//! - [`text_style`] defines a closed style vocabulary.
//! - [`text_style_resolve`] resolves specified styles to computed styles.
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
//! ## Errors
//!
//! Inline style resolution can fail if any spans include declarations that require parsing (for
//! example OpenType settings supplied as a raw CSS-like string). In that case, run iterators yield
//! an error item.
//!
//! ## Example: Styled spans
//!
//! ```
//! use styled_text::StyledText;
//! use text_style::{FontSize, InlineStyle, Specified};
//! use text_style_resolve::{ComputedInlineStyle, ComputedParagraphStyle};
//!
//! let base_inline = ComputedInlineStyle::default();
//! let base_paragraph = ComputedParagraphStyle::default();
//! let mut text = StyledText::new("Hello world!", base_inline, base_paragraph);
//!
//! // Make "world!" 1.5x larger.
//! let world = 6..12;
//! let style = InlineStyle::new().font_size(Specified::Value(FontSize::Em(1.5)));
//! text.apply_span(world, style).unwrap();
//!
//! let runs: Vec<_> = text
//!     .resolved_inline_runs_coalesced()
//!     .map(Result::unwrap)
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
//! use text_style::InlineStyle;
//! use text_style_resolve::{ComputedInlineStyle, ComputedParagraphStyle};
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
//!     0..8,
//!     Attr {
//!         style: InlineStyle::new(),
//!         href: Some(Arc::from("https://example.invalid")),
//!     },
//! )
//! .unwrap();
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
