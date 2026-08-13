// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Font selection for emoji variation sequences.

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;
use std::vec::Vec;

use fontique::{Collection, CollectionOptions, MapVariant, SourceCache};
use peniko::Blob;

use super::utils::ColorBrush;
use crate::{FontContext, FontFamily, LayoutContext, StyleProperty};

const MONO_FONT: &str = "noto_emoji/NotoEmoji-Subset.ttf";
const COLOR_FONT: &str = "noto_color_emoji/NotoColorEmoji-Subset.ttf";
const CBDT_FONT: &str = "noto_color_emoji/NotoColorEmoji-CBTF-Subset.ttf";

fn font_blob(file: &str) -> Blob<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../parley_dev/assets/fonts")
        .join(file);
    Blob::new(Arc::new(std::fs::read(path).unwrap()))
}

fn font_context(files: &[&str]) -> (FontContext, Vec<u64>) {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    let mut ids = Vec::new();
    for file in files {
        let blob = font_blob(file);
        ids.push(blob.id());
        collection.register_fonts(blob, None);
    }
    let fcx = FontContext {
        collection,
        source_cache: SourceCache::default(),
    };
    (fcx, ids)
}

/// Returns the blob id of the font behind each run, consecutive duplicates removed.
fn selected_font_ids(fcx: &mut FontContext, families: &str, text: &str) -> Vec<u64> {
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.ranged_builder(fcx, text, 1.0, true);

    builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
        Cow::Borrowed(families),
    )));

    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    let mut ids = Vec::new();
    for line in layout.lines() {
        for run in line.runs() {
            let id = run.font().font.data.id();
            if ids.last() != Some(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

#[test]
fn variation_selectors_select_presentation_over_stack_order() {
    let (mut fcx, ids) = font_context(&[MONO_FONT, COLOR_FONT]);
    let (mono, color) = (ids[0], ids[1]);

    // U+270C bare, with VS16, and with VS15.
    let text = "\u{270C}\u{270C}\u{FE0F}\u{270C}\u{FE0E}";

    // The bare codepoint follows the stack. VS16 still reaches the color font.
    assert_eq!(
        selected_font_ids(&mut fcx, "Noto Emoji, Noto Color Emoji", text),
        [mono, color, mono],
    );

    // VS15 still reaches the monochrome font.
    assert_eq!(
        selected_font_ids(&mut fcx, "Noto Color Emoji, Noto Emoji", text),
        [color, mono],
    );
}

#[test]
fn mismatched_presentation_still_renders_the_base_character() {
    let (mut fcx, ids) = font_context(&[MONO_FONT]);

    // VS16 with no color font available falls back to the monochrome glyph.
    assert_eq!(
        selected_font_ids(&mut fcx, "Noto Emoji", "\u{270C}\u{FE0F}"),
        [ids[0]],
    );
}

#[test]
fn a_matched_font_without_the_glyph_does_not_shadow_coverage() {
    let (mut fcx, ids) = font_context(&["roboto_fonts/Roboto-Regular.ttf", COLOR_FONT]);

    // Roboto matches the requested text presentation but has no U+270C glyph,
    // so the color font's base glyph still wins over rendering nothing.
    assert_eq!(
        selected_font_ids(&mut fcx, "Roboto, Noto Color Emoji", "\u{270C}\u{FE0E}"),
        [ids[1]],
    );
}

fn charmap_index(fcx: &mut FontContext, family: &str) -> fontique::CharmapIndex {
    let family_id = fcx.collection.family_id(family).unwrap();
    let family = fcx.collection.family(family_id).unwrap();
    family.fonts()[0].charmap_index()
}

#[test]
fn charmap_maps_variation_sequences_through_cmap_format_14() {
    let (mut fcx, _) = font_context(&[CBDT_FONT, MONO_FONT]);

    // The CBDT subset declares its nominal glyphs correct for its FE0F sequences.
    let cbdt = font_blob(CBDT_FONT);
    let charmap = charmap_index(&mut fcx, "Noto Color Emoji CBTF")
        .charmap(cbdt.data())
        .unwrap();
    assert!(matches!(
        charmap.map_variant('\u{2705}', '\u{FE0F}'),
        Some(MapVariant::UseDefault)
    ));
    assert_eq!(charmap.map_variant('\u{2705}', '\u{FE0E}'), None);

    // The monochrome subset has no format 14 subtable.
    let mono = font_blob(MONO_FONT);
    let mono_charmap = charmap_index(&mut fcx, "Noto Emoji")
        .charmap(mono.data())
        .unwrap();
    assert_eq!(mono_charmap.map_variant('\u{270C}', '\u{FE0F}'), None);
}
