// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Styled Text Parley adapts [`styled_text`] to Parley's low-level style-run
//! builder.
//! It lowers resolved styled-text segments into Parley's style table and range
//! runs, while reusing scratch storage across layout builds.
//!
//! The crate also provides a Parley-shaped first style vocabulary.
//! [`ParleyLayoutStyle`] holds the fields that can affect shaping and line
//! layout.
//! [`ParleyPaintStyle`] holds paint-only fields such as brushes and decorations.
//! Interning those payloads separately means paint-only changes can share
//! layout identity when the styled text is lowered.
//!
//! This adapter does not own document structure, inline boxes, cascading, or
//! renderer-specific style semantics.
//! Callers can use the provided Parley style payloads for a simple path, or keep
//! their own style types in `styled_text` and use the generic lowering
//! functions.
//!
//! ## Concepts
//!
//! - [`ParleyStyledTextBuilder`] is a [`StyledTextBuilder`] configured with the
//!   default Parley style payloads and patch type.
//! - [`ParleyStyleChange`] is a partial style patch for common Parley fields,
//!   with public fields for less common changes.
//! - [`ParleyStyleRunWorkspace`] keeps the reusable segment workspace and the
//!   temporary [`StyleId`] to Parley style-index map.
//! - [`build_layout_from_parley_styled_text`] creates a Parley [`Layout`] from
//!   text built with the default Parley payloads.
//! - [`push_style_runs`] is the lower-level hook for callers that want to feed
//!   Parley style runs themselves.
//!
//! ## Building a Parley layout
//!
//! ```no_run
//! use parley::{FontContext, FontWeight, LayoutContext};
//! use styled_text_parley::{
//!     ParleyLayoutStyle, ParleyPaintStyle, ParleyStyleChange, ParleyStyleRunWorkspace,
//!     ParleyStyledTextBuilder, build_layout_from_parley_styled_text,
//! };
//!
//! let mut text = ParleyStyledTextBuilder::<()>::new(
//!     ParleyLayoutStyle::default(),
//!     ParleyPaintStyle::default(),
//! );
//! text.push("Hello ");
//! text.push_with(
//!     "styled text",
//!     ParleyStyleChange::default()
//!         .font_size(24.0)
//!         .font_weight(FontWeight::BOLD),
//! );
//! let styled = text.finish();
//!
//! let mut font_cx = FontContext::new();
//! let mut layout_cx = LayoutContext::<()>::new();
//! let mut workspace = ParleyStyleRunWorkspace::new();
//! let mut layout = build_layout_from_parley_styled_text(
//!     &mut layout_cx,
//!     &mut font_cx,
//!     &styled,
//!     &mut workspace,
//!     1.0,
//!     true,
//! ).unwrap();
//! layout.break_all_lines(Some(240.0));
//! ```
//!
//! ## Features
//!
//! - `std` (enabled by default): Enables `std` support in [`parley`] and
//!   [`styled_text`].
//! - `libm`: Enables the `libm` feature of [`parley`].

// LINEBENDER LINT SET - lib.rs - v3
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::{self, Debug};

use parley::{
    Brush, FontContext, FontFamily, FontFeatures, FontStyle, FontVariations, FontWeight, FontWidth,
    Language, Layout, LayoutContext, LineHeight, OverflowWrap, StyleRunBuilder, TextStyle,
    TextWrapMode, WordBreak,
};
use styled_text::{
    SegmentStyle, StyleId, StylePatch, StyledSegmentsWorkspace, StyledText, StyledTextBuilder,
    TextRange, TextStorage,
};

/// Styled text that uses the default Parley style payloads.
pub type ParleyStyledText<T, B> = StyledText<T, ParleyLayoutStyle, ParleyPaintStyle<B>>;

/// Builder for styled text that uses the default Parley style payloads.
pub type ParleyStyledTextBuilder<B> =
    StyledTextBuilder<ParleyLayoutStyle, ParleyPaintStyle<B>, ParleyStyleChange<B>>;

/// Reusable allocation workspace for lowering styled text into Parley style runs.
///
/// Reuse this across layout builds to retain both the styled-segment workspace
/// and the temporary map from [`styled_text::StyleId`] to Parley `u16` style indices.
#[derive(Clone, Debug, Default)]
pub struct ParleyStyleRunWorkspace {
    segments: StyledSegmentsWorkspace,
    style_indices: Vec<u16>,
}

