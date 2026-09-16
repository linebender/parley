// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for `StyleProperty::GraphemeReplacement` (e.g. password masking).

use crate::{test_name, util::TestEnv};
use parley::{Cluster, Layout, StyleProperty};

use crate::util::ColorBrush;

const BULLET: char = '\u{2022}';

/// Collects the glyph ids of every glyph in the layout, in visual order.
fn glyph_ids(layout: &Layout<ColorBrush>) -> Vec<u32> {
    layout
        .lines()
        .flat_map(|line| {
            line.runs()
                .flat_map(|run| run.visual_clusters().flat_map(|c| c.glyphs().map(|g| g.id)))
        })
        .collect()
}

/// Build a masked layout of `text`.
fn masked_layout(env: &mut TestEnv, text: &str) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::GraphemeReplacement(Some(BULLET)));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout
}

/// Masking renders exactly one replacement glyph per grapheme, regardless of ligatures,
/// combining marks, whitespace, emoji or text direction, and all glyphs are the same glyph.
#[test]
fn masking_one_glyph_per_grapheme() {
    let mut env = TestEnv::new(test_name!(), None);
    let bullet = masked_layout(&mut env, "\u{2022}");
    let bullet_ids = glyph_ids(&bullet);
    assert_eq!(bullet_ids.len(), 1);
    let bullet_id = bullet_ids[0];
    let bullet_advance = bullet.width();

    for (text, graphemes) in [
        ("abcde", 5),
        // Ligature: unmasked this shapes into a single glyph.
        ("fi", 2),
        ("office", 6),
        // Combining mark: unmasked this shapes into two glyphs for one grapheme.
        ("e\u{301}x", 2),
        // Whitespace is masked as well.
        ("a b", 3),
        // Emoji would fall back to an emoji font, which doesn't have a bullet glyph.
        ("\u{1f440}a", 2),
        // Emoji ZWJ sequence: one grapheme.
        ("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}", 1),
        // RTL.
        ("\u{5e9}\u{5dc}\u{5d5}\u{5dd}", 4),
        // Arabic joining forms.
        ("\u{644}\u{627}\u{62d}", 3),
        // Mixed direction.
        ("ab \u{5e9}\u{5dc} cd", 8),
    ] {
        let layout = masked_layout(&mut env, text);
        let ids = glyph_ids(&layout);
        assert_eq!(
            ids.len(),
            graphemes,
            "{text:?}: expected one glyph per grapheme, got {ids:?}"
        );
        assert!(
            ids.iter().all(|&id| id == bullet_id),
            "{text:?}: all glyphs should be the replacement glyph, got {ids:?}"
        );

        // Every run uses a font that has the replacement glyph, so all bullets are equally wide.
        assert!(
            (layout.width() - bullet_advance * graphemes as f32).abs() < 0.01,
            "{text:?}: layout width should be {graphemes} bullets wide"
        );

        // The underlying text is preserved: every byte still maps to a cluster.
        for byte in 0..text.len() {
            if text.is_char_boundary(byte) {
                assert!(
                    Cluster::from_byte_index(&layout, byte).is_some(),
                    "{text:?}: byte {byte} has no cluster"
                );
            }
        }
    }
}

/// Masking only part of the text masks exactly that part.
#[test]
fn masking_partial_range() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "abcdef";

    let mut builder = env.ranged_builder(text);
    builder.push(StyleProperty::GraphemeReplacement(Some(BULLET)), 2..4);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    let mut unmasked = env.ranged_builder(text).build(text);
    unmasked.break_all_lines(None);
    let unmasked_ids = glyph_ids(&unmasked);

    let bullet_id = glyph_ids(&masked_layout(&mut env, "\u{2022}"))[0];
    let ids = glyph_ids(&layout);
    assert_eq!(ids.len(), 6);
    assert_eq!(&ids[..2], &unmasked_ids[..2]);
    assert_eq!(&ids[2..4], &[bullet_id, bullet_id]);
    assert_eq!(&ids[4..], &unmasked_ids[4..]);
}

/// `PlainEditor::set_password` masks the text while editing continues to work on the real text.
#[test]
fn masking_editor_password() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hunter2");
    editor.set_password(true);
    assert!(editor.is_password());

    let mut driver = env.driver(&mut editor);
    driver.move_to_text_end();
    driver.insert_or_replace_selection("!");
    driver.select_all();
    let layout = driver.editor.layout(driver.font_cx, driver.layout_cx);
    let ids = glyph_ids(layout);
    assert_eq!(ids.len(), 8);
    assert!(ids.iter().all(|&id| id == ids[0]));
    assert_eq!(editor.text(), "hunter2!");
    assert_eq!(editor.selected_text(), Some("hunter2!"));
    env.check_editor_snapshot(&mut editor);
}
