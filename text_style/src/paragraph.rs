// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::ComputedParagraphStyle;
use crate::ParagraphDeclaration;
use crate::ParagraphResolveContext;
use crate::resolve::resolve_paragraph_declarations;
use crate::specified::Specified;
use alloc::vec::Vec;

/// The paragraph's base direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum BaseDirection {
    /// Choose direction automatically (commonly "first-strong").
    #[default]
    Auto,
    /// Left-to-right.
    Ltr,
    /// Right-to-left.
    Rtl,
}

/// Control over word breaking, named for the CSS property.
///
/// See: <https://www.w3.org/TR/css-text-3/#word-break-property>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum WordBreak {
    /// Customary rules.
    #[default]
    Normal,
    /// Breaking is allowed within "words".
    BreakAll,
    /// Breaking is forbidden within "words".
    KeepAll,
}

/// Control over "emergency" line-breaking.
///
/// See: <https://www.w3.org/TR/css-text-3/#overflow-wrap-property>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum OverflowWrap {
    /// Only break at opportunities specified by word-breaking rules.
    #[default]
    Normal,
    /// Words may be broken at an arbitrary point if needed.
    Anywhere,
    /// Like `Anywhere`, but treated differently for min-content sizing in some engines.
    BreakWord,
}

/// Control over non-"emergency" line-breaking.
///
/// See: <https://www.w3.org/TR/css-text-4/#text-wrap-mode>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TextWrapMode {
    /// Wrap as needed to prevent overflow.
    #[default]
    Wrap,
    /// Do not wrap at soft-wrap opportunities.
    NoWrap,
}

/// A set of specified paragraph declarations for a block of text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParagraphStyle {
    declarations: Vec<ParagraphDeclaration>,
}

impl ParagraphStyle {
    /// Creates an empty paragraph style (no declarations).
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the declarations in this style, in authoring order.
    pub fn declarations(&self) -> &[ParagraphDeclaration] {
        &self.declarations
    }

    /// Appends an arbitrary declaration.
    pub fn push(mut self, declaration: ParagraphDeclaration) -> Self {
        self.declarations.push(declaration);
        self
    }

    /// Sets the base direction.
    pub fn base_direction(self, value: Specified<BaseDirection>) -> Self {
        self.push(ParagraphDeclaration::BaseDirection(value))
    }

    /// Sets `word-break`.
    pub fn word_break(self, value: Specified<WordBreak>) -> Self {
        self.push(ParagraphDeclaration::WordBreak(value))
    }

    /// Sets `overflow-wrap`.
    pub fn overflow_wrap(self, value: Specified<OverflowWrap>) -> Self {
        self.push(ParagraphDeclaration::OverflowWrap(value))
    }

    /// Sets `text-wrap-mode`.
    pub fn text_wrap_mode(self, value: Specified<TextWrapMode>) -> Self {
        self.push(ParagraphDeclaration::TextWrapMode(value))
    }

    /// Resolves this style relative to the provided context.
    pub fn resolve(&self, ctx: ParagraphResolveContext<'_>) -> ComputedParagraphStyle {
        resolve_paragraph_declarations(&self.declarations, ctx)
    }
}