impl ParleyStyleRunWorkspace {
    /// Creates an empty workspace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears retained style-index data while keeping allocations for reuse.
    ///
    /// Segment scratch data is rebuilt the next time the workspace is used.
    pub fn clear(&mut self) {
        self.style_indices.clear();
    }
}

/// Layout-affecting style payload for the default Parley integration.
///
/// These fields are separated from [`ParleyPaintStyle`] so paint-only changes
/// can share layout payloads and avoid invalidating shaping or line layout.
#[derive(Clone, Debug, PartialEq)]
pub struct ParleyLayoutStyle {
    /// CSS `font-family` property value.
    pub font_family: FontFamily<'static>,
    /// Font size.
    pub font_size: f32,
    /// Font width.
    pub font_width: FontWidth,
    /// Font style.
    pub font_style: FontStyle,
    /// Font weight.
    pub font_weight: FontWeight,
    /// Font variation settings.
    pub font_variations: FontVariations<'static>,
    /// Font feature settings.
    pub font_features: FontFeatures<'static>,
    /// Locale.
    pub locale: Option<Language>,
    /// Line height.
    pub line_height: LineHeight,
    /// Extra spacing between words.
    pub word_spacing: f32,
    /// Extra spacing between letters.
    pub letter_spacing: f32,
    /// Control over where words can wrap.
    pub word_break: WordBreak,
    /// Control over emergency line breaking.
    pub overflow_wrap: OverflowWrap,
    /// Control over non-emergency line breaking.
    pub text_wrap_mode: TextWrapMode,
}

impl Default for ParleyLayoutStyle {
    fn default() -> Self {
        let style = TextStyle::<()>::default();
        Self {
            font_family: style.font_family,
            font_size: style.font_size,
            font_width: style.font_width,
            font_style: style.font_style,
            font_weight: style.font_weight,
            font_variations: style.font_variations,
            font_features: style.font_features,
            locale: style.locale,
            line_height: style.line_height,
            word_spacing: style.word_spacing,
            letter_spacing: style.letter_spacing,
            word_break: style.word_break,
            overflow_wrap: style.overflow_wrap,
            text_wrap_mode: style.text_wrap_mode,
        }
    }
}

/// Paint-only style payload for the default Parley integration.
///
/// These fields affect rendered glyphs and decorations, but not shaping or line
/// layout.
#[derive(Clone, Debug, PartialEq)]
pub struct ParleyPaintStyle<B: Brush> {
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
}

impl<B: Brush> ParleyPaintStyle<B> {
    /// Creates a paint style with the given text brush and default decorations.
    #[must_use]
    pub fn new(brush: B) -> Self {
        Self {
            brush,
            ..Self::default()
        }
    }
}

impl<B: Brush> Default for ParleyPaintStyle<B> {
    fn default() -> Self {
        let style = TextStyle::<B>::default();
        Self {
            brush: style.brush,
            has_underline: style.has_underline,
            underline_offset: style.underline_offset,
            underline_size: style.underline_size,
            underline_brush: style.underline_brush,
            has_strikethrough: style.has_strikethrough,
            strikethrough_offset: style.strikethrough_offset,
            strikethrough_size: style.strikethrough_size,
            strikethrough_brush: style.strikethrough_brush,
        }
    }
}

