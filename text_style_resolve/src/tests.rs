// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    ComputedInlineStyle, ComputedLineHeight, ComputedParagraphStyle, InlineResolveContext,
    ParagraphResolveContext, ParseSettingsError, ResolveStyleError, ResolveStyleExt,
};
use text_style::{
    BaseDirection, FontSize, InlineDeclaration, InlineStyle, LineHeight, ParagraphStyle, Setting,
    Settings, Spacing, Specified, Tag, WordBreak,
};

#[test]
fn specified_inherit_initial_and_value() {
    let parent = ComputedInlineStyle::default().with_font_size_px(20.0);
    let initial = ComputedInlineStyle::default().with_font_size_px(10.0);
    let root = ComputedInlineStyle::default().with_font_size_px(8.0);

    let ctx = InlineResolveContext::new(&parent, &initial, &root);
    let computed = InlineStyle::new()
        .font_size(Specified::Inherit)
        .resolve(ctx)
        .unwrap();
    assert_eq!(computed.font_size_px(), 20.0);

    let computed = InlineStyle::new()
        .font_size(Specified::Initial)
        .resolve(ctx)
        .unwrap();
    assert_eq!(computed.font_size_px(), 10.0);

    let computed = InlineStyle::new()
        .font_size(Specified::Value(FontSize::Em(2.0)))
        .resolve(ctx)
        .unwrap();
    assert_eq!(computed.font_size_px(), 40.0);

    let computed = InlineStyle::new()
        .font_size(Specified::Value(FontSize::Rem(2.0)))
        .resolve(ctx)
        .unwrap();
    assert_eq!(computed.font_size_px(), 16.0);
}

#[test]
fn em_values_resolve_against_computed_font_size() {
    let base = ComputedInlineStyle::default();

    // letter-spacing depends on computed font size, regardless of declaration order.
    let style = InlineStyle::new()
        .letter_spacing(Specified::Value(Spacing::Em(0.5)))
        .font_size(Specified::Value(FontSize::Px(20.0)));
    let ctx = InlineResolveContext::new(&base, &base, &base);
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.font_size_px(), 20.0);
    assert_eq!(computed.letter_spacing_px(), 10.0);
}

#[test]
fn paragraph_base_direction_resolves() {
    let base = ComputedParagraphStyle::default();
    let style = ParagraphStyle::new().base_direction(Specified::Value(BaseDirection::Rtl));
    let ctx = ParagraphResolveContext::new(&base, &base, &base);
    let computed = style.resolve(ctx);
    assert_eq!(computed.base_direction(), BaseDirection::Rtl);
}

#[test]
fn last_declaration_wins_within_a_style() {
    let base = ComputedInlineStyle::default();
    let ctx = InlineResolveContext::new(&base, &base, &base);

    let style = InlineStyle::new()
        .font_size(Specified::Value(FontSize::Px(10.0)))
        .font_size(Specified::Value(FontSize::Px(20.0)));
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.font_size_px(), 20.0);

    let style = InlineStyle::new()
        .letter_spacing(Specified::Value(Spacing::Em(1.0)))
        .letter_spacing(Specified::Value(Spacing::Px(3.0)));
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.letter_spacing_px(), 3.0);
}

#[test]
fn from_declarations_builds_style_in_order() {
    let base = ComputedInlineStyle::default();
    let ctx = InlineResolveContext::new(&base, &base, &base);

    let style = InlineStyle::from_declarations([
        InlineDeclaration::FontSize(Specified::Value(FontSize::Px(10.0))),
        InlineDeclaration::FontSize(Specified::Value(FontSize::Px(20.0))),
    ]);
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.font_size_px(), 20.0);
}

#[test]
fn rem_resolves_against_root_font_size() {
    let parent = ComputedInlineStyle::default();
    let root = ComputedInlineStyle::default().with_font_size_px(10.0);

    let initial = parent.clone();
    let ctx = InlineResolveContext::new(&parent, &initial, &root);
    let style = InlineStyle::new().font_size(Specified::Value(FontSize::Rem(2.0)));
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.font_size_px(), 20.0);
}

#[test]
fn spacing_rem_resolves_against_root_font_size() {
    let parent = ComputedInlineStyle::default();
    let root = ComputedInlineStyle::default().with_font_size_px(10.0);
    let initial = parent.clone();

    let ctx = InlineResolveContext::new(&parent, &initial, &root);
    let style = InlineStyle::new().letter_spacing(Specified::Value(Spacing::Rem(1.5)));
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.letter_spacing_px(), 15.0);
}

