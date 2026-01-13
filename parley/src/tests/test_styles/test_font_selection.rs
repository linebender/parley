// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for font selection style properties.

use alloc::borrow::Cow;
use alloc::format;

use crate::layout::Alignment;
use crate::style::StyleProperty;
use crate::test_name;
use crate::tests::utils::{ColorBrush, TestEnv, samples};
use crate::{AlignmentOptions, Layout};

/// Helper to build a layout with a single font size applied
fn build_with_font_size(env: &mut TestEnv, text: &str, size: f32) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontSize(size));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    layout
}

// ============================================================================
// FontSize Tests
// ============================================================================

#[test]
fn style_font_size_values() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    // Test multiple font sizes
    for size in [12.0, 16.0, 24.0, 36.0, 48.0] {
        let layout = build_with_font_size(&mut env, text, size);

        // Snapshot for visual verification
        env.with_name(&format!("size_{size}"))
            .check_layout_snapshot(&layout);
    }
}

// ============================================================================
// FontWeight Tests
// ============================================================================

#[test]
fn style_font_weight_values() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    use crate::FontWeight;
    use crate::style::FontFamily;

    for (weight, name) in [
        (FontWeight::THIN, "thin"),
        (FontWeight::LIGHT, "light"),
        (FontWeight::NORMAL, "normal"),
        (FontWeight::MEDIUM, "medium"),
        (FontWeight::SEMI_BOLD, "semibold"),
        (FontWeight::BOLD, "bold"),
        (FontWeight::BLACK, "black"),
    ] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named("Roboto Flex")));
        builder.push_default(StyleProperty::FontWeight(weight));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.with_name(name).check_layout_snapshot(&layout);
    }
}

// ============================================================================
// FontWidth Tests
// ============================================================================

#[test]
fn style_font_width_values() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    use crate::FontWidth;
    use crate::style::FontFamily;

    for (width, name) in [
        (FontWidth::ULTRA_CONDENSED, "ultra_condensed"),
        (FontWidth::CONDENSED, "condensed"),
        (FontWidth::NORMAL, "normal"),
        (FontWidth::EXPANDED, "expanded"),
        (FontWidth::ULTRA_EXPANDED, "ultra_expanded"),
    ] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named("Roboto Flex")));
        builder.push_default(StyleProperty::FontWidth(width));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.with_name(name).check_layout_snapshot(&layout);
    }
}

// ============================================================================
// FontStyle Tests
// ============================================================================

#[test]
fn style_font_style_values() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    use crate::setting::Tag;
    use crate::style::{FontFamily, FontVariation, FontVariations};

    // Using Roboto Flex with slnt axis for italic/oblique effects
    // TODO: FontStyle property doesn't automatically map to slnt axis for variable fonts,
    // so, for this test, we use FontVariations directly
    for (slnt_value, name) in [(0.0, "normal"), (-10.0, "italic"), (-10.0, "oblique")] {
        let variations = FontVariations::List(Cow::Borrowed(&[FontVariation {
            tag: Tag::new(b"slnt"),
            value: slnt_value,
        }]));

        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named("Roboto Flex")));
        builder.push_default(StyleProperty::FontVariations(variations));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.with_name(name).check_layout_snapshot(&layout);
    }
}

// ============================================================================
// FontFamily Tests
// ============================================================================

#[test]
fn style_font_family_named() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    use crate::style::FontFamily;

    // Test with Roboto (should be available in test fonts)
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontFamily(FontFamily::named("Roboto")));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    env.with_name("roboto").check_layout_snapshot(&layout);
}