/// Partial style patch for the default Parley style payloads.
///
/// Each `Some` field replaces the corresponding field in the current full
/// style. Callers that need inheritance, cascading, or a smaller domain-specific
/// patch type can continue to implement [`StylePatch`] directly. The fluent
/// methods cover common fields; all fields are public for less common changes.
///
/// Fields whose destination value is itself optional use `Option<Option<T>>`:
/// the outer `None` leaves the current value unchanged, `Some(None)` clears it,
/// and `Some(Some(value))` sets it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParleyStyleChange<B: Brush> {
    /// CSS `font-family` property value.
    pub font_family: Option<FontFamily<'static>>,
    /// Font size.
    pub font_size: Option<f32>,
    /// Font width.
    pub font_width: Option<FontWidth>,
    /// Font style.
    pub font_style: Option<FontStyle>,
    /// Font weight.
    pub font_weight: Option<FontWeight>,
    /// Font variation settings.
    pub font_variations: Option<FontVariations<'static>>,
    /// Font feature settings.
    pub font_features: Option<FontFeatures<'static>>,
    /// Locale.
    ///
    /// The outer option controls whether this patch changes the locale; the
    /// inner option is the resulting locale value.
    pub locale: Option<Option<Language>>,
    /// Brush for rendering text.
    pub brush: Option<B>,
    /// Underline decoration.
    pub underline: Option<bool>,
    /// Offset of the underline decoration.
    ///
    /// The outer option controls whether this patch changes the offset; the
    /// inner option is the resulting offset value.
    pub underline_offset: Option<Option<f32>>,
    /// Size of the underline decoration.
    ///
    /// The outer option controls whether this patch changes the size; the inner
    /// option is the resulting size value.
    pub underline_size: Option<Option<f32>>,
    /// Brush for rendering the underline decoration.
    ///
    /// The outer option controls whether this patch changes the brush; the
    /// inner option is the resulting brush value.
    pub underline_brush: Option<Option<B>>,
    /// Strikethrough decoration.
    pub strikethrough: Option<bool>,
    /// Offset of the strikethrough decoration.
    ///
    /// The outer option controls whether this patch changes the offset; the
    /// inner option is the resulting offset value.
    pub strikethrough_offset: Option<Option<f32>>,
    /// Size of the strikethrough decoration.
    ///
    /// The outer option controls whether this patch changes the size; the inner
    /// option is the resulting size value.
    pub strikethrough_size: Option<Option<f32>>,
    /// Brush for rendering the strikethrough decoration.
    ///
    /// The outer option controls whether this patch changes the brush; the
    /// inner option is the resulting brush value.
    pub strikethrough_brush: Option<Option<B>>,
    /// Line height.
    pub line_height: Option<LineHeight>,
    /// Extra spacing between words.
    pub word_spacing: Option<f32>,
    /// Extra spacing between letters.
    pub letter_spacing: Option<f32>,
    /// Control over where words can wrap.
    pub word_break: Option<WordBreak>,
    /// Control over emergency line breaking.
    pub overflow_wrap: Option<OverflowWrap>,
    /// Control over non-emergency line breaking.
    pub text_wrap_mode: Option<TextWrapMode>,
}

impl<B: Brush> ParleyStyleChange<B> {
    /// Sets font size.
    #[must_use]
    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Sets font weight.
    #[must_use]
    pub fn font_weight(mut self, font_weight: FontWeight) -> Self {
        self.font_weight = Some(font_weight);
        self
    }

    /// Sets underline decoration.
    #[must_use]
    pub fn underline(mut self, enabled: bool) -> Self {
        self.underline = Some(enabled);
        self
    }

    /// Sets strikethrough decoration.
    #[must_use]
    pub fn strikethrough(mut self, enabled: bool) -> Self {
        self.strikethrough = Some(enabled);
        self
    }

    /// Sets letter spacing.
    #[must_use]
    pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = Some(letter_spacing);
        self
    }

    /// Sets the text brush.
    #[must_use]
    pub fn brush(mut self, brush: B) -> Self {
        self.brush = Some(brush);
        self
    }
}

