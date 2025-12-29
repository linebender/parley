// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use super::declarations::ParagraphDeclaration;
use super::specified::Specified;

pub use text_primitives::{BaseDirection, OverflowWrap, TextWrapMode, WordBreak};

/// A set of specified paragraph declarations for a block of text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParagraphStyle {
    declarations: Vec<ParagraphDeclaration>,
}

impl ParagraphStyle {
    /// Creates an empty paragraph style (no declarations).
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the declarations in this style, in authoring order.
    #[inline]
    pub fn declarations(&self) -> &[ParagraphDeclaration] {
        &self.declarations
    }

    /// Removes all declarations from this style, retaining the allocated storage.
    #[inline]
    pub fn clear(&mut self) {
        self.declarations.clear();
    }

    /// Appends an arbitrary declaration.
    #[inline]
    pub fn push(mut self, declaration: ParagraphDeclaration) -> Self {
        self.declarations.push(declaration);
        self
    }

    /// Sets the base direction.
    #[inline]
    pub fn base_direction(self, value: Specified<BaseDirection>) -> Self {
        self.push(ParagraphDeclaration::BaseDirection(value))
    }

    /// Sets `word-break`.
    #[inline]
    pub fn word_break(self, value: Specified<WordBreak>) -> Self {
        self.push(ParagraphDeclaration::WordBreak(value))
    }

    /// Sets `overflow-wrap`.
    #[inline]
    pub fn overflow_wrap(self, value: Specified<OverflowWrap>) -> Self {
        self.push(ParagraphDeclaration::OverflowWrap(value))
    }

    /// Sets `text-wrap-mode`.
    #[inline]
    pub fn text_wrap_mode(self, value: Specified<TextWrapMode>) -> Self {
        self.push(ParagraphDeclaration::TextWrapMode(value))
    }
}
