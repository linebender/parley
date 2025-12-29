// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use super::Language;
use super::specified::Specified;
use super::values::{FontSize, FontStyle, LineHeight, Spacing};
use super::{BaseDirection, BidiControl, FontFamily, FontFeatures, FontVariations, FontWidth};
use super::{FontWeight as FontWeightValue, OverflowWrap, TextWrapMode, WordBreak};

/// A single inline style declaration.
#[derive(Clone, Debug, PartialEq)]
pub enum InlineDeclaration {
    /// CSS `font-family`.
    FontFamily(Specified<FontFamily>),
    /// Font size.
    FontSize(Specified<FontSize>),
    /// Font style.
    FontStyle(Specified<FontStyle>),
    /// Font weight.
    FontWeight(Specified<FontWeightValue>),
    /// Font width / stretch.
    FontWidth(Specified<FontWidth>),
    /// Font variation settings (OpenType axis values).
    FontVariations(Specified<FontVariations>),
    /// Font feature settings (OpenType feature values).
    FontFeatures(Specified<FontFeatures>),
    /// Locale/language tag, if any.
    Locale(Specified<Option<Language>>),
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
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty style with capacity for `capacity` declarations.
    ///
    /// This is useful when building styles programmatically and you have a good idea of how many
    /// declarations you will add.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            declarations: Vec::with_capacity(capacity),
        }
    }

    /// Creates a style from an iterator of declarations.
    ///
    /// This is a convenience for building styles without chaining many setter calls.
    ///
    /// ## Example
    ///
    /// ```
    /// use styled_text::style::{FontSize, InlineDeclaration, InlineStyle, Specified};
    ///
    /// let style = InlineStyle::from_declarations([
    ///     InlineDeclaration::FontSize(Specified::Value(FontSize::Px(16.0))),
    ///     InlineDeclaration::Underline(Specified::Value(true)),
    /// ]);
    /// assert_eq!(style.declarations().len(), 2);
    /// ```
    #[inline]
    pub fn from_declarations<I>(declarations: I) -> Self
    where
        I: IntoIterator<Item = InlineDeclaration>,
    {
        Self {
            declarations: declarations.into_iter().collect(),
        }
    }

    /// Returns the declarations in this style, in authoring order.
    #[inline]
    pub fn declarations(&self) -> &[InlineDeclaration] {
        &self.declarations
    }

    /// Removes all declarations from this style, retaining the allocated storage.
    ///
    /// This is useful for reusing an `InlineStyle` as scratch storage when generating many
    /// computed runs.
    ///
    /// ## Example
    ///
    /// ```
    /// use styled_text::style::{InlineDeclaration, InlineStyle, Specified};
    ///
    /// let mut style = InlineStyle::new().underline(Specified::Value(true));
    /// assert_eq!(style.declarations().len(), 1);
    ///
    /// style.clear();
    /// assert!(style.declarations().is_empty());
    ///
    /// style.push_declaration(InlineDeclaration::Underline(Specified::Value(false)));
    /// assert_eq!(style.declarations().len(), 1);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.declarations.clear();
    }

    /// Appends a declaration to this style.
    #[inline]
    pub fn push_declaration(&mut self, declaration: InlineDeclaration) {
        self.declarations.push(declaration);
    }

    /// Appends an arbitrary declaration.
    #[inline]
    pub fn push(mut self, declaration: InlineDeclaration) -> Self {
        self.declarations.push(declaration);
        self
    }

    /// Sets `font-family`.
    #[inline]
    pub fn font_family(self, value: Specified<FontFamily>) -> Self {
        self.push(InlineDeclaration::FontFamily(value))
    }

    /// Sets `font-family` to a concrete value.
    #[inline]
    pub fn with_font_family(self, value: FontFamily) -> Self {
        self.font_family(Specified::Value(value))
    }

    /// Sets `font-family: inherit`.
    #[inline]
    pub fn inherit_font_family(self) -> Self {
        self.font_family(Specified::Inherit)
    }

    /// Sets `font-family: initial`.
    #[inline]
    pub fn initial_font_family(self) -> Self {
        self.font_family(Specified::Initial)
    }

    /// Sets `font-size`.
    #[inline]
    pub fn font_size(self, value: Specified<FontSize>) -> Self {
        self.push(InlineDeclaration::FontSize(value))
    }

    /// Sets `font-size` to a concrete value.
    #[inline]
    pub fn with_font_size(self, value: FontSize) -> Self {
        self.font_size(Specified::Value(value))
    }

    /// Sets `font-size: inherit`.
    #[inline]
    pub fn inherit_font_size(self) -> Self {
        self.font_size(Specified::Inherit)
    }

    /// Sets `font-size: initial`.
    #[inline]
    pub fn initial_font_size(self) -> Self {
        self.font_size(Specified::Initial)
    }

    /// Sets `font-style`.
    #[inline]
    pub fn font_style(self, value: Specified<FontStyle>) -> Self {
        self.push(InlineDeclaration::FontStyle(value))
    }

    /// Sets `font-style` to a concrete value.
    #[inline]
    pub fn with_font_style(self, value: FontStyle) -> Self {
        self.font_style(Specified::Value(value))
    }

    /// Sets `font-style: inherit`.
    #[inline]
    pub fn inherit_font_style(self) -> Self {
        self.font_style(Specified::Inherit)
    }

    /// Sets `font-style: initial`.
    #[inline]
    pub fn initial_font_style(self) -> Self {
        self.font_style(Specified::Initial)
    }

    /// Sets `font-weight`.
    #[inline]
    pub fn font_weight(self, value: Specified<FontWeightValue>) -> Self {
        self.push(InlineDeclaration::FontWeight(value))
    }

    /// Sets `font-weight` to a concrete value.
    #[inline]
    pub fn with_font_weight(self, value: FontWeightValue) -> Self {
        self.font_weight(Specified::Value(value))
    }

    /// Sets `font-weight: inherit`.
    #[inline]
    pub fn inherit_font_weight(self) -> Self {
        self.font_weight(Specified::Inherit)
    }

    /// Sets `font-weight: initial`.
    #[inline]
    pub fn initial_font_weight(self) -> Self {
        self.font_weight(Specified::Initial)
    }

    /// Sets `font-width` / stretch.
    #[inline]
    pub fn font_width(self, value: Specified<FontWidth>) -> Self {
        self.push(InlineDeclaration::FontWidth(value))
    }

    /// Sets `font-width` to a concrete value.
    #[inline]
    pub fn with_font_width(self, value: FontWidth) -> Self {
        self.font_width(Specified::Value(value))
    }

    /// Sets `font-width: inherit`.
    #[inline]
    pub fn inherit_font_width(self) -> Self {
        self.font_width(Specified::Inherit)
    }

    /// Sets `font-width: initial`.
    #[inline]
    pub fn initial_font_width(self) -> Self {
        self.font_width(Specified::Initial)
    }

    /// Sets font variation settings.
    #[inline]
    pub fn font_variations(self, value: Specified<FontVariations>) -> Self {
        self.push(InlineDeclaration::FontVariations(value))
    }

    /// Sets font variation settings to a concrete value.
    #[inline]
    pub fn with_font_variations(self, value: FontVariations) -> Self {
        self.font_variations(Specified::Value(value))
    }

    /// Sets font variation settings to `inherit`.
    #[inline]
    pub fn inherit_font_variations(self) -> Self {
        self.font_variations(Specified::Inherit)
    }

    /// Sets font variation settings to `initial`.
    #[inline]
    pub fn initial_font_variations(self) -> Self {
        self.font_variations(Specified::Initial)
    }

    /// Sets font feature settings.
    #[inline]
    pub fn font_features(self, value: Specified<FontFeatures>) -> Self {
        self.push(InlineDeclaration::FontFeatures(value))
    }

    /// Sets font feature settings to a concrete value.
    #[inline]
    pub fn with_font_features(self, value: FontFeatures) -> Self {
        self.font_features(Specified::Value(value))
    }

    /// Sets font feature settings to `inherit`.
    #[inline]
    pub fn inherit_font_features(self) -> Self {
        self.font_features(Specified::Inherit)
    }

    /// Sets font feature settings to `initial`.
    #[inline]
    pub fn initial_font_features(self) -> Self {
        self.font_features(Specified::Initial)
    }

    /// Sets `locale` (language tag), if any.
    #[inline]
    pub fn locale(self, value: Specified<Option<Language>>) -> Self {
        self.push(InlineDeclaration::Locale(value))
    }

    /// Sets `locale` (language tag) to a concrete value.
    #[inline]
    pub fn with_locale(self, value: Language) -> Self {
        self.locale(Specified::Value(Some(value)))
    }

    /// Clears `locale` (language tag).
    #[inline]
    pub fn without_locale(self) -> Self {
        self.locale(Specified::Value(None))
    }

    /// Sets `text-decoration-line: underline`.
    #[inline]
    pub fn underline(self, value: Specified<bool>) -> Self {
        self.push(InlineDeclaration::Underline(value))
    }

    /// Sets `text-decoration-line: underline` to a concrete value.
    #[inline]
    pub fn with_underline(self, value: bool) -> Self {
        self.underline(Specified::Value(value))
    }

    /// Sets `text-decoration-line: underline` to `inherit`.
    #[inline]
    pub fn inherit_underline(self) -> Self {
        self.underline(Specified::Inherit)
    }

    /// Sets `text-decoration-line: underline` to `initial`.
    #[inline]
    pub fn initial_underline(self) -> Self {
        self.underline(Specified::Initial)
    }

    /// Sets `text-decoration-line: line-through`.
    #[inline]
    pub fn strikethrough(self, value: Specified<bool>) -> Self {
        self.push(InlineDeclaration::Strikethrough(value))
    }

    /// Sets `text-decoration-line: line-through` to a concrete value.
    #[inline]
    pub fn with_strikethrough(self, value: bool) -> Self {
        self.strikethrough(Specified::Value(value))
    }

    /// Sets `text-decoration-line: line-through` to `inherit`.
    #[inline]
    pub fn inherit_strikethrough(self) -> Self {
        self.strikethrough(Specified::Inherit)
    }

    /// Sets `text-decoration-line: line-through` to `initial`.
    #[inline]
    pub fn initial_strikethrough(self) -> Self {
        self.strikethrough(Specified::Initial)
    }

    /// Sets `line-height`.
    #[inline]
    pub fn line_height(self, value: Specified<LineHeight>) -> Self {
        self.push(InlineDeclaration::LineHeight(value))
    }

    /// Sets `word-spacing`.
    #[inline]
    pub fn word_spacing(self, value: Specified<Spacing>) -> Self {
        self.push(InlineDeclaration::WordSpacing(value))
    }

    /// Sets `letter-spacing`.
    #[inline]
    pub fn letter_spacing(self, value: Specified<Spacing>) -> Self {
        self.push(InlineDeclaration::LetterSpacing(value))
    }

    /// Sets bidi controls for this span.
    #[inline]
    pub fn bidi_control(self, value: Specified<BidiControl>) -> Self {
        self.push(InlineDeclaration::BidiControl(value))
    }
}

/// A single paragraph style declaration.
#[derive(Clone, Debug, PartialEq)]
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
