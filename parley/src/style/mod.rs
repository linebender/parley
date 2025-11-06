// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rich styling support.

mod brush;
mod font;
mod styleset;

use alloc::borrow::Cow;

pub use brush::*;
pub use font::{
    FontFamily, FontFeature, FontSettings, FontStack, FontStyle, FontVariation, FontWeight,
    FontWidth, GenericFamily,
};
pub use icu_segmenter::options::LineBreakWordOption;
pub use styleset::StyleSet;

use crate::util::nearly_eq;

#[derive(Debug, Clone, Copy)]
pub enum WhiteSpaceCollapse {
    Collapse,
    Preserve,
}

/// Control over "emergency" line-breaking.
///
/// See <https://drafts.csswg.org/css-text/#overflow-wrap-property> for more information.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum OverflowWrap {
    /// Even with extremely long words, lines can only break at places specified in
    /// [`LineBreakWordOption`].
    #[default]
    Normal,
    /// Words may be broken at an arbitrary point if there are no other places in the line to break
    /// them.
    Anywhere,
    /// Like [`OverflowWrap::Anywhere`], except arbitrary wrapping opportunities are not considered
    /// when calculating the minimum content width (see [`crate::Layout::calculate_content_widths`]).
    BreakWord,
}

/// The height that this text takes up. The default is `MetricsRelative(1.0)`, which is the given
/// font's preferred line height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    /// The line's height is a multiple of the "line height" defined by the font's metrics--the sum
    /// of the ascender height, descender height, and line gap / leading.
    MetricsRelative(f32),
    /// Line height specified as a multiple of the font size. This is how the CSS `line-height`
    /// property behaves if given a unitless number. Useful if you're using system-defined generic
    /// font families and want the line heights to be consistent across platforms.
    FontSizeRelative(f32),
    /// Line height specified in absolute units. This can be useful for ensuring all lines are
    /// spaced a whole number of pixels apart, or fitting lines into a given layout container
    /// height.
    Absolute(f32),
}

impl Default for LineHeight {
    fn default() -> Self {
        Self::MetricsRelative(1.0)
    }
}

impl LineHeight {
    pub(crate) fn nearly_eq(self, other: Self) -> bool {
        match (self, other) {
            (Self::MetricsRelative(a), Self::MetricsRelative(b))
            | (Self::FontSizeRelative(a), Self::FontSizeRelative(b))
            | (Self::Absolute(a), Self::Absolute(b)) => nearly_eq(a, b),
            _ => false,
        }
    }

    pub(crate) fn scale(self, scale: f32) -> Self {
        match self {
            Self::Absolute(value) => Self::Absolute(value * scale),
            // The other variants are relative to the font size, so scaling here needn't do anything
            value => value,
        }
    }
}

