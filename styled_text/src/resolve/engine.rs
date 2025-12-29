// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;

use super::{ComputedInlineStyle, ComputedLineHeight, ComputedParagraphStyle};
use super::{InlineResolveContext, ParagraphResolveContext};
use crate::style::{
    BaseDirection, BidiControl, FontFamily, FontFeatures, FontSize, FontStyle, FontVariations,
    FontWeight, FontWidth, InlineDeclaration, Language, LineHeight, OverflowWrap,
    ParagraphDeclaration, Spacing, Specified, TextWrapMode, WordBreak,
};
use crate::style::{FontFeature, FontVariation};

/// Resolves a list of inline declarations into a computed inline style.
///
/// The input is treated as a declaration list: if multiple declarations of the same property are
/// present, the last declaration wins.
pub fn resolve_inline_declarations<'a, I>(
    declarations: I,
    ctx: InlineResolveContext<'_>,
) -> ComputedInlineStyle
where
    I: IntoIterator<Item = &'a InlineDeclaration>,
{
    let mut font_family: Option<&Specified<FontFamily>> = None;
    let mut font_size: Option<&Specified<FontSize>> = None;
    let mut font_style: Option<&Specified<FontStyle>> = None;
    let mut font_weight: Option<&Specified<FontWeight>> = None;
    let mut font_width: Option<&Specified<FontWidth>> = None;
    let mut font_variations: Option<&Specified<FontVariations>> = None;
    let mut font_features: Option<&Specified<FontFeatures>> = None;
    let mut locale: Option<&Specified<Option<Language>>> = None;
    let mut underline: Option<&Specified<bool>> = None;
    let mut strikethrough: Option<&Specified<bool>> = None;
    let mut line_height: Option<&Specified<LineHeight>> = None;
    let mut word_spacing: Option<&Specified<Spacing>> = None;
    let mut letter_spacing: Option<&Specified<Spacing>> = None;
    let mut bidi_control: Option<&Specified<BidiControl>> = None;

    for decl in declarations {
        match decl {
            InlineDeclaration::FontFamily(v) => font_family = Some(v),
            InlineDeclaration::FontSize(v) => font_size = Some(v),
            InlineDeclaration::FontStyle(v) => font_style = Some(v),
            InlineDeclaration::FontWeight(v) => font_weight = Some(v),
            InlineDeclaration::FontWidth(v) => font_width = Some(v),
            InlineDeclaration::FontVariations(v) => font_variations = Some(v),
            InlineDeclaration::FontFeatures(v) => font_features = Some(v),
            InlineDeclaration::Locale(v) => locale = Some(v),
            InlineDeclaration::Underline(v) => underline = Some(v),
            InlineDeclaration::Strikethrough(v) => strikethrough = Some(v),
            InlineDeclaration::LineHeight(v) => line_height = Some(v),
            InlineDeclaration::WordSpacing(v) => word_spacing = Some(v),
            InlineDeclaration::LetterSpacing(v) => letter_spacing = Some(v),
            InlineDeclaration::BidiControl(v) => bidi_control = Some(v),
        }
    }

    let parent = ctx.parent();
    let initial = ctx.initial();
    let root = ctx.root();

    let mut out = parent.clone();

    if let Some(value) = font_family {
        out.font_family =
            resolve_specified(value, &parent.font_family, &initial.font_family).clone();
    }

    if let Some(value) = font_size {
        out.font_size_px = match value {
            Specified::Inherit => parent.font_size_px,
            Specified::Initial => initial.font_size_px,
            Specified::Value(v) => resolve_font_size(*v, parent.font_size_px, root.font_size_px),
        };
    }

    if let Some(value) = font_style {
        out.font_style = *resolve_specified(value, &parent.font_style, &initial.font_style);
    }

    if let Some(value) = font_weight {
        out.font_weight = *resolve_specified(value, &parent.font_weight, &initial.font_weight);
    }

    if let Some(value) = font_width {
        out.font_width = *resolve_specified(value, &parent.font_width, &initial.font_width);
    }

    if let Some(value) = font_variations {
        out.font_variations =
            resolve_variations(value, &parent.font_variations, &initial.font_variations);
    }

    if let Some(value) = font_features {
        out.font_features = resolve_features(value, &parent.font_features, &initial.font_features);
    }

    if let Some(value) = locale {
        out.locale = *resolve_specified(value, &parent.locale, &initial.locale);
    }

    if let Some(value) = underline {
        out.underline = *resolve_specified(value, &parent.underline, &initial.underline);
    }

    if let Some(value) = strikethrough {
        out.strikethrough =
            *resolve_specified(value, &parent.strikethrough, &initial.strikethrough);
    }

    if let Some(value) = bidi_control {
        out.bidi_control = *resolve_specified(value, &parent.bidi_control, &initial.bidi_control);
    }

    if let Some(value) = line_height {
        out.line_height = match value {
            Specified::Inherit => parent.line_height,
            Specified::Initial => initial.line_height,
            Specified::Value(v) => resolve_line_height(*v, out.font_size_px, root.font_size_px),
        };
    }

    if let Some(value) = word_spacing {
        out.word_spacing_px = match value {
            Specified::Inherit => parent.word_spacing_px,
            Specified::Initial => initial.word_spacing_px,
            Specified::Value(v) => resolve_spacing(*v, out.font_size_px, root.font_size_px),
        };
    }

    if let Some(value) = letter_spacing {
        out.letter_spacing_px = match value {
            Specified::Inherit => parent.letter_spacing_px,
            Specified::Initial => initial.letter_spacing_px,
            Specified::Value(v) => resolve_spacing(*v, out.font_size_px, root.font_size_px),
        };
    }

    out
}

