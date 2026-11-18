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
fn pending_font_requests_use_unicode_range_and_fallback() {
    use fontique::{
        Blob, Collection, CollectionOptions, FontId, QueryFamily, SourceCache, UnicodeRangeSet,
    };
    use parley::style::FontFamily;
    use parley::{FontContext, LayoutContext};
    use std::sync::Arc;

    const ROBOTO: &[u8] =
        include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");
    for shared in [false, true] {
        let mut fonts = FontContext {
            collection: Collection::new(CollectionOptions {
                shared,
                system_fonts: false,
            }),
            source_cache: SourceCache::default(),
        };
        let (one, two) = (FontId::new(), FontId::new());
        let fallback = Blob::new(Arc::new(ROBOTO.to_vec()));
        fonts.collection.register_font(
            FontId::new(),
            fallback.clone(),
            0,
            Some(FontInfoOverride {
                family_name: Some("Fallback"),
                ..Default::default()
            }),
        );
        assert!(
            fonts
                .collection
                .register_pending_font(one, FontInfoOverride::default())
                .is_none()
        );
        let range = UnicodeRangeSet::new(std::iter::once(u32::from('a')..=u32::from('z')));
        let info = FontInfoOverride {
            family_name: Some("A"),
            unicode_range: Some(&range),
            ..Default::default()
        };
        let family = fonts.collection.register_pending_font(one, info).unwrap();
        assert_eq!(fonts.collection.family_id("A"), Some(family));
        assert!(fonts.collection.register_pending_font(one, info).is_none());
        let mut cloned = fonts.collection.clone();

        let mut context = LayoutContext::<[u8; 4]>::new();
        for (text, expected) in [("abc abc", vec![one]), ("123", vec![]), ("", vec![])] {
            let mut builder = context.ranged_builder(&mut fonts, text, 1.0, true);
            builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
                "A, Fallback".into(),
            )));
            let mut layout = builder.build(text);
            layout.break_all_lines(None);
            assert_eq!(
                context.drain_pending_font_requests().collect::<Vec<_>>(),
                expected
            );
            assert_eq!(context.drain_pending_font_requests().count(), 0);
            let runs: Vec<_> = layout.lines().flat_map(|line| line.runs()).collect();
            if !text.is_empty() {
                assert!(!runs.is_empty());
                assert!(runs.iter().all(|run| run.font().data.id() == fallback.id()));
            }
        }
        {
            let mut query = fonts.collection.query(&mut fonts.source_cache);
            query.set_families([QueryFamily::Named("A"), QueryFamily::Named("Fallback")]);
            assert_eq!(
                query.first_available_font().unwrap().blob.id(),
                fallback.id()
            );
        }
        assert!(
            fonts
                .collection
                .resolve_pending_font(one, fallback.clone(), 0)
        );
        // The resolved font keeps its identifier and is no longer pending.
        assert!(fonts.collection.font(one).unwrap().pending().is_none());
        assert_eq!(fonts.collection.font_family_id(one), Some(family));
        assert_eq!(
            cloned.font(one).unwrap().pending().is_none(),
            shared,
            "clones see the font only when the collection is shared"
        );
        assert!(
            fonts
                .collection
                .family(family)
                .unwrap()
                .fonts()
                .iter()
                .all(|font| font.load(None).is_some())
        );

        fonts.collection.register_pending_font(two, info).unwrap();
        assert!(fonts.collection.unregister_font(two));
        assert!(fonts.collection.font(two).is_none());
        // An identifier can be registered again once its font is removed.
        assert!(fonts.collection.register_pending_font(two, info).is_some());
    }
}

#[test]
fn pending_metrics_and_resolution_preserve_face_priority() {
    use fontique::{
        Blob, Collection, CollectionOptions, FontId, QueryCandidate, QueryStatus, SourceCache,
        UnicodeRangeSet,
    };
    use std::sync::Arc;

    let bytes = include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");
    for shared in [false, true] {
        for order in [[0, 1], [1, 0]] {
            let ids = [FontId::new(), FontId::new()];
            let mut collection = Collection::new(CollectionOptions {
                shared,
                system_fonts: false,
            });
            let range = UnicodeRangeSet::new(std::iter::once(32..=127));
            let info = FontInfoOverride {
                family_name: Some("Metrics"),
                unicode_range: Some(&range),
                ..FontInfoOverride::default()
            };
            let family = collection.register_pending_font(ids[0], info).unwrap();
            collection.register_pending_font(ids[1], info).unwrap();
            let mut cache = SourceCache::default();
            {
                let mut query = collection.query(&mut cache);
                query.set_families([family]);
                query.matches_with(|_| panic!("pending faces must not yield loaded fonts"));
                let mut keys = Vec::new();
                assert!(
                    query
                        .first_available_font_with(|key| keys.push(key))
                        .is_none()
                );
                assert_eq!(keys, [ids[1], ids[0]]);
                let mut candidates = Vec::new();
                query.matches_with_pending(|candidate| {
                    let QueryCandidate::Pending { id, unicode_range } = candidate else {
                        panic!("unexpected loaded font")
                    };
                    assert!(unicode_range.unwrap().contains(' '));
                    candidates.push(id);
                    QueryStatus::Continue
                });
                assert_eq!(candidates, [ids[1], ids[0]]);
            }
            let font = Blob::new(Arc::new(bytes.to_vec()));
            assert!(!collection.resolve_pending_font(FontId::new(), font.clone(), 0));
            // The data holds a single font, so only index 0 names one.
            assert!(!collection.resolve_pending_font(ids[0], font, 1));
            assert!(!collection.resolve_pending_font(
                ids[0],
                Blob::new(Arc::new(Vec::<u8>::new())),
                0
            ));
            let fonts = [
                Blob::new(Arc::new(bytes.to_vec())),
                Blob::new(Arc::new(bytes.to_vec())),
            ];
            for i in order {
                assert!(collection.resolve_pending_font(ids[i], fonts[i].clone(), 0));
                // A font with data cannot be resolved again.
                assert!(!collection.resolve_pending_font(ids[i], fonts[i].clone(), 0));
            }
            let family_info = collection.family(family).unwrap();
            let loaded: Vec<_> = family_info
                .fonts()
                .iter()
                .map(|font| font.load(None).unwrap().id())
                .collect();
            assert_eq!(loaded, [fonts[1].id(), fonts[0].id()]);
            let loaded_ids: Vec<_> = family_info.fonts().iter().map(|font| font.id()).collect();
            assert_eq!(loaded_ids, [ids[1], ids[0]]);
            let mut query = collection.query(&mut cache);
            query.set_families([family]);
            assert_eq!(
                query.first_available_font().unwrap().blob.id(),
                fonts[1].id()
            );
        }
    }
}

