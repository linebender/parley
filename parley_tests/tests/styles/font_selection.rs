// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for font selection style properties.

use std::borrow::Cow;

use crate::test_name;
use crate::util::{ColorBrush, TestEnv, samples};
use fontique::FontInfoOverride;
use parley::style::StyleProperty;
use parley::{FontWeight, Layout};

/// Helper to build a layout with a single font size applied
fn build_with_font_size(env: &mut TestEnv, text: &str, size: f32) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontSize(size));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
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

    use parley::FontWeight;
    use parley::style::FontFamily;

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

        env.with_name(name).check_layout_snapshot(&layout);
    }
}

#[test]
fn style_font_weight_uses_variable_axis_default() {
    use parley::setting::Tag;
    use parley::style::{FontFamily, FontVariation, FontVariations};

    const FAMILY: &str = "Roboto Flex Declared Thin";
    const TEXT: &str = "Variable weight\nVariable weight";

    let mut env = TestEnv::new(test_name!(), None);
    let roboto_flex = env
        .collection()
        .family_by_name("Roboto Flex")
        .unwrap()
        .default_font()
        .unwrap()
        .load(None)
        .unwrap();
    // Re-register Roboto Flex with a user declared `weight` override.
    let registered = env.collection().register_fonts(
        roboto_flex,
        Some(FontInfoOverride {
            family_name: Some(FAMILY),
            weight: Some(FontWeight::THIN),
            ..Default::default()
        }),
    );
    let font = &registered[0].1[0];
    let weight_axis = font
        .axes()
        .iter()
        .find(|axis| axis.tag.to_be_bytes() == *b"wght")
        .unwrap();
    assert_eq!(font.weight(), FontWeight::THIN);
    assert_eq!(weight_axis.default, FontWeight::NORMAL.value());

    let mut builder = env.ranged_builder(TEXT);
    builder.push_default(StyleProperty::FontSize(48.0));
    builder.push_default(StyleProperty::FontFamily(FontFamily::named(FAMILY)));
    builder.push_default(StyleProperty::FontWeight(FontWeight::THIN));

    // The first line relies on Fontique's synthesis. Give the second line the
    // equivalent variation explicitly so the snapshot includes a reference.
    let second_line = TEXT.find('\n').unwrap() + 1;
    builder.push(
        StyleProperty::FontVariations(FontVariations::List(Cow::Borrowed(&[FontVariation {
            tag: Tag::new(b"wght"),
            value: FontWeight::THIN.value(),
        }]))),
        second_line..,
    );

    let mut layout = builder.build(TEXT);
    layout.break_all_lines(None);

    env.check_layout_snapshot(&layout);
}

// ============================================================================
// FontWidth Tests
// ============================================================================

#[test]
fn style_font_width_values() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    use parley::FontWidth;
    use parley::style::FontFamily;

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

    use parley::FontStyle;
    use parley::style::FontFamily;

    // Note Roboto Flex has a `slnt` axis but no `ital` axis, so italic and oblique get a `slnt`
    // variation.
    for (style, name) in [
        (FontStyle::Normal, "normal"),
        (FontStyle::Italic, "italic"),
        (FontStyle::Oblique(None), "oblique"),
    ] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named("Roboto Flex")));
        builder.push_default(StyleProperty::FontStyle(style));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);

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

    use parley::style::FontFamily;

    // Test with Roboto (should be available in test fonts)
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontFamily(FontFamily::named("Roboto")));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    env.with_name("roboto").check_layout_snapshot(&layout);
}

// ============================================================================
// unicode-range Tests
// ============================================================================

/// A font whose `unicode-range` excludes "é" but includes its decomposition can serve it.
#[test]
fn style_font_unicode_range_decomposition() {
    use fontique::UnicodeRangeSet;
    use parley::PositionedLayoutItem;
    use parley::style::FontFamily;

    let mut env = TestEnv::new(test_name!(), None);
    let mut load = |name| {
        let family = env.collection().family_by_name(name).unwrap();
        family.default_font().unwrap().load(None).unwrap()
    };
    let (roboto, arimo) = (load("Roboto"), load("Arimo"));
    // "e" and U+0301 COMBINING ACUTE ACCENT. Arimo is registered last, so it is tried first.
    let range = UnicodeRangeSet::new([0x65..=0x65, 0x301..=0x301]);
    for (font, unicode_range) in [(roboto, None), (arimo.clone(), Some(&range))] {
        let info = FontInfoOverride {
            family_name: Some("Composite"),
            unicode_range,
            ..Default::default()
        };
        env.collection().register_fonts(font, Some(info));
    }

    for (text, from_arimo) in [("x", false), ("\u{e9}", true)] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named("Composite")));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        let item = layout.lines().next().unwrap().items().next().unwrap();
        let PositionedLayoutItem::GlyphRun(run) = item else {
            unreachable!()
        };
        assert_eq!(
            run.run().font().data.id() == arimo.id(),
            from_arimo,
            "{text}"
        );
    }
}

/// A pending font is requested by text in its range, and used once its data is supplied.
#[test]
fn style_font_pending() {
    use fontique::{GenericFamily, UnicodeRangeSet};
    use parley::PositionedLayoutItem;

    let mut env = TestEnv::new(test_name!(), None);
    let family = env.collection().family_by_name("Arimo").unwrap();
    let arimo = family.default_font().unwrap().load(None).unwrap();
    let mut declare = |family, range| {
        let range = UnicodeRangeSet::new([range]);
        let info = FontInfoOverride {
            unicode_range: Some(&range),
            ..Default::default()
        };
        env.collection().declare_font(family, info)
    };
    let space = declare("Space", 0x20..=0x20).1;
    let digits = declare("Digits", 0x30..=0x39);
    declare("Other", 0x30..=0x39);
    let collection = env.collection();
    collection.set_generic_families(GenericFamily::SansSerif, [digits.0].into_iter());

    // Returns the fonts that `text` requests, and the font of its first run.
    let build = |env: &mut TestEnv, families: &'static str, text| {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontFamily(families.into()));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        let item = layout.lines().next().and_then(|line| line.items().next());
        let font = item.map(|item| match item {
            PositionedLayoutItem::GlyphRun(run) => run.run().font().data.id(),
            _ => unreachable!(),
        });
        let requests = env.collection().drain_font_requests();
        (requests.collect::<Vec<_>>(), font)
    };
    // The second list has no font to use. No font has U+4E00 and a tab needs no font, but
    // pending fonts are not used for those either. Only the first pending font is requested.
    for families in ["Digits, Other, Roboto", "Digits, Other"] {
        assert_eq!(build(&mut env, families, "\u{4e00}\t").0, [], "{families}");
        assert_eq!(build(&mut env, families, "1").0, [digits.1], "{families}");
    }
    // The first available font is requested for its metrics.
    assert_eq!(build(&mut env, "Space, Roboto", "x").0, [space]);
    assert!(env.collection().supply_font(digits.1, arimo.clone(), 0));
    let built = build(&mut env, "Digits, Roboto", "1");
    assert_eq!(built, (vec![], Some(arimo.id())));
}