/// Resolves a list of paragraph declarations into a computed paragraph style.
///
/// The input is treated as a declaration list: if multiple declarations of the same property are
/// present, the last declaration wins.
pub fn resolve_paragraph_declarations(
    declarations: &[ParagraphDeclaration],
    ctx: ParagraphResolveContext<'_>,
) -> ComputedParagraphStyle {
    let mut base_direction: Option<&Specified<BaseDirection>> = None;
    let mut word_break: Option<&Specified<WordBreak>> = None;
    let mut overflow_wrap: Option<&Specified<OverflowWrap>> = None;
    let mut text_wrap_mode: Option<&Specified<TextWrapMode>> = None;

    for decl in declarations {
        match decl {
            ParagraphDeclaration::BaseDirection(v) => base_direction = Some(v),
            ParagraphDeclaration::WordBreak(v) => word_break = Some(v),
            ParagraphDeclaration::OverflowWrap(v) => overflow_wrap = Some(v),
            ParagraphDeclaration::TextWrapMode(v) => text_wrap_mode = Some(v),
        }
    }

    let parent = ctx.parent();
    let initial = ctx.initial();

    let mut out = parent.clone();

    if let Some(value) = base_direction {
        out.base_direction =
            *resolve_specified(value, &parent.base_direction, &initial.base_direction);
    }
    if let Some(value) = word_break {
        out.word_break = *resolve_specified(value, &parent.word_break, &initial.word_break);
    }
    if let Some(value) = overflow_wrap {
        out.overflow_wrap =
            *resolve_specified(value, &parent.overflow_wrap, &initial.overflow_wrap);
    }
    if let Some(value) = text_wrap_mode {
        out.text_wrap_mode =
            *resolve_specified(value, &parent.text_wrap_mode, &initial.text_wrap_mode);
    }

    out
}

#[inline]
fn resolve_specified<'a, T>(specified: &'a Specified<T>, parent: &'a T, initial: &'a T) -> &'a T {
    match specified {
        Specified::Inherit => parent,
        Specified::Initial => initial,
        Specified::Value(value) => value,
    }
}

#[inline]
fn resolve_font_size(specified: FontSize, parent_font_size_px: f32, root_font_size_px: f32) -> f32 {
    match specified {
        FontSize::Px(px) => px,
        FontSize::Em(em) => parent_font_size_px * em,
        FontSize::Rem(rem) => root_font_size_px * rem,
    }
}

#[inline]
fn resolve_spacing(specified: Spacing, font_size_px: f32, root_font_size_px: f32) -> f32 {
    match specified {
        Spacing::Px(px) => px,
        Spacing::Em(em) => font_size_px * em,
        Spacing::Rem(rem) => root_font_size_px * rem,
    }
}

#[inline]
fn resolve_line_height(
    specified: LineHeight,
    font_size_px: f32,
    root_font_size_px: f32,
) -> ComputedLineHeight {
    match specified {
        LineHeight::Normal => ComputedLineHeight::MetricsRelative(1.0),
        LineHeight::Factor(f) => ComputedLineHeight::FontSizeRelative(f),
        LineHeight::Px(px) => ComputedLineHeight::Px(px),
        LineHeight::Em(em) => ComputedLineHeight::Px(font_size_px * em),
        LineHeight::Rem(rem) => ComputedLineHeight::Px(root_font_size_px * rem),
    }
}

#[inline]
fn resolve_variations(
    specified: &Specified<FontVariations>,
    parent: &Arc<[FontVariation]>,
    initial: &Arc<[FontVariation]>,
) -> Arc<[FontVariation]> {
    match specified {
        Specified::Inherit => Arc::clone(parent),
        Specified::Initial => Arc::clone(initial),
        Specified::Value(value) => Arc::clone(value.as_arc_slice()),
    }
}

#[inline]
fn resolve_features(
    specified: &Specified<FontFeatures>,
    parent: &Arc<[FontFeature]>,
    initial: &Arc<[FontFeature]>,
) -> Arc<[FontFeature]> {
    match specified {
        Specified::Inherit => Arc::clone(parent),
        Specified::Initial => Arc::clone(initial),
        Specified::Value(value) => Arc::clone(value.as_arc_slice()),
    }
}
