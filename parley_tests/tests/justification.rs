// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Automatic CJ justification. See `parley_tests/justification.html` for manual Chrome comparisons.

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{
    Affinity, Alignment, AlignmentOptions, Cursor, FontFamily, Layout, PositionedLayoutItem,
    Selection, StyleProperty,
};

const JUSTIFY_LAST: AlignmentOptions = AlignmentOptions {
    align_when_overflowing: false,
    last_line_alignment: Some(Alignment::Justify),
};

fn build(env: &mut TestEnv, text: &str, underline_first: bool) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(FontFamily::from(
        "Roboto, 'Noto Sans CJK SC', 'Noto Kufi Arabic', 'Noto Emoji'",
    ));
    builder.push_default(StyleProperty::FontSize(16.));
    if underline_first {
        builder.push(
            StyleProperty::Underline(true),
            0..text.chars().next().unwrap().len_utf8(),
        );
    }
    builder.build(text)
}

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}

fn glyphs(layout: &Layout<ColorBrush>) -> Vec<parley::Glyph> {
    layout
        .lines()
        .flat_map(|line| line.items())
        .flat_map(|item| match item {
            PositionedLayoutItem::GlyphRun(run) => run.positioned_glyphs().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn justification_cj() {
    let mut env = TestEnv::new(test_name!(), None);
    for (name, text, expands) in [
        ("pair", "漢字", true),
        ("space", "漢 字", true),
        ("latin_edges", "a漢字b", true),
        ("triple", "漢字典", true),
        ("ideographic_space", "漢　字", true),
        ("ideographic_space_latin", "a　b", true),
        ("ideographic_space_trailing", "漢　", false),
        ("punctuation", "漢、。字", true),
        ("trailing_space", "漢字 ", true),
        ("nbsp", "漢\u{a0}字", true),
        ("trailing_nbsp", "漢字\u{a0}", true),
        ("ignorable", "漢\u{2060}字", true),
        ("all_ignorable", "\u{2060}", false),
        ("no_opportunity", "漢", false),
        ("ligature", "fi漢", true),
        ("bidi", "漢مرحبا字", true),
        ("rtl_no_opportunity", "مرحبا", false),
        // Emoji-presented graphemes are expandable (by default like 👀, or via U+FE0F).
        // Text-presented pictographs are only expandable if Wide (👀 is Wide; ✌ is not).
        ("emoji", "a👀b", true),
        ("emoji_text_selector", "a👀\u{fe0e}b", true),
        ("emoji_selector", "a✌\u{fe0f}b", true),
        ("text_pictograph", "a✌b", false),
    ] {
        let mut layout = build(&mut env, text, false);
        layout.break_all_lines(None);
        let natural = layout.get(0).unwrap().metrics().advance;
        let width = (natural * 2.).max(16.);
        layout.set_alignment(Alignment::Justify, JUSTIFY_LAST);
        layout.break_all_lines(Some(width));
        assert_eq!(layout.len(), 1, "{text:?}");
        if expands {
            let line = layout.get(0).unwrap();
            let advance: f32 = line.runs().map(|run| run.advance()).sum();
            close(advance - line.metrics().hanging_advance, width);
        }
        if name == "ligature" {
            assert_eq!(
                glyphs(&layout).len(),
                2,
                "fi must remain a single ligature glyph"
            );
        }
        env.with_name(name).check_layout_snapshot(&layout);
    }
}

#[test]
fn justification_cj_halfway_geometry() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut layout = build(&mut env, "漢字", true);
    layout.break_all_lines(None);
    let natural_glyphs = glyphs(&layout);
    assert_eq!(natural_glyphs.len(), 2);
    assert!(natural_glyphs.iter().all(|glyph| glyph.id != 0));
    close(natural_glyphs[0].advance, 16.);
    close(natural_glyphs[1].advance, 16.);

    // One opportunity between two 16px glyphs in 52px leaves 20px free, split 10px to each side.
    layout.set_alignment(Alignment::Justify, JUSTIFY_LAST);
    layout.break_all_lines(Some(52.));
    let positioned = glyphs(&layout);
    close(positioned[0].x, 0.);
    close(positioned[0].advance, 26.);
    close(positioned[1].x, 36.);
    close(positioned[1].x + natural_glyphs[1].advance, 52.);
    let clusters: Vec<_> = layout
        .get(0)
        .unwrap()
        .runs()
        .flat_map(|run| run.clusters())
        .collect();
    assert_eq!(clusters.len(), 2);
    close(clusters[0].advance(), 26.);
    close(clusters[1].advance(), 26.);

    let caret = Cursor::from_byte_index(&layout, 3, Affinity::Downstream);
    let cursor = caret.geometry(&layout, 0.);
    close(cursor.x0 as f32, 26.);
    let selection = Selection::new(
        Cursor::from_byte_index(&layout, 0, Affinity::Downstream),
        caret,
    )
    .geometry(&layout);
    assert_eq!(selection.len(), 1);
    close(selection[0].0.x0 as f32, 0.);
    close(selection[0].0.x1 as f32, 26.);
    let runs: Vec<_> = layout
        .get(0)
        .unwrap()
        .items()
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(run) => Some(run),
            _ => None,
        })
        .collect();
    assert_eq!(runs.len(), 2);
    assert!(runs[0].style().underline.is_some());
    // First run's advance determines its underline extent.
    close(runs[0].offset() + runs[0].advance(), 26.);
    close(runs[1].offset(), 26.);
    env.render_and_check_snapshot(&layout, Some(caret.geometry(&layout, 1.)), &selection);

    // Switching alignment and re-justifying produces the same positions.
    layout.set_alignment(Alignment::Start, AlignmentOptions::default());
    layout.break_all_lines(Some(52.));
    assert_ne!(glyphs(&layout), positioned);
    layout.set_alignment(Alignment::Justify, JUSTIFY_LAST);
    layout.break_all_lines(Some(52.));
    assert_eq!(glyphs(&layout), positioned);
}
