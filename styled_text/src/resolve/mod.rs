// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Specified→computed resolution for [`style`](crate::style).
//!
//! [`style`](crate::style) intentionally focuses on a lightweight, shareable style vocabulary:
//! declarations, specified values, and common value types.
//!
//! This module provides the “engine” layer:
//! - Computed style types ([`ComputedInlineStyle`], [`ComputedParagraphStyle`])
//! - Resolution contexts ([`InlineResolveContext`], [`ParagraphResolveContext`])
//! - Specified→computed resolution
//!
//! It is `no_std` + `alloc` friendly.

mod computed;
mod context;
mod engine;

#[cfg(test)]
mod tests;

pub use computed::{ComputedInlineStyle, ComputedLineHeight, ComputedParagraphStyle};
pub use context::{InlineResolveContext, ParagraphResolveContext};
pub use engine::{resolve_inline_declarations, resolve_paragraph_declarations};

use crate::style::{InlineStyle, ParagraphStyle};

/// Extension trait that adds resolution helpers to [`style`](crate::style) types.
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
    type Computed = ComputedInlineStyle;
    type Context<'a> = InlineResolveContext<'a>;

    #[inline]
    fn resolve(&self, ctx: Self::Context<'_>) -> Self::Computed {
        resolve_inline_declarations(self.declarations(), ctx)
    }
}

impl ResolveStyleExt for ParagraphStyle {
    type Computed = ComputedParagraphStyle;
    type Context<'a> = ParagraphResolveContext<'a>;

    #[inline]
    fn resolve(&self, ctx: Self::Context<'_>) -> Self::Computed {
        resolve_paragraph_declarations(self.declarations(), ctx)
    }
}
