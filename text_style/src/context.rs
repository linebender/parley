// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{ComputedInlineStyle, ComputedParagraphStyle};

/// Inputs used to resolve specified inline declarations into a computed inline style.
//
// These contexts are `#[non_exhaustive]` and use private fields with accessors so we can extend
// them (for example with viewport data for `vw`/`vh`/`vmin`/`vmax`) without forcing downstream
// breaking changes.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct InlineResolveContext<'a> {
    /// The inherited style context for `Specified::Inherit` and relative units like `em`.
    parent: &'a ComputedInlineStyle,
    /// The initial style for `Specified::Initial`.
    initial: &'a ComputedInlineStyle,
    /// The root style for root-relative units like `rem`.
    root: &'a ComputedInlineStyle,
}

impl<'a> InlineResolveContext<'a> {
    /// Creates a context where `parent`, `initial`, and `root` are provided explicitly.
    pub fn new(
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

    /// Returns the parent style.
    pub fn parent(&self) -> &'a ComputedInlineStyle {
        self.parent
    }

    /// Returns the initial style.
    pub fn initial(&self) -> &'a ComputedInlineStyle {
        self.initial
    }

    /// Returns the root style.
    pub fn root(&self) -> &'a ComputedInlineStyle {
        self.root
    }
}

/// Inputs used to resolve specified paragraph declarations into a computed paragraph style.
//
// See `InlineResolveContext` for why these structs are designed for future expansion.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct ParagraphResolveContext<'a> {
    /// The inherited style context for `Specified::Inherit`.
    parent: &'a ComputedParagraphStyle,
    /// The initial style for `Specified::Initial`.
    initial: &'a ComputedParagraphStyle,
    /// The root style (reserved for future root-relative paragraph properties).
    root: &'a ComputedParagraphStyle,
}

impl<'a> ParagraphResolveContext<'a> {
    /// Creates a context where `parent`, `initial`, and `root` are provided explicitly.
    pub fn new(
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

    /// Returns the parent style.
    pub fn parent(&self) -> &'a ComputedParagraphStyle {
        self.parent
    }

    /// Returns the initial style.
    pub fn initial(&self) -> &'a ComputedParagraphStyle {
        self.initial
    }

    /// Returns the root style.
    pub fn root(&self) -> &'a ComputedParagraphStyle {
        self.root
    }
}
