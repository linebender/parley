// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for style property interactions.
//!
//! These tests verify behavior when multiple style properties are combined,
//! especially when they might affect each other.

use alloc::borrow::Cow;

use crate::layout::Alignment;
use crate::setting::Tag;
use crate::style::{FontFeature, FontSettings, FontVariation, LineHeight, StyleProperty};
use crate::test_name;
use crate::tests::utils::{samples, TestEnv};
use crate::AlignmentOptions;

// ============================================================================
// FontSize × LineHeight Interactions
// ============================================================================

#[test]
fn interaction_font_size_line_height_relative() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN_MULTILINE;

    // Test FontSizeRelative line height at different font sizes
    // The line height should scale proportionally with font size
    for font_size in [12.0, 24.0, 36.0] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.5)));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.with_name(&format!("size_{font_size}"))
            .check_layout_snapshot(&layout);
    }
}

#[test]
fn interaction_font_size_line_height_absolute() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN_MULTILINE;

    // Test Absolute line height at different font sizes
    // The line height should stay constant regardless of font size
    let absolute_height = 30.0;

    for font_size in [12.0, 24.0, 36.0] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(absolute_height)));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.with_name(&format!("size_{font_size}"))
            .check_layout_snapshot(&layout);
    }
}

// ============================================================================
// LetterSpacing × Ligatures Interactions
// ============================================================================

#[test]
// TODO: Ligatures should break with letter spacing. They currently do not.
fn interaction_letter_spacing_ligatures() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LIGATURES;

    // Without letter spacing - ligatures should form (if font supports them)
    let features_on = FontSettings::List(Cow::Borrowed(&[FontFeature {
        tag: Tag::new(b"liga"),
        value: 1,
    }]));

    let mut builder_no_spacing = env.ranged_builder(text);
    builder_no_spacing.push_default(StyleProperty::FontFeatures(features_on.clone()));
    builder_no_spacing.push_default(StyleProperty::LetterSpacing(0.0));
    let mut layout_no_spacing = builder_no_spacing.build(text);
    layout_no_spacing.break_all_lines(None);
    layout_no_spacing.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("no_spacing")
        .check_layout_snapshot(&layout_no_spacing);

    // With letter spacing - ligatures SHOULD break
    let mut builder_with_spacing = env.ranged_builder(text);
    builder_with_spacing.push_default(StyleProperty::FontFeatures(features_on));
    builder_with_spacing.push_default(StyleProperty::LetterSpacing(2.0));
    let mut layout_with_spacing = builder_with_spacing.build(text);
    layout_with_spacing.break_all_lines(None);
    layout_with_spacing.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("with_spacing")
        .check_layout_snapshot(&layout_with_spacing);
}

// ============================================================================
// FontWeight × FontVariations(wght) Interactions
// ============================================================================

#[test]
fn interaction_font_weight_vs_variations() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    use crate::style::{FontFamily, FontStack};
    use crate::FontWeight;

    // FontWeight only
    let mut builder_weight = env.ranged_builder(text);
    builder_weight.push_default(StyleProperty::FontStack(FontStack::Single(
        FontFamily::Named(Cow::Borrowed("Arimo")),
    )));
    builder_weight.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
    let mut layout_weight = builder_weight.build(text);
    layout_weight.break_all_lines(None);
    layout_weight.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("weight_only")
        .check_layout_snapshot(&layout_weight);

    // FontVariations(wght) only
    let variations = FontSettings::List(Cow::Borrowed(&[FontVariation {
        tag: Tag::new(b"wght"),
        value: 700.0,
    }]));

    let mut builder_variation = env.ranged_builder(text);
    builder_variation.push_default(StyleProperty::FontStack(FontStack::Single(
        FontFamily::Named(Cow::Borrowed("Arimo")),
    )));
    builder_variation.push_default(StyleProperty::FontVariations(variations.clone()));
    let mut layout_variation = builder_variation.build(text);
    layout_variation.break_all_lines(None);
    layout_variation.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("variation_only")
        .check_layout_snapshot(&layout_variation);

    // Both FontWeight and FontVariations - which takes precedence?
    let mut builder_both = env.ranged_builder(text);
    builder_both.push_default(StyleProperty::FontStack(FontStack::Single(
        FontFamily::Named(Cow::Borrowed("Arimo")),
    )));
    builder_both.push_default(StyleProperty::FontWeight(FontWeight::LIGHT)); // 300
    builder_both.push_default(StyleProperty::FontVariations(variations)); // 700
    let mut layout_both = builder_both.build(text);
    layout_both.break_all_lines(None);
    layout_both.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("both").check_layout_snapshot(&layout_both);
}

// ============================================================================
// WordSpacing × Alignment::Justify Interactions
// ============================================================================