impl<B: Brush> StylePatch<ParleyLayoutStyle, ParleyPaintStyle<B>> for ParleyStyleChange<B> {
    fn apply_to(&self, layout: &mut ParleyLayoutStyle, paint: &mut ParleyPaintStyle<B>) {
        if let Some(font_family) = &self.font_family {
            layout.font_family = font_family.clone();
        }
        if let Some(font_size) = self.font_size {
            layout.font_size = font_size;
        }
        if let Some(font_width) = self.font_width {
            layout.font_width = font_width;
        }
        if let Some(font_style) = self.font_style {
            layout.font_style = font_style;
        }
        if let Some(font_weight) = self.font_weight {
            layout.font_weight = font_weight;
        }
        if let Some(font_variations) = &self.font_variations {
            layout.font_variations = font_variations.clone();
        }
        if let Some(font_features) = &self.font_features {
            layout.font_features = font_features.clone();
        }
        if let Some(locale) = self.locale {
            layout.locale = locale;
        }
        if let Some(brush) = &self.brush {
            paint.brush = brush.clone();
        }
        if let Some(underline) = self.underline {
            paint.has_underline = underline;
        }
        if let Some(underline_offset) = self.underline_offset {
            paint.underline_offset = underline_offset;
        }
        if let Some(underline_size) = self.underline_size {
            paint.underline_size = underline_size;
        }
        if let Some(underline_brush) = &self.underline_brush {
            paint.underline_brush.clone_from(underline_brush);
        }
        if let Some(strikethrough) = self.strikethrough {
            paint.has_strikethrough = strikethrough;
        }
        if let Some(strikethrough_offset) = self.strikethrough_offset {
            paint.strikethrough_offset = strikethrough_offset;
        }
        if let Some(strikethrough_size) = self.strikethrough_size {
            paint.strikethrough_size = strikethrough_size;
        }
        if let Some(strikethrough_brush) = &self.strikethrough_brush {
            paint.strikethrough_brush.clone_from(strikethrough_brush);
        }
        if let Some(line_height) = self.line_height {
            layout.line_height = line_height;
        }
        if let Some(word_spacing) = self.word_spacing {
            layout.word_spacing = word_spacing;
        }
        if let Some(letter_spacing) = self.letter_spacing {
            layout.letter_spacing = letter_spacing;
        }
        if let Some(word_break) = self.word_break {
            layout.word_break = word_break;
        }
        if let Some(overflow_wrap) = self.overflow_wrap {
            layout.overflow_wrap = overflow_wrap;
        }
        if let Some(text_wrap_mode) = self.text_wrap_mode {
            layout.text_wrap_mode = text_wrap_mode;
        }
    }
}

/// Error returned when styled text cannot be lowered to Parley.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The styled text is not backed by one contiguous string.
    ///
    /// Parley's current style-run builder accepts a single `&str`; chunked text
    /// storage should be flattened or handled by a future chunk-aware adapter.
    NonContiguousText,
    /// The styled text style table is too large for Parley's `u16` style
    /// indices.
    TooManyStyles {
        /// Number of interned styled-text styles.
        count: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonContiguousText => {
                f.write_str("styled text is not backed by one contiguous string")
            }
            Self::TooManyStyles { count } => {
                write!(
                    f,
                    "styled text has {count} styles, but Parley supports at most \
                     {MAX_PARLEY_STYLES}"
                )
            }
        }
    }
}

impl core::error::Error for Error {}

/// Pushes resolved styled-text segments into an existing Parley style-run builder.
///
/// The `push_style` callback receives the Parley builder and each interned full
/// styled-text style in [`styled_text::StyleId`] table order. It must push a
/// Parley style and return the style-table index produced by
/// [`StyleRunBuilder::push_style`].
///
/// This pushes every interned styled-text style, including styles not currently
/// used by any resolved segment. That preserves the simple table-order lowering
/// contract without allocating a filtered style table. The temporary
/// styled-text-to-Parley style-index map is stored in `workspace` and reused
/// across calls.
pub fn push_style_runs<T, L, P, B, F>(
    builder: &mut StyleRunBuilder<'_, B>,
    styled: &StyledText<T, L, P>,
    workspace: &mut ParleyStyleRunWorkspace,
    mut push_style: F,
) -> Result<(), Error>
where
    T: Debug + TextStorage,
    B: Brush,
    F: FnMut(&mut StyleRunBuilder<'_, B>, SegmentStyle<'_, L, P>) -> u16,
{
    let style_count = styled.style_set().style_len();
    workspace.style_indices.clear();
    check_style_count(style_count)?;
    let max_runs = styled.style_spans_len().saturating_mul(2).saturating_add(1);
    builder.reserve(style_count, max_runs);
    workspace.style_indices.reserve(style_count);

    for style_id in styled.style_set().style_ids() {
        let style = styled.style_set().segment_style(style_id);
        workspace.style_indices.push(push_style(builder, style));
    }

    let mut pending: Option<(TextRange, StyleId)> = None;
    for segment in workspace.segments.segments(styled) {
        let range = segment.range();
        let style = segment.style();
        match pending.take() {
            Some((pending_range, pending_style))
                if pending_style == style && pending_range.end() == range.start() =>
            {
                pending = Some((
                    TextRange::new_unchecked(pending_range.start(), range.end()),
                    pending_style,
                ));
            }
            Some((pending_range, pending_style)) => {
                let style_index = workspace.style_indices[pending_style.index()];
                builder.push_style_run(style_index, pending_range.as_range());
                pending = Some((range, style));
            }
            None => {
                pending = Some((range, style));
            }
        }
    }
    if let Some((range, style)) = pending {
        let style_index = workspace.style_indices[style.index()];
        builder.push_style_run(style_index, range.as_range());
    }

    Ok(())
}