#[test]
fn pending_font_status_controls_reporting() {
    use fontique::{
        Blob, Collection, CollectionOptions, FontId, PendingFontStatus, QueryStatus, SourceCache,
    };
    use std::sync::Arc;

    fn reported(collection: &mut Collection, family: fontique::FamilyId) -> Vec<FontId> {
        let mut cache = SourceCache::default();
        let mut query = collection.query(&mut cache);
        query.set_families([family]);
        let mut metrics_keys = Vec::new();
        assert!(
            query
                .first_available_font_with(|key| metrics_keys.push(key))
                .is_none()
        );
        let mut keys = Vec::new();
        query.matches_with_pending(|candidate| {
            if let fontique::QueryCandidate::Pending { id, .. } = candidate {
                keys.push(id);
            }
            QueryStatus::Continue
        });
        assert_eq!(keys, metrics_keys);
        keys
    }

    let bytes = include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");
    for shared in [false, true] {
        let mut collection = Collection::new(CollectionOptions {
            shared,
            system_fonts: false,
        });
        let info = FontInfoOverride {
            family_name: Some("Statuses"),
            ..FontInfoOverride::default()
        };
        let (one, two) = (FontId::new(), FontId::new());
        let family = collection.register_pending_font(one, info).unwrap();
        collection.register_pending_font(two, info).unwrap();
        let mut clone = collection.clone();
        assert_eq!(
            collection.pending_font_status(one),
            Some(PendingFontStatus::Unloaded)
        );
        assert_eq!(reported(&mut collection, family), [two, one]);

        // A face that is loading is reported no more, on this collection.
        assert!(!collection.set_pending_font_status(FontId::new(), PendingFontStatus::Loading));
        assert!(!collection.set_pending_font_status(one, PendingFontStatus::Unloaded));
        assert!(collection.set_pending_font_status(one, PendingFontStatus::Loading));
        assert!(!collection.set_pending_font_status(one, PendingFontStatus::Loading));
        assert_eq!(
            collection.pending_font_status(one),
            Some(PendingFontStatus::Loading)
        );
        assert_eq!(reported(&mut collection, family), [two]);
        // The status is shared only between clones of a shared collection.
        let clone_status = if shared {
            PendingFontStatus::Loading
        } else {
            PendingFontStatus::Unloaded
        };
        assert_eq!(clone.pending_font_status(one), Some(clone_status));

        // A failed face keeps its slot and is not reported either.
        assert!(collection.set_pending_font_status(one, PendingFontStatus::Failed));
        assert!(collection.set_pending_font_status(two, PendingFontStatus::Failed));
        assert_eq!(
            collection.pending_font_status(one),
            Some(PendingFontStatus::Failed)
        );
        assert_eq!(collection.family(family).unwrap().fonts().len(), 2);
        assert_eq!(reported(&mut collection, family), []);

        // A face in any status can still be resolved, in its original position, or removed.
        let font = Blob::new(Arc::new(bytes.to_vec()));
        assert!(collection.resolve_pending_font(one, font.clone(), 0));
        assert_eq!(collection.pending_font_status(one), None);
        let fonts = collection.family(family).unwrap();
        assert_eq!(fonts.fonts()[1].load(None).unwrap().id(), font.id());
        assert!(collection.unregister_font(two));
        assert_eq!(collection.pending_font_status(two), None);
        assert_eq!(collection.family(family).unwrap().fonts().len(), 1);
    }
}

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
    let registered = env.collection().register_font(
        fontique::FontId::new(),
        roboto_flex,
        0,
        Some(FontInfoOverride {
            family_name: Some(FAMILY),
            weight: Some(FontWeight::THIN.into()),
            ..Default::default()
        }),
    );
    let font = &registered.unwrap().1;
    let weight_axis = font
        .axes()
        .iter()
        .find(|axis| axis.tag.to_be_bytes() == *b"wght")
        .unwrap();
    assert_eq!(
        font.weight(),
        fontique::FontWeightRange::from(FontWeight::THIN)
    );
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
        env.collection()
            .register_font(fontique::FontId::new(), font, 0, Some(info));
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
