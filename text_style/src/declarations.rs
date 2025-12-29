// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::FontWeight as FontWeightValue;
use crate::InlineResolveContext;
use crate::ResolveStyleError;
use crate::bidi::BidiControl;
use crate::font::FontStack;
use crate::resolve::resolve_inline_declarations;
use crate::specified::Specified;
use crate::values::{FontSize, FontStyle, LineHeight, Spacing};
use crate::{
    BaseDirection, ComputedInlineStyle, ComputedParagraphStyle, OverflowWrap, TextWrapMode,
    WordBreak,
};
use crate::{FontWidth, Settings};

/// A single inline style declaration.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum InlineDeclaration {
    /// Font family stack.
    FontStack(Specified<FontStack>),
    /// Font size.
    FontSize(Specified<FontSize>),
    /// Font style.
    FontStyle(Specified<FontStyle>),
    /// Font weight.
    FontWeight(Specified<FontWeightValue>),
    /// Font width / stretch.
    FontWidth(Specified<FontWidth>),
    /// Font variation settings (OpenType axis values).
    FontVariations(Specified<Settings<f32>>),
    /// Font feature settings (OpenType feature values).
    FontFeatures(Specified<Settings<u16>>),
    /// Locale/language tag, if any.
    Locale(Specified<Option<Arc<str>>>),
    /// Underline decoration.
    Underline(Specified<bool>),
    /// Strikethrough decoration.
    Strikethrough(Specified<bool>),
    /// Line height.
    LineHeight(Specified<LineHeight>),
    /// Extra spacing between words.
    WordSpacing(Specified<Spacing>),
    /// Extra spacing between letters.
    LetterSpacing(Specified<Spacing>),
    /// Inline bidi control.
    BidiControl(Specified<BidiControl>),
}

/// A set of specified inline declarations for a span.
///
/// This is a declaration list (not a “one field per property” struct). When multiple declarations
/// of the same property are present, the last declaration wins.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InlineStyle {
    declarations: Vec<InlineDeclaration>,
}

impl InlineStyle {
    /// Creates an empty style (no declarations).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a style from an iterator of declarations.
    ///
    /// This is a convenience for building styles without chaining many setter calls.
    ///
    /// ## Example
    ///
    /// ```
    /// use text_style::{FontSize, InlineDeclaration, InlineStyle, Specified};
    ///
    /// let style = InlineStyle::from_declarations([
    ///     InlineDeclaration::FontSize(Specified::Value(FontSize::Px(16.0))),
    ///     InlineDeclaration::Underline(Specified::Value(true)),
    /// ]);
    /// assert_eq!(style.declarations().len(), 2);
    /// ```
    pub fn from_declarations<I>(declarations: I) -> Self
    where
        I: IntoIterator<Item = InlineDeclaration>,
    {
        Self {
            declarations: declarations.into_iter().collect(),
        }
    }

    /// Returns the declarations in this style, in authoring order.
    pub fn declarations(&self) -> &[InlineDeclaration] {
        &self.declarations
    }

    /// Appends a declaration to this style.
    pub fn push_declaration(&mut self, declaration: InlineDeclaration) {
        self.declarations.push(declaration);
    }

    /// Appends an arbitrary declaration.
    pub fn push(mut self, declaration: InlineDeclaration) -> Self {
        self.declarations.push(declaration);
        self
    }

    /// Sets `font-family` / font stack.
    pub fn font_stack(self, value: Specified<FontStack>) -> Self {
        self.push(InlineDeclaration::FontStack(value))
    }

    /// Sets `font-size`.
    pub fn font_size(self, value: Specified<FontSize>) -> Self {
        self.push(InlineDeclaration::FontSize(value))
    }

    /// Sets `font-style`.
    pub fn font_style(self, value: Specified<FontStyle>) -> Self {
        self.push(InlineDeclaration::FontStyle(value))
    }

    /// Sets `font-weight`.
    pub fn font_weight(self, value: Specified<FontWeightValue>) -> Self {
        self.push(InlineDeclaration::FontWeight(value))
    }

    /// Sets `font-width` / stretch.
    pub fn font_width(self, value: Specified<FontWidth>) -> Self {
        self.push(InlineDeclaration::FontWidth(value))
    }

    /// Sets font variation settings.
    pub fn font_variations(self, value: Specified<Settings<f32>>) -> Self {
        self.push(InlineDeclaration::FontVariations(value))
    }

    /// Sets font feature settings.
    pub fn font_features(self, value: Specified<Settings<u16>>) -> Self {
        self.push(InlineDeclaration::FontFeatures(value))
    }

    /// Sets `locale` (language tag), if any.
    pub fn locale(self, value: Specified<Option<Arc<str>>>) -> Self {
        self.push(InlineDeclaration::Locale(value))
    }

    /// Sets `text-decoration-line: underline`.
    pub fn underline(self, value: Specified<bool>) -> Self {
        self.push(InlineDeclaration::Underline(value))
    }

    /// Sets `text-decoration-line: line-through`.
    pub fn strikethrough(self, value: Specified<bool>) -> Self {
        self.push(InlineDeclaration::Strikethrough(value))
    }

    /// Sets `line-height`.
    pub fn line_height(self, value: Specified<LineHeight>) -> Self {
        self.push(InlineDeclaration::LineHeight(value))
    }

    /// Sets `word-spacing`.
    pub fn word_spacing(self, value: Specified<Spacing>) -> Self {
        self.push(InlineDeclaration::WordSpacing(value))
    }

    /// Sets `letter-spacing`.
    pub fn letter_spacing(self, value: Specified<Spacing>) -> Self {
        self.push(InlineDeclaration::LetterSpacing(value))
    }

    /// Sets bidi controls for this span.
    pub fn bidi_control(self, value: Specified<BidiControl>) -> Self {
        self.push(InlineDeclaration::BidiControl(value))
    }

    /// Resolves this style relative to the provided context.
    ///
    /// This can fail if any declarations require parsing and the provided values are invalid (for
    /// example OpenType settings supplied as [`Settings::Source`]).
    pub fn resolve(
        &self,
        ctx: InlineResolveContext<'_>,
    ) -> Result<ComputedInlineStyle, ResolveStyleError> {
        resolve_inline_declarations(&self.declarations, ctx)
    }
}

/// A single paragraph style declaration.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ParagraphDeclaration {
    /// The paragraph's base direction.
    BaseDirection(Specified<BaseDirection>),
    /// Control over where words can wrap.
    WordBreak(Specified<WordBreak>),
    /// Control over "emergency" line-breaking.
    OverflowWrap(Specified<OverflowWrap>),
    /// Control over non-"emergency" line-breaking.
    TextWrapMode(Specified<TextWrapMode>),
}

// Keep these `use`s alive until the corresponding resolve code is factored into their own modules.
#[expect(unused_imports, reason = "Used by public types in this module.")]
use ComputedParagraphStyle as _;