/// Pushes one default Parley style payload into a Parley style-run builder.
///
/// This is the [`push_style_runs`] callback for [`ParleyStyledText`].
pub fn push_parley_style<B: Brush>(
    builder: &mut StyleRunBuilder<'_, B>,
    style: SegmentStyle<'_, ParleyLayoutStyle, ParleyPaintStyle<B>>,
) -> u16 {
    let layout = style.layout();
    let paint = style.paint();
    builder.push_style(TextStyle {
        font_family: layout.font_family.clone(),
        font_size: layout.font_size,
        font_width: layout.font_width,
        font_style: layout.font_style,
        font_weight: layout.font_weight,
        font_variations: layout.font_variations.clone(),
        font_features: layout.font_features.clone(),
        locale: layout.locale,
        brush: paint.brush.clone(),
        has_underline: paint.has_underline,
        underline_offset: paint.underline_offset,
        underline_size: paint.underline_size,
        underline_brush: paint.underline_brush.clone(),
        has_strikethrough: paint.has_strikethrough,
        strikethrough_offset: paint.strikethrough_offset,
        strikethrough_size: paint.strikethrough_size,
        strikethrough_brush: paint.strikethrough_brush.clone(),
        line_height: layout.line_height,
        word_spacing: layout.word_spacing,
        letter_spacing: layout.letter_spacing,
        word_break: layout.word_break,
        overflow_wrap: layout.overflow_wrap,
        text_wrap_mode: layout.text_wrap_mode,
    })
}

const MAX_PARLEY_STYLES: usize = u16::MAX as usize + 1;

fn check_style_count(count: usize) -> Result<(), Error> {
    if count > MAX_PARLEY_STYLES {
        return Err(Error::TooManyStyles { count });
    }
    Ok(())
}

/// Builds a Parley layout from styled text backed by a contiguous string.
///
/// The callback has the same contract as [`push_style_runs`]. This helper is
/// intentionally thin so callers remain in control of how their interned style
/// payloads become Parley [`parley::TextStyle`] values.
pub fn build_layout_from_styled_text<T, L, P, B, F>(
    layout_cx: &mut LayoutContext<B>,
    font_cx: &mut FontContext,
    styled: &StyledText<T, L, P>,
    workspace: &mut ParleyStyleRunWorkspace,
    scale: f32,
    quantize: bool,
    push_style: F,
) -> Result<Layout<B>, Error>
where
    T: Debug + TextStorage,
    B: Brush,
    F: FnMut(&mut StyleRunBuilder<'_, B>, SegmentStyle<'_, L, P>) -> u16,
{
    let text = styled.as_str().ok_or(Error::NonContiguousText)?;
    let mut builder = layout_cx.style_run_builder(font_cx, text, scale, quantize);
    push_style_runs(&mut builder, styled, workspace, push_style)?;
    Ok(builder.build(text))
}