#[test]
fn line_height_rem_resolves_against_root_font_size() {
    let parent = ComputedInlineStyle::default();
    let root = ComputedInlineStyle::default().with_font_size_px(10.0);
    let initial = parent.clone();

    let ctx = InlineResolveContext::new(&parent, &initial, &root);
    let style = InlineStyle::new().line_height(Specified::Value(LineHeight::Rem(2.0)));
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.line_height(), ComputedLineHeight::Px(20.0));
}

#[test]
fn em_spacing_uses_final_computed_font_size_when_font_size_is_rem() {
    let parent = ComputedInlineStyle::default();
    let root = ComputedInlineStyle::default().with_font_size_px(10.0);
    let initial = parent.clone();
    let ctx = InlineResolveContext::new(&parent, &initial, &root);

    // font-size resolves to 20px, so 0.5em should become 10px.
    let style = InlineStyle::new()
        .font_size(Specified::Value(FontSize::Rem(2.0)))
        .letter_spacing(Specified::Value(Spacing::Em(0.5)));
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.font_size_px(), 20.0);
    assert_eq!(computed.letter_spacing_px(), 10.0);
}

#[test]
fn paragraph_inherit_and_initial() {
    let parent = ComputedParagraphStyle {
        word_break: WordBreak::KeepAll,
        ..ComputedParagraphStyle::default()
    };
    let initial = ComputedParagraphStyle::default();
    let ctx = ParagraphResolveContext::new(&parent, &initial, &initial);

    let computed = ParagraphStyle::new()
        .word_break(Specified::Inherit)
        .resolve(ctx);
    assert_eq!(computed.word_break(), WordBreak::KeepAll);

    let computed = ParagraphStyle::new()
        .word_break(Specified::Initial)
        .resolve(ctx);
    assert_eq!(computed.word_break(), WordBreak::Normal);
}

#[test]
fn variation_settings_source_parses_during_resolution() {
    let base = ComputedInlineStyle::default();
    let ctx = InlineResolveContext::new(&base, &base, &base);
    let style = InlineStyle::new().font_variations(Specified::Value(Settings::source(
        "\"wght\" 700, \"wdth\" 120",
    )));

    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.font_variations().len(), 2);
    assert_eq!(
        computed.font_variations()[0],
        Setting::new(Tag::from_bytes(*b"wght"), 700.0)
    );
}

#[test]
fn feature_settings_source_parses_during_resolution() {
    let base = ComputedInlineStyle::default();
    let ctx = InlineResolveContext::new(&base, &base, &base);
    let style = InlineStyle::new().font_features(Specified::Value(Settings::source(
        "\"liga\" on, \"kern\" 0, \"calt\"",
    )));

    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.font_features().len(), 3);
    assert_eq!(
        computed.font_features()[0],
        Setting::new(Tag::from_bytes(*b"liga"), 1)
    );
    assert_eq!(
        computed.font_features()[1],
        Setting::new(Tag::from_bytes(*b"kern"), 0)
    );
    assert_eq!(
        computed.font_features()[2],
        Setting::new(Tag::from_bytes(*b"calt"), 1)
    );
}

#[test]
fn invalid_settings_source_returns_error() {
    let base = ComputedInlineStyle::default();
    let ctx = InlineResolveContext::new(&base, &base, &base);
    let style = InlineStyle::new().font_variations(Specified::Value(Settings::source("wght 1")));

    assert_eq!(
        style.resolve(ctx),
        Err(ResolveStyleError::FontVariations(
            ParseSettingsError::InvalidSyntax
        ))
    );
}

#[test]
fn settings_whitespace_and_trailing_comma_are_ok() {
    let base = ComputedInlineStyle::default();
    let ctx = InlineResolveContext::new(&base, &base, &base);

    let style = InlineStyle::new()
        .font_variations(Specified::Value(Settings::source("  \"wght\"   700  ,  ")));
    let computed = style.resolve(ctx).unwrap();
    assert_eq!(computed.font_variations().len(), 1);
}

#[test]
fn feature_out_of_range_returns_error() {
    let base = ComputedInlineStyle::default();
    let ctx = InlineResolveContext::new(&base, &base, &base);

    let style =
        InlineStyle::new().font_features(Specified::Value(Settings::source("\"liga\" 70000")));
    assert_eq!(
        style.resolve(ctx),
        Err(ResolveStyleError::FontFeatures(
            ParseSettingsError::OutOfRange
        ))
    );
}

#[test]
fn invalid_tag_length_returns_error() {
    let base = ComputedInlineStyle::default();
    let ctx = InlineResolveContext::new(&base, &base, &base);

    let style = InlineStyle::new().font_variations(Specified::Value(Settings::source("\"wg\" 1")));
    assert_eq!(
        style.resolve(ctx),
        Err(ResolveStyleError::FontVariations(
            ParseSettingsError::InvalidTag
        ))
    );
}
