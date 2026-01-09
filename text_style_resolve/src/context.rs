// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{ComputedInlineStyle, ComputedParagraphStyle};

/// Context required to resolve inline styles.
///
/// This is a minimal context that supports:
/// - inheritance (`parent`)
/// - `initial` reset semantics
/// - root-relative units like `rem` (`root`)
///
/// `InlineResolveContext` is intentionally a small, extensible struct. It is marked
/// `#[non_exhaustive]` and uses private fields so it can grow over time (for example to include
/// viewport information for `vw`/`vh` units) without breaking downstream code.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct InlineResolveContext<'a> {
    parent: &'a ComputedInlineStyle,
    initial: &'a ComputedInlineStyle,
    root: &'a ComputedInlineStyle,
}

impl<'a> InlineResolveContext<'a> {
    /// Creates a new resolution context.
    pub fn new(
        parent: &'a ComputedInlineStyle,
        initial: &'a ComputedInlineStyle,
        root: &'a ComputedInlineStyle,
    ) -> Self {
        // NOTE: This is deliberately a small struct with accessors so we can extend it later
        // (e.g. add viewport metrics for `vw`/`vh`/`vmin`/`vmax`) without redesigning call sites.
        Self {
            parent,
            initial,
            root,
        }
    }

    /// Returns the parent (inherited) computed style.
    pub fn parent(&self) -> &'a ComputedInlineStyle {
        self.parent
    }

    /// Returns the style used for `initial` resets.
    pub fn initial(&self) -> &'a ComputedInlineStyle {
        self.initial
    }

    /// Returns the root style used for root-relative units such as `rem`.
    pub fn root(&self) -> &'a ComputedInlineStyle {
        self.root
    }
}

/// Context required to resolve paragraph styles.
///
/// This mirrors [`InlineResolveContext`], but paragraph resolution is currently infallible.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct ParagraphResolveContext<'a> {
    parent: &'a ComputedParagraphStyle,
    initial: &'a ComputedParagraphStyle,
    root: &'a ComputedParagraphStyle,
}

impl<'a> ParagraphResolveContext<'a> {
    /// Creates a new resolution context.
    pub fn new(
        parent: &'a ComputedParagraphStyle,
        initial: &'a ComputedParagraphStyle,
        root: &'a ComputedParagraphStyle,
    ) -> Self {
        // NOTE: This is deliberately a small struct with accessors so we can extend it later.
        Self {
            parent,
            initial,
            root,
        }
    }

    /// Returns the parent (inherited) computed style.
    pub fn parent(&self) -> &'a ComputedParagraphStyle {
        self.parent
    }

    /// Returns the style used for `initial` resets.
    pub fn initial(&self) -> &'a ComputedParagraphStyle {
        self.initial
    }

    /// Returns the root style used for root-relative properties.
    pub fn root(&self) -> &'a ComputedParagraphStyle {
        self.root
    }
}