/// Builds a Parley layout from styled text using the default Parley style
/// payloads.
pub fn build_layout_from_parley_styled_text<T, B>(
    layout_cx: &mut LayoutContext<B>,
    font_cx: &mut FontContext,
    styled: &ParleyStyledText<T, B>,
    workspace: &mut ParleyStyleRunWorkspace,
    scale: f32,
    quantize: bool,
) -> Result<Layout<B>, Error>
where
    T: Debug + TextStorage,
    B: Brush,
{
    build_layout_from_styled_text(
        layout_cx,
        font_cx,
        styled,
        workspace,
        scale,
        quantize,
        push_parley_style,
    )
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use alloc::vec;
    use alloc::vec::Vec;

    use parley::{FontWeight, TextStyle};
    use styled_text::{StyleSetBuilder, StyledSegmentsWorkspace, StyledText};

    use super::{
        Error, MAX_PARLEY_STYLES, ParleyLayoutStyle, ParleyPaintStyle, ParleyStyleChange,
        ParleyStyleRunWorkspace, ParleyStyledTextBuilder, build_layout_from_parley_styled_text,
        check_style_count, push_style_runs,
    };

    #[test]
    fn generic_lowering_accepts_custom_style_payloads() {
        let mut style_builder = StyleSetBuilder::<u8, ()>::new();
        let base = style_builder.intern_style(12, ());
        let large = style_builder.intern_style(24, ());
        let styles = Arc::new(style_builder.finish());

        let mut styled = StyledText::new("abcd", styles, base);
        styled
            .apply_style_bytes(1..2, large)
            .expect("valid style range");
        styled
            .apply_style_bytes(2..3, large)
            .expect("valid style range");

        let mut font_cx = parley::FontContext::new();
        let mut layout_cx = parley::LayoutContext::<()>::new();
        let mut builder = layout_cx.style_run_builder(&mut font_cx, "abcd", 1.0, false);
        let mut workspace = ParleyStyleRunWorkspace::new();

        let mut pushed_font_sizes = Vec::new();
        push_style_runs(&mut builder, &styled, &mut workspace, |builder, style| {
            let font_size = f32::from(*style.layout());
            pushed_font_sizes.push(font_size);
            let parley_style = TextStyle::<()> {
                font_size,
                ..TextStyle::default()
            };
            builder.push_style(parley_style)
        })
        .expect("style count fits Parley");

        let _layout = builder.build("abcd");
        assert_eq!(pushed_font_sizes, vec![12.0, 24.0]);
    }

    #[test]
    fn rejects_style_tables_larger_than_parley_can_index() {
        let count = MAX_PARLEY_STYLES + 1;
        assert_eq!(
            check_style_count(count),
            Err(Error::TooManyStyles { count })
        );
    }

    #[test]
    fn parley_style_change_resolves_default_payloads_independently() {
        let mut builder = ParleyStyledTextBuilder::<[u8; 4]>::new(
            ParleyLayoutStyle::default(),
            ParleyPaintStyle::new([0, 0, 0, 255]),
        );
        let all = builder.push("abcd");
        builder.apply(
            all,
            ParleyStyleChange::default()
                .font_size(24.0)
                .font_weight(FontWeight::BOLD),
        );
        builder
            .apply_bytes(1..3, ParleyStyleChange::default().brush([255, 0, 0, 255]))
            .expect("valid range");

        let styled = builder.finish();
        assert_eq!(styled.style_set().style_len(), 3);
        assert_eq!(styled.style_set().layout_len(), 2);
        assert_eq!(styled.style_set().paint_len(), 2);

        let mut workspace = StyledSegmentsWorkspace::new();
        let segments = workspace.segments(&styled).collect::<Vec<_>>();
        assert_eq!(segments.len(), 3);

        let first = styled
            .style_set()
            .get_style(segments[0].style())
            .expect("segment style is interned");
        let second = styled
            .style_set()
            .get_style(segments[1].style())
            .expect("segment style is interned");
        assert_eq!(first.layout_id(), second.layout_id());
        assert_ne!(first.paint_id(), second.paint_id());

        let style = styled.style_set().segment_style(segments[1].style());
        assert_eq!(style.layout().font_size, 24.0);
        assert_eq!(style.layout().font_weight, FontWeight::BOLD);
        assert_eq!(style.paint().brush, [255, 0, 0, 255]);
    }

    #[test]
    fn builds_layout_from_default_parley_payloads() {
        let mut builder = ParleyStyledTextBuilder::<()>::new(
            ParleyLayoutStyle::default(),
            ParleyPaintStyle::default(),
        );
        builder.push_with("abcd", ParleyStyleChange::default().font_size(24.0));
        let styled = builder.finish();

        let mut font_cx = parley::FontContext::new();
        let mut layout_cx = parley::LayoutContext::<()>::new();
        let mut workspace = ParleyStyleRunWorkspace::new();
        let layout = build_layout_from_parley_styled_text(
            &mut layout_cx,
            &mut font_cx,
            &styled,
            &mut workspace,
            1.0,
            false,
        )
        .expect("string storage is contiguous");

        assert_eq!(layout.styles().len(), 2);
    }
}
