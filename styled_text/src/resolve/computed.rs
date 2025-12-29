// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;

use crate::style::Language;
use crate::style::{
    BaseDirection, BidiControl, FontFamily, FontFeature, FontStyle, FontVariation, FontWeight,
    FontWidth, OverflowWrap, TextWrapMode, WordBreak,
};

/// A computed (resolved) line height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComputedLineHeight {
    /// A multiple of the font's preferred line height (metrics-based).
    MetricsRelative(f32),
    /// A multiple of the font size.
    FontSizeRelative(f32),
    /// An absolute value in CSS pixels.
    Px(f32),
}

impl Default for ComputedLineHeight {
    fn default() -> Self {
        Self::MetricsRelative(1.0)
    }
}

/// A computed (resolved) inline style.
///
/// This type is intentionally opaque: it can gain new properties over time without breaking
/// downstream code.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedInlineStyle {
    pub(crate) font_family: FontFamily,
    pub(crate) font_size_px: f32,
    pub(crate) font_width: FontWidth,
    pub(crate) font_style: FontStyle,
    pub(crate) font_weight: FontWeight,
    pub(crate) font_variations: Arc<[FontVariation]>,
    pub(crate) font_features: Arc<[FontFeature]>,
    pub(crate) locale: Option<Language>,
    pub(crate) underline: bool,
    pub(crate) strikethrough: bool,
    pub(crate) line_height: ComputedLineHeight,
    pub(crate) word_spacing_px: f32,
    pub(crate) letter_spacing_px: f32,
    pub(crate) bidi_control: BidiControl,
}

impl Default for ComputedInlineStyle {
    fn default() -> Self {
        Self {
            font_family: crate::style::GenericFamily::SansSerif.into(),
            font_size_px: 16.0,
            font_width: FontWidth::NORMAL,
            font_style: FontStyle::default(),
            font_weight: FontWeight::NORMAL,
            font_variations: Arc::from([]),
            font_features: Arc::from([]),
            locale: None,
            underline: false,
            strikethrough: false,
            line_height: ComputedLineHeight::default(),
            word_spacing_px: 0.0,
            letter_spacing_px: 0.0,
            bidi_control: BidiControl::default(),
        }
    }
}

impl ComputedInlineStyle {
    /// Returns the computed `font-family` value.
    #[inline]
    pub const fn font_family(&self) -> &FontFamily {
        &self.font_family
    }

    /// Returns the computed font size in CSS pixels.
    #[inline]
    pub const fn font_size_px(&self) -> f32 {
        self.font_size_px
    }

    /// Returns the computed font width / stretch.
    #[inline]
    pub const fn font_width(&self) -> FontWidth {
        self.font_width
    }

    /// Returns a new style with `font-size` set to `px`.
    #[inline]
    pub fn with_font_size_px(mut self, px: f32) -> Self {
        self.font_size_px = px;
        self
    }

    /// Returns the computed font style.
    #[inline]
    pub const fn font_style(&self) -> FontStyle {
        self.font_style
    }

    /// Returns the computed font weight.
    #[inline]
    pub const fn font_weight(&self) -> FontWeight {
        self.font_weight
    }

    /// Returns computed font variation settings (OpenType axis values).
    #[inline]
    pub fn font_variations(&self) -> &[FontVariation] {
        &self.font_variations
    }

    /// Returns computed font feature settings (OpenType feature values).
    #[inline]
    pub fn font_features(&self) -> &[FontFeature] {
        &self.font_features
    }

    /// Returns the locale/language tag, if any.
    #[inline]
    pub fn locale(&self) -> Option<&Language> {
        self.locale.as_ref()
    }

    /// Returns whether underline is enabled.
    #[inline]
    pub const fn underline(&self) -> bool {
        self.underline
    }

    /// Returns whether strikethrough is enabled.
    #[inline]
    pub const fn strikethrough(&self) -> bool {
        self.strikethrough
    }

    /// Returns the computed line height.
    #[inline]
    pub const fn line_height(&self) -> ComputedLineHeight {
        self.line_height
    }

    /// Returns computed extra spacing between words in CSS pixels.
    #[inline]
    pub const fn word_spacing_px(&self) -> f32 {
        self.word_spacing_px
    }

    /// Returns computed extra spacing between letters in CSS pixels.
    #[inline]
    pub const fn letter_spacing_px(&self) -> f32 {
        self.letter_spacing_px
    }

    /// Returns the computed inline bidi control.
    #[inline]
    pub const fn bidi_control(&self) -> BidiControl {
        self.bidi_control
    }
}

/// A computed (resolved) paragraph style.
///
/// This type is intentionally opaque: it can gain new properties over time without breaking
/// downstream code.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ComputedParagraphStyle {
    pub(crate) base_direction: BaseDirection,
    pub(crate) word_break: WordBreak,
    pub(crate) overflow_wrap: OverflowWrap,
    pub(crate) text_wrap_mode: TextWrapMode,
}

impl ComputedParagraphStyle {
    /// Returns the paragraph base direction.
    #[inline]
    pub const fn base_direction(&self) -> BaseDirection {
        self.base_direction
    }

    /// Returns `word-break`.
    #[inline]
    pub const fn word_break(&self) -> WordBreak {
        self.word_break
    }

    /// Returns `overflow-wrap`.
    #[inline]
    pub const fn overflow_wrap(&self) -> OverflowWrap {
        self.overflow_wrap
    }

    /// Returns `text-wrap-mode`.
    #[inline]
    pub const fn text_wrap_mode(&self) -> TextWrapMode {
        self.text_wrap_mode
    }
}