/// Properties that define a style.
#[derive(Clone, PartialEq, Debug)]
pub enum StyleProperty<'a, B: Brush> {
    /// Font family stack.
    FontStack(FontStack<'a>),
    /// Font size.
    FontSize(f32),
    /// Font width.
    FontWidth(FontWidth),
    /// Font style.
    FontStyle(FontStyle),
    /// Font weight.
    FontWeight(FontWeight),
    /// Font variation settings.
    FontVariations(FontSettings<'a, FontVariation>),
    /// Font feature settings.
    FontFeatures(FontSettings<'a, FontFeature>),
    /// Locale.
    Locale(Option<&'a str>),
    /// Brush for rendering text.
    Brush(B),
    /// Underline decoration.
    Underline(bool),
    /// Offset of the underline decoration.
    UnderlineOffset(Option<f32>),
    /// Size of the underline decoration.
    UnderlineSize(Option<f32>),
    /// Brush for rendering the underline decoration.
    UnderlineBrush(Option<B>),
    /// Strikethrough decoration.
    Strikethrough(bool),
    /// Offset of the strikethrough decoration.
    StrikethroughOffset(Option<f32>),
    /// Size of the strikethrough decoration.
    StrikethroughSize(Option<f32>),
    /// Brush for rendering the strikethrough decoration.
    StrikethroughBrush(Option<B>),
    /// Line height.
    LineHeight(LineHeight),
    /// Extra spacing between words.
    WordSpacing(f32),
    /// Extra spacing between letters.
    LetterSpacing(f32),
    /// Control over where words can wrap.
    WordBreak(LineBreakWordOption),
    /// Control over "emergency" line-breaking.
    OverflowWrap(OverflowWrap),
}

/// Unresolved styles.
#[derive(Clone, PartialEq, Debug)]
pub struct TextStyle<'a, B: Brush> {
    /// Font family stack.
    pub font_stack: FontStack<'a>,
    /// Font size.
    pub font_size: f32,
    /// Font width.
    pub font_width: FontWidth,
    /// Font style.
    pub font_style: FontStyle,
    /// Font weight.
    pub font_weight: FontWeight,
    /// Font variation settings.
    pub font_variations: FontSettings<'a, FontVariation>,
    /// Font feature settings.
    pub font_features: FontSettings<'a, FontFeature>,
    /// Locale.
    pub locale: Option<&'a str>,
    /// Brush for rendering text.
    pub brush: B,
    /// Underline decoration.
    pub has_underline: bool,
    /// Offset of the underline decoration.
    pub underline_offset: Option<f32>,
    /// Size of the underline decoration.
    pub underline_size: Option<f32>,
    /// Brush for rendering the underline decoration.
    pub underline_brush: Option<B>,
    /// Strikethrough decoration.
    pub has_strikethrough: bool,
    /// Offset of the strikethrough decoration.
    pub strikethrough_offset: Option<f32>,
    /// Size of the strikethrough decoration.
    pub strikethrough_size: Option<f32>,
    /// Brush for rendering the strikethrough decoration.
    pub strikethrough_brush: Option<B>,
    /// Line height.
    pub line_height: LineHeight,
    /// Extra spacing between words.
    pub word_spacing: f32,
    /// Extra spacing between letters.
    pub letter_spacing: f32,
    /// Control over where words can wrap.
    pub word_break: LineBreakWordOption,
    /// Control over "emergency" line-breaking.
    pub overflow_wrap: OverflowWrap,
}

impl<B: Brush> Default for TextStyle<'_, B> {
    fn default() -> Self {
        TextStyle {
            font_stack: FontStack::Source(Cow::Borrowed("sans-serif")),
            font_size: 16.0,
            font_width: FontWidth::default(),
            font_style: FontStyle::default(),
            font_weight: FontWeight::default(),
            font_variations: FontSettings::List(Cow::Borrowed(&[])),
            font_features: FontSettings::List(Cow::Borrowed(&[])),
            locale: None,
            brush: B::default(),
            has_underline: false,
            underline_offset: None,
            underline_size: None,
            underline_brush: None,
            has_strikethrough: false,
            strikethrough_offset: None,
            strikethrough_size: None,
            strikethrough_brush: None,
            line_height: LineHeight::default(),
            word_spacing: 0.0,
            letter_spacing: 0.0,
            word_break: LineBreakWordOption::default(),
            overflow_wrap: OverflowWrap::default(),
        }
    }
}

impl<'a, B: Brush> From<FontStack<'a>> for StyleProperty<'a, B> {
    fn from(fs: FontStack<'a>) -> Self {
        StyleProperty::FontStack(fs)
    }
}

impl<'a, B: Brush> From<&'a [FontFamily<'a>]> for StyleProperty<'a, B> {
    fn from(fs: &'a [FontFamily<'a>]) -> Self {
        StyleProperty::FontStack(fs.into())
    }
}

impl<'a, B: Brush> From<FontFamily<'a>> for StyleProperty<'a, B> {
    fn from(f: FontFamily<'a>) -> Self {
        StyleProperty::FontStack(FontStack::from(f))
    }
}

impl<B: Brush> From<GenericFamily> for StyleProperty<'_, B> {
    fn from(f: GenericFamily) -> Self {
        StyleProperty::FontStack(f.into())
    }
}

impl<B: Brush> From<LineHeight> for StyleProperty<'_, B> {
    fn from(value: LineHeight) -> Self {
        StyleProperty::LineHeight(value)
    }
}
