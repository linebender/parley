// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::ParseSettingsError;
use crate::computed::{ComputedInlineStyle, ComputedLineHeight, ComputedParagraphStyle};
use crate::context::{InlineResolveContext, ParagraphResolveContext};
use crate::error::ResolveStyleError;
use crate::parse::{parse_feature_settings, parse_variation_settings};
use text_style::{
    BaseDirection, BidiControl, FontSize, FontStack, FontStyle, FontWeight, FontWidth,
    InlineDeclaration, LineHeight, OverflowWrap, ParagraphDeclaration, Setting, Settings, Spacing,
    Specified, TextWrapMode, WordBreak,
};

/// Resolves a list of inline declarations into a computed inline style.
///
/// The input is treated as a declaration list: if multiple declarations of the same property are
/// present, the last declaration wins.
pub fn resolve_inline_declarations(
    declarations: &[InlineDeclaration],
    ctx: InlineResolveContext<'_>,
) -> Result<ComputedInlineStyle, ResolveStyleError> {
    let mut font_stack: Option<&Specified<FontStack>> = None;
    let mut font_size: Option<&Specified<FontSize>> = None;
    let mut font_style: Option<&Specified<FontStyle>> = None;
    let mut font_weight: Option<&Specified<FontWeight>> = None;
    let mut font_width: Option<&Specified<FontWidth>> = None;
    let mut font_variations: Option<&Specified<Settings<f32>>> = None;
    let mut font_features: Option<&Specified<Settings<u16>>> = None;
    let mut locale: Option<&Specified<Option<alloc::sync::Arc<str>>>> = None;
    let mut underline: Option<&Specified<bool>> = None;
    let mut strikethrough: Option<&Specified<bool>> = None;
    let mut line_height: Option<&Specified<LineHeight>> = None;
    let mut word_spacing: Option<&Specified<Spacing>> = None;
    let mut letter_spacing: Option<&Specified<Spacing>> = None;
    let mut bidi_control: Option<&Specified<BidiControl>> = None;

    for decl in declarations {
        match decl {
            InlineDeclaration::FontStack(v) => font_stack = Some(v),
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
            _ => {}
        }
    }

    let parent = ctx.parent();
    let initial = ctx.initial();
    let root = ctx.root();

    let mut out = parent.clone();

    if let Some(value) = font_stack {
        out.font_stack = resolve_specified(value, &parent.font_stack, &initial.font_stack).clone();
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
            resolve_settings_f32(value, &parent.font_variations, &initial.font_variations)
                .map_err(ResolveStyleError::FontVariations)?;
    }

    if let Some(value) = font_features {
        out.font_features =
            resolve_settings_u16(value, &parent.font_features, &initial.font_features)
                .map_err(ResolveStyleError::FontFeatures)?;
    }

    if let Some(value) = locale {
        out.locale = resolve_specified(value, &parent.locale, &initial.locale).clone();
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

    // Dependent properties follow font-size.
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

    Ok(out)
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
            _ => {}
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

fn resolve_specified<'a, T>(specified: &'a Specified<T>, parent: &'a T, initial: &'a T) -> &'a T {
    match specified {
        Specified::Inherit => parent,
        Specified::Initial => initial,
        Specified::Value(value) => value,
    }
}

fn resolve_font_size(specified: FontSize, parent_font_size_px: f32, root_font_size_px: f32) -> f32 {
    match specified {
        FontSize::Px(px) => px,
        FontSize::Em(em) => parent_font_size_px * em,
        FontSize::Rem(rem) => root_font_size_px * rem,
        _ => {
            debug_assert!(
                false,
                "unhandled FontSize variant; update text_style_resolve for the new unit"
            );
            parent_font_size_px
        }
    }
}

fn resolve_spacing(specified: Spacing, font_size_px: f32, root_font_size_px: f32) -> f32 {
    match specified {
        Spacing::Px(px) => px,
        Spacing::Em(em) => font_size_px * em,
        Spacing::Rem(rem) => root_font_size_px * rem,
        _ => {
            debug_assert!(
                false,
                "unhandled Spacing variant; update text_style_resolve for the new unit"
            );
            0.0
        }
    }
}

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
        _ => {
            debug_assert!(
                false,
                "unhandled LineHeight variant; update text_style_resolve for the new unit"
            );
            ComputedLineHeight::default()
        }
    }
}

fn resolve_settings_f32(
    specified: &Specified<Settings<f32>>,
    parent: &[Setting<f32>],
    initial: &[Setting<f32>],
) -> Result<Vec<Setting<f32>>, ParseSettingsError> {
    match specified {
        Specified::Inherit => Ok(parent.to_vec()),
        Specified::Initial => Ok(initial.to_vec()),
        Specified::Value(Settings::List(list)) => Ok(list.clone()),
        Specified::Value(Settings::Source(source)) => parse_variation_settings(source),
    }
}

fn resolve_settings_u16(
    specified: &Specified<Settings<u16>>,
    parent: &[Setting<u16>],
    initial: &[Setting<u16>],
) -> Result<Vec<Setting<u16>>, ParseSettingsError> {
    match specified {
        Specified::Inherit => Ok(parent.to_vec()),
        Specified::Initial => Ok(initial.to_vec()),
        Specified::Value(Settings::List(list)) => Ok(list.clone()),
        Specified::Value(Settings::Source(source)) => parse_feature_settings(source),
    }
}
