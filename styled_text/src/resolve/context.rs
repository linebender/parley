// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ComputedInlineStyle, ComputedParagraphStyle};

/// Context required to resolve inline styles.
///
/// This is a minimal context that supports:
/// - inheritance (`parent`)
/// - `initial` reset semantics
/// - root-relative units like `rem` (`root`)
///
/// `InlineResolveContext` is intentionally a small struct with private fields so it can grow over
/// time (for example to include viewport information for `vw`/`vh` units).
#[derive(Clone, Copy, Debug)]
pub struct InlineResolveContext<'a> {
    parent: &'a ComputedInlineStyle,
    initial: &'a ComputedInlineStyle,
    root: &'a ComputedInlineStyle,
}

impl<'a> InlineResolveContext<'a> {
    /// Creates a new resolution context.
    #[inline]
    pub const fn new(
        parent: &'a ComputedInlineStyle,
        initial: &'a ComputedInlineStyle,
        root: &'a ComputedInlineStyle,
    ) -> Self {
        Self {
            parent,
            initial,
            root,
        }
    }

    /// Returns the parent (inherited) computed style.
    #[inline]
    pub const fn parent(&self) -> &'a ComputedInlineStyle {
        self.parent
    }

    /// Returns the style used for `initial` resets.
    #[inline]
    pub const fn initial(&self) -> &'a ComputedInlineStyle {
        self.initial
    }

    /// Returns the root style used for root-relative units such as `rem`.
    #[inline]
    pub const fn root(&self) -> &'a ComputedInlineStyle {
        self.root
    }
}

/// Context required to resolve paragraph styles.
///
/// This mirrors [`InlineResolveContext`], but paragraph resolution is currently infallible.
#[derive(Clone, Copy, Debug)]
pub struct ParagraphResolveContext<'a> {
    parent: &'a ComputedParagraphStyle,
    initial: &'a ComputedParagraphStyle,
    root: &'a ComputedParagraphStyle,
}

impl<'a> ParagraphResolveContext<'a> {
    /// Creates a new resolution context.
    #[inline]
    pub const fn new(
        parent: &'a ComputedParagraphStyle,
        initial: &'a ComputedParagraphStyle,
        root: &'a ComputedParagraphStyle,
    ) -> Self {
        Self {
            parent,
            initial,
            root,
        }
    }

    /// Returns the parent (inherited) computed style.
    #[inline]
    pub const fn parent(&self) -> &'a ComputedParagraphStyle {
        self.parent
    }

    /// Returns the style used for `initial` resets.
    #[inline]
    pub const fn initial(&self) -> &'a ComputedParagraphStyle {
        self.initial
    }

    /// Returns the root style used for root-relative properties.
    #[inline]
    pub const fn root(&self) -> &'a ComputedParagraphStyle {
        self.root
    }
}
