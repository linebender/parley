// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Specified→computed resolution for [`text_style`].
//!
//! [`text_style`] intentionally focuses on a lightweight, shareable style vocabulary:
//! declarations, specified values, and common value types.
//!
//! This crate provides the “engine” layer:
//! - Computed style types ([`ComputedInlineStyle`], [`ComputedParagraphStyle`])
//! - Resolution contexts ([`InlineResolveContext`], [`ParagraphResolveContext`])
//! - Specified→computed resolution (including parsing of raw OpenType settings sources)
//!
//! It is `no_std` + `alloc` friendly.
//!
//! ## Example
//!
//! ```
//! use text_style::{BaseDirection, FontSize, InlineStyle, ParagraphStyle, Specified};
//! use text_style_resolve::{
//!     ComputedInlineStyle, ComputedParagraphStyle, InlineResolveContext, ParagraphResolveContext,
//!     ResolveStyleExt,
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
//! assert!(computed_inline.underline());
//!
//! // "direction: rtl"
//! let paragraph = ParagraphStyle::new().base_direction(Specified::Value(BaseDirection::Rtl));
//! let paragraph_ctx =
//!     ParagraphResolveContext::new(&base_paragraph, &base_paragraph, &base_paragraph);
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

mod computed;
mod context;
mod error;
mod parse;
mod resolve;
#[cfg(test)]
mod tests;

pub use computed::{ComputedInlineStyle, ComputedLineHeight, ComputedParagraphStyle};
pub use context::{InlineResolveContext, ParagraphResolveContext};
pub use error::ResolveStyleError;
pub use parse::ParseSettingsError;

pub use resolve::{resolve_inline_declarations, resolve_paragraph_declarations};

use text_style::{InlineStyle, ParagraphStyle};

/// Extension trait that adds resolution helpers to [`text_style`] types.
pub trait ResolveStyleExt {
    /// The computed result type.
    type Computed;
    /// The resolution context type.
    type Context<'a>
    where
        Self: 'a;

    /// Resolves this style relative to the provided context.
    fn resolve(&self, ctx: Self::Context<'_>) -> Self::Computed;
}

impl ResolveStyleExt for InlineStyle {
    type Computed = Result<ComputedInlineStyle, ResolveStyleError>;
    type Context<'a> = InlineResolveContext<'a>;

    fn resolve(&self, ctx: Self::Context<'_>) -> Self::Computed {
        resolve_inline_declarations(self.declarations(), ctx)
    }
}

impl ResolveStyleExt for ParagraphStyle {
    type Computed = ComputedParagraphStyle;
    type Context<'a> = ParagraphResolveContext<'a>;

    fn resolve(&self, ctx: Self::Context<'_>) -> Self::Computed {
        resolve_paragraph_declarations(self.declarations(), ctx)
    }
}