// TODO: Word spacing does not expand content box for justified text.
#[test]
fn interaction_word_spacing_justify() {
    let mut env = TestEnv::new(test_name!(), None);
    // Use text that will wrap to multiple lines when constrained
    let text = "The quick brown fox jumps over the lazy dog and runs away quickly.";

    // Justified with no extra word spacing
    let mut builder_no_spacing = env.ranged_builder(text);
    builder_no_spacing.push_default(StyleProperty::WordSpacing(0.0));
    let mut layout_no_spacing = builder_no_spacing.build(text);
    layout_no_spacing.break_all_lines(Some(200.0));
    layout_no_spacing.align(Some(200.0), Alignment::Justify, AlignmentOptions::default());

    env.with_name("justify_no_spacing")
        .check_layout_snapshot(&layout_no_spacing);

    // Justified with extra word spacing
    let mut builder_with_spacing = env.ranged_builder(text);
    builder_with_spacing.push_default(StyleProperty::WordSpacing(4.0));
    let mut layout_with_spacing = builder_with_spacing.build(text);
    layout_with_spacing.break_all_lines(Some(200.0));
    layout_with_spacing.align(Some(200.0), Alignment::Justify, AlignmentOptions::default());

    env.with_name("justify_with_spacing")
        .check_layout_snapshot(&layout_with_spacing);
}

// ============================================================================
// Multiple Decoration Interactions
// ============================================================================

#[test]
fn interaction_underline_with_offset_and_size() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    // Default underline
    let mut builder_default = env.ranged_builder(text);
    builder_default.push_default(StyleProperty::Underline(true));
    let mut layout_default = builder_default.build(text);
    layout_default.break_all_lines(None);
    layout_default.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("default")
        .check_layout_snapshot(&layout_default);

    // Custom offset and size
    let mut builder_custom = env.ranged_builder(text);
    builder_custom.push_default(StyleProperty::Underline(true));
    builder_custom.push_default(StyleProperty::UnderlineOffset(Some(3.0)));
    builder_custom.push_default(StyleProperty::UnderlineSize(Some(2.0)));
    let mut layout_custom = builder_custom.build(text);
    layout_custom.break_all_lines(None);
    layout_custom.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("custom").check_layout_snapshot(&layout_custom);
}

// ============================================================================
// Font Selection Combinations
// ============================================================================

#[test]
fn interaction_weight_style_width() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    use crate::setting::Tag;
    use crate::style::{FontFamily, FontSettings, FontStack, FontVariation};
    use crate::{FontWeight, FontWidth};

    // Bold + Italic (using slnt axis)
    let italic_variation = FontSettings::List(Cow::Borrowed(&[FontVariation {
        tag: Tag::new(b"slnt"),
        value: -10.0,
    }]));

    let mut builder_bold_italic = env.ranged_builder(text);
    builder_bold_italic.push_default(StyleProperty::FontStack(FontStack::Single(
        FontFamily::Named(Cow::Borrowed("Roboto Flex")),
    )));
    builder_bold_italic.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
    builder_bold_italic.push_default(StyleProperty::FontVariations(italic_variation.clone()));
    let mut layout_bold_italic = builder_bold_italic.build(text);
    layout_bold_italic.break_all_lines(None);
    layout_bold_italic.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("bold_italic")
        .check_layout_snapshot(&layout_bold_italic);

    // Light + Condensed
    let mut builder_light_condensed = env.ranged_builder(text);
    builder_light_condensed.push_default(StyleProperty::FontStack(FontStack::Single(
        FontFamily::Named(Cow::Borrowed("Roboto Flex")),
    )));
    builder_light_condensed.push_default(StyleProperty::FontWeight(FontWeight::LIGHT));
    builder_light_condensed.push_default(StyleProperty::FontWidth(FontWidth::CONDENSED));
    let mut layout_light_condensed = builder_light_condensed.build(text);
    layout_light_condensed.break_all_lines(None);
    layout_light_condensed.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("light_condensed")
        .check_layout_snapshot(&layout_light_condensed);

    // Black + Expanded + Italic (using slnt axis)
    let mut builder_complex = env.ranged_builder(text);
    builder_complex.push_default(StyleProperty::FontStack(FontStack::Single(
        FontFamily::Named(Cow::Borrowed("Roboto Flex")),
    )));
    builder_complex.push_default(StyleProperty::FontWeight(FontWeight::BLACK));
    builder_complex.push_default(StyleProperty::FontWidth(FontWidth::EXPANDED));
    builder_complex.push_default(StyleProperty::FontVariations(italic_variation));
    let mut layout_complex = builder_complex.build(text);
    layout_complex.break_all_lines(None);
    layout_complex.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("black_expanded_italic")
        .check_layout_snapshot(&layout_complex);
}

