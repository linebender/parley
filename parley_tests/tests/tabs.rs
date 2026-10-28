// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stop tests.
//!
//! Tabs shift the following glyph to the next tab stop, following CSS `tab-size`. The tab size is
//! a length; the tests express it as a multiple of the advance of a space, as CSS does, so that
//! the expected positions can be read as "n spaces in".

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{
    Alignment, AlignmentOptions, Cluster, ClusterSide, IndentOptions, Layout, PositionedLayoutItem,
    StyleProperty, TextWrapMode, WhiteSpaceCollapse,
};

fn nearly_eq(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-3, "{actual} != {expected}");
}

/// The advance of `text` on a single line, with none of it hanging.
fn advance(env: &mut TestEnv, text: &str) -> f32 {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::WhiteSpaceCollapse(
        WhiteSpaceCollapse::Preserve,
    ));
    builder.push_default(StyleProperty::TextWrapMode(TextWrapMode::NoWrap));
    let mut layout: Layout<ColorBrush> = builder.build(text);
    layout.break_all_lines(None);
    layout.full_width()
}

/// Lays out `text` on one line, with the given tab size.
fn build(env: &mut TestEnv, text: &str, tab_size: f32) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout
}

/// The x positions of the glyphs on line `line_index`, in visual order.
///
/// Tabs have no glyphs, so they don't appear here; the glyph after a tab shows where it landed.
fn glyph_xs(layout: &Layout<ColorBrush>, line_index: usize) -> Vec<f32> {
    let line = layout.get(line_index).expect("line exists");
    line.items()
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(run) => Some(run),
            PositionedLayoutItem::InlineBox(_) => None,
        })
        .flat_map(|run| {
            run.positioned_glyphs()
                .map(|glyph| glyph.x)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The glyph runs on line `line_index` as `(offset, advance)`, in visual order.
fn glyph_run_extents(layout: &Layout<ColorBrush>, line_index: usize) -> Vec<(f32, f32)> {
    let line = layout.get(line_index).expect("line exists");
    line.items()
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(run) => Some((run.offset(), run.advance())),
            PositionedLayoutItem::InlineBox(_) => None,
        })
        .collect()
}

#[test]
fn tab_advances_the_line_to_the_next_stop() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let b = advance(&mut env, "b");
    assert!(a < 4. * space, "the first stop is past the first letter");

    let layout = build(&mut env, "a\tb", 8. * space);
    nearly_eq(layout.width(), 8. * space + b);
    let [a_x, b_x] = glyph_xs(&layout, 0)[..] else {
        panic!("expected two glyphs");
    };
    nearly_eq(a_x, 0.);
    nearly_eq(b_x, 8. * space);

    let layout = build(&mut env, "a\tb", 4. * space);
    nearly_eq(layout.width(), 4. * space + b);
    nearly_eq(glyph_xs(&layout, 0)[1], 4. * space);

    // Once the text is past the first stop, the tab goes to the second.
    let aa = advance(&mut env, "aa");
    assert!(aa > 4. * space && aa < 8. * space - space / 2.);
    let layout = build(&mut env, "aa\tb", 4. * space);
    nearly_eq(layout.width(), 8. * space + b);
    nearly_eq(glyph_xs(&layout, 0)[2], 8. * space);

    // Consecutive tabs each advance a stop.
    let layout = build(&mut env, "a\t\t\tb", 4. * space);
    nearly_eq(layout.width(), 12. * space + b);
    nearly_eq(glyph_xs(&layout, 0)[1], 12. * space);
}

#[test]
fn tab_size_is_a_length_in_layout_units() {
    let mut env = TestEnv::new(test_name!(), None);
    let a = advance(&mut env, "a");

    let layout = build(&mut env, "a\tb", a + 5.);
    nearly_eq(glyph_xs(&layout, 0)[1], a + 5.);
}

#[test]
fn tab_default_size_gives_no_advance() {
    let mut env = TestEnv::new(test_name!(), None);
    let a = advance(&mut env, "a");

    // Parley can't measure the CSS initial value (eight spaces of the block container's font)
    // yet, so until a tab size is set, a tab takes no space.
    let text = "a\tb";
    let mut layout = env.ranged_builder(text).build(text);
    layout.break_all_lines(None);
    nearly_eq(layout.width(), advance(&mut env, "ab"));
    let [a_x, b_x] = glyph_xs(&layout, 0)[..] else {
        panic!("expected two glyphs");
    };
    nearly_eq(a_x, 0.);
    nearly_eq(b_x, a);

    // Explicitly setting a tab size that isn't positive does the same.
    for tab_size in [0., -10.] {
        let layout = build(&mut env, "a\tb", tab_size);
        nearly_eq(glyph_xs(&layout, 0)[1], a);
    }
}

#[test]
fn tab_skips_a_stop_less_than_a_sixteenth_of_the_tab_size_away() {
    let mut env = TestEnv::new(test_name!(), None);
    let a = advance(&mut env, "a");

    // The first stop is a thirty-second of the tab size past "a", so the tab goes to the second.
    let tab_size = a * 32. / 31.;
    let layout = build(&mut env, "a\tb", tab_size);
    nearly_eq(glyph_xs(&layout, 0)[1], 2. * tab_size);

    // A stop a bit more than a sixteenth away is used.
    let tab_size = a * 16. / 15. + 0.01;
    let layout = build(&mut env, "a\tb", tab_size);
    nearly_eq(glyph_xs(&layout, 0)[1], tab_size);
}

#[test]
fn tab_gets_no_letter_or_word_spacing_of_its_own() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let tab_size = 8. * space;
    assert!(a + 10. < tab_size);

    // Letter spacing applies after "a", but not after the tab, so "b" lands exactly on the stop.
    // (Chrome agrees: with `tab-size: 4; letter-spacing: 10px`, the glyph after a tab sits on
    // the stop.)
    let text = "a\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    builder.push_default(StyleProperty::LetterSpacing(10.));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(glyph_xs(&layout, 0)[1], tab_size);

    // The same with word spacing: the space before the tab gets both gaps, the tab gets neither.
    let text = "a \tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    builder.push_default(StyleProperty::LetterSpacing(10.));
    builder.push_default(StyleProperty::WordSpacing(10.));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let xs = glyph_xs(&layout, 0);
    nearly_eq(xs[1], a + 10.);
    assert!(
        a + 10. + space + 20. > tab_size,
        "the space pushes past the first stop"
    );
    nearly_eq(xs[2], 2. * tab_size);

    // Without a tab size, the tab still gets no gap: "b" follows the gap after "a" only.
    let text = "a\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::LetterSpacing(2.));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(glyph_xs(&layout, 0)[1], a + 2.);
}

#[test]
fn tab_stops_are_measured_from_the_indented_line_start() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let b = advance(&mut env, "b");
    let tab_size = 4. * space;

    // With the indent, the first stop is already past "a", and the stop nearest to the tab is
    // less than a sixteenth of the tab size before it, so the tab goes two stops along.
    let text = "a\tb";
    let indent = tab_size - space / 8.;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    let mut layout = builder.build(text);
    layout.set_text_indent(indent, IndentOptions::default());
    layout.break_all_lines(None);
    // Alignment applies the indent to the glyph positions.
    layout.align(Alignment::Start, AlignmentOptions::default());
    let [a_x, b_x] = glyph_xs(&layout, 0)[..] else {
        panic!("expected two glyphs");
    };
    nearly_eq(a_x, indent);
    nearly_eq(b_x, 2. * tab_size);

    // Under a negative indent, the stops are still at multiples of the tab size from the
    // unindented edge: a tab a space before it advances just that space.
    let text = "\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    let mut layout = builder.build(text);
    layout.set_text_indent(-space, IndentOptions::default());
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    nearly_eq(layout.get(0).unwrap().metrics().advance, space + b);
    nearly_eq(glyph_xs(&layout, 0)[0], 0.);
}

#[test]
fn tab_resized_when_relayout_moves_it() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let aaa = advance(&mut env, "aaa");
    let aaa_aaa = advance(&mut env, "aaa aaa");
    assert!(aaa < 8. * space && aaa_aaa > 8. * space && aaa_aaa < 16. * space - space / 2.);

    let text = "aaa aaa\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(8. * space));
    let mut layout = builder.build(text);

    // On one line, the tab is past the first stop.
    layout.break_all_lines(None);
    assert_eq!(layout.len(), 1);
    nearly_eq(glyph_xs(&layout, 0)[7], 16. * space);

    // Wrapped after the space, the tab is before the first stop, so it must be sized again at
    // its new position. (The width is chosen so that the second "aaa" doesn't fit, as the tab
    // itself would hang off the end of the first line.)
    layout.break_all_lines(Some(aaa_aaa - 1.));
    assert_eq!(layout.len(), 2);
    nearly_eq(glyph_xs(&layout, 1)[3], 8. * space);

    // Relaid out on one line, the tab is past the first stop again.
    layout.break_all_lines(None);
    assert_eq!(layout.len(), 1);
    nearly_eq(glyph_xs(&layout, 0)[7], 16. * space);
}

#[test]
fn tab_placed_before_a_taken_break_is_sized_on_the_next_line() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let aaa = advance(&mut env, "aaa");
    let aaa_aaa = advance(&mut env, "aaa aaa");
    let tab_size = 8. * space;
    assert!(aaa < tab_size && aaa_aaa > tab_size);

    // "aaa aaa" fits, and the tab after it hangs, so the breaker places the tab on the first
    // line before finding that "!!!" doesn't fit and backtracking to the break after the space.
    // (A tab is a break opportunity itself, but not before "!", UAX #14 LB13.) The tab must then
    // be sized from the start of the second line, not keep the advance it was given on the
    // first.
    let text = "aaa aaa\t!!!";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(aaa_aaa + 1.));
    assert_eq!(layout.len(), 2);
    let first = layout.get(0).unwrap();
    assert_eq!(&text[first.text_range()], "aaa ");
    nearly_eq(first.metrics().advance, aaa + space);
    nearly_eq(first.metrics().hanging_advance, space);
    let second = layout.get(1).unwrap();
    nearly_eq(
        second.metrics().advance,
        tab_size + advance(&mut env, "!!!"),
    );
    nearly_eq(glyph_xs(&layout, 1)[3], tab_size);
}

#[test]
fn tab_sized_when_breaking_by_length() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let ab = advance(&mut env, "ab");
    assert!(ab < 8. * space);

    let text = "ab\tc\tab";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(8. * space));
    let mut layout = builder.build(text);
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(3);
    breaker.break_next_with_length(4);
    breaker.finish();

    assert_eq!(layout.len(), 2);
    // The first line is "ab\t", with the tab hanging off the end of the wrapped line.
    let line = layout.get(0).unwrap();
    nearly_eq(line.metrics().advance, 8. * space);
    nearly_eq(line.metrics().hanging_advance, 8. * space - ab);
    // The second line starts with "c" at the line start, then a tab to the first stop.
    nearly_eq(glyph_xs(&layout, 1)[1], 8. * space);
}

#[test]
fn tab_sized_after_newline_when_breaking_by_length() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let ab_cd = advance(&mut env, "abcd");
    let tab_size = 20. * space;
    assert!(ab_cd < tab_size - space / 2.);

    // `break_next_with_length` does not break at newlines, so the newline and the tab share a
    // line. The newline has no advance, so the tab starts after "ab" + "cd" and must reach the
    // first stop.
    let text = "ab\ncd\tef";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    let mut layout = builder.build(text);
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(100);
    breaker.finish();

    assert_eq!(layout.len(), 1);
    // Glyphs in order: a b c d e f; "e" shows where the tab landed.
    nearly_eq(glyph_xs(&layout, 0)[4], tab_size);
}

#[test]
fn tab_trailing_hangs_like_a_space() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let tab_size = 4. * space;

    // Under `WhiteSpaceCollapse::Collapse`, trailing whitespace hangs unconditionally, so the tab
    // is excluded from the layout's width.
    let text = "a\t";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    builder.push_default(StyleProperty::WhiteSpaceCollapse(
        WhiteSpaceCollapse::Collapse,
    ));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(layout.width(), a);
    nearly_eq(layout.full_width(), tab_size);
    nearly_eq(
        layout.get(0).unwrap().metrics().hanging_advance,
        tab_size - a,
    );

    // Under `WhiteSpaceCollapse::Preserve`, whitespace before the end of the layout hangs only as
    // far as it overflows, like a space would.
    let layout = build(&mut env, "a\t", tab_size);
    nearly_eq(layout.width(), tab_size);
    nearly_eq(layout.full_width(), tab_size);
}

#[test]
fn tab_advance_is_visible_to_clusters_and_hit_testing() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let tab_size = 8. * space;

    let layout = build(&mut env, "a\tb", tab_size);
    let tab = Cluster::from_byte_index(&layout, 1).unwrap();
    assert_eq!(tab.text_range(), 1..2);
    nearly_eq(tab.advance(), tab_size - a);
    nearly_eq(tab.visual_offset().unwrap(), a);
    let b = Cluster::from_byte_index(&layout, 2).unwrap();
    nearly_eq(b.visual_offset().unwrap(), tab_size);

    // A point in the right half of the tab hits the tab, not the text around it.
    let (hit, side) = Cluster::from_point(&layout, tab_size - 1., 1.).unwrap();
    assert_eq!(hit.text_range(), 1..2);
    assert_eq!(side, ClusterSide::Right);
}

#[test]
fn tab_columns_align() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");

    // Each line's second column starts at the first stop a tab reaches: the same stop for the
    // first three lines, and the next one for the line whose first word is wider.
    let text = "1\tfoo\tx\n12\tfoo\tx\n123\tfoo\tx\n1234567\tfoo\tx";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(4. * space));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

#[test]
fn tab_stops_in_rtl_text_are_measured_from_the_right_edge() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let tab_size = 4. * space;
    let alefs = advance(&mut env, "\u{627}\u{627}");
    let bas = advance(&mut env, "\u{628}\u{628}\u{628}\u{628}");
    assert!(alefs < tab_size - space / 2. && bas > tab_size && bas < 2. * tab_size - space / 2.);

    // In a right-to-left line the start edge is the right edge, so the logically first word sits
    // at the right and the tab fills out to the first stop measured from there. Visually, the
    // glyph run after the tab must then end exactly one tab size short of the right edge.
    let mut layout = build(
        &mut env,
        "\u{627}\u{627}\t\u{628}\u{628}\u{628}\u{628}",
        tab_size,
    );
    layout.align(Alignment::Start, AlignmentOptions::default());
    nearly_eq(layout.width(), tab_size + bas);
    let [(bas_offset, bas_advance), (alefs_offset, alefs_advance)] =
        glyph_run_extents(&layout, 0)[..]
    else {
        panic!("expected two glyph runs split by the tab");
    };
    nearly_eq(bas_offset, 0.);
    nearly_eq(bas_advance, bas);
    nearly_eq(alefs_offset, layout.width() - alefs);
    nearly_eq(alefs_advance, alefs);
    nearly_eq(alefs_offset - (bas_offset + bas_advance), tab_size - alefs);

    // A first word wider than one tab size pushes the tab to the second stop from the right edge.
    let mut layout = build(
        &mut env,
        "\u{628}\u{628}\u{628}\u{628}\t\u{627}\u{627}",
        tab_size,
    );
    layout.align(Alignment::Start, AlignmentOptions::default());
    nearly_eq(layout.width(), 2. * tab_size + alefs);
    let [(alefs_offset, _), (bas_offset, _)] = glyph_run_extents(&layout, 0)[..] else {
        panic!("expected two glyph runs split by the tab");
    };
    nearly_eq(alefs_offset, 0.);
    nearly_eq(bas_offset, layout.width() - bas);
}

#[test]
fn justification_stretches_spaces_around_a_tab_but_not_the_tab() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let b = advance(&mut env, "b");
    let tab_size = 8. * space;
    assert!(2. * a + space < tab_size - space / 2.);
    // Justify every line, including the last, so a single line suffices.
    let justify_all = AlignmentOptions {
        last_line_alignment: Some(Alignment::Justify),
        ..AlignmentOptions::default()
    };

    // Tab-aware justification is deferred: the tab keeps the advance the breaker gave it (to the
    // first stop) and only the spaces stretch, each by the same amount, so the text after the
    // tab moves off its stop by that amount. Glyphs in order: a, space, a, b, space, b.
    let text = "a a\tb b";
    let width = 200.;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(width));
    layout.align(Alignment::Justify, justify_all);
    assert_eq!(layout.len(), 1);
    let natural_width = tab_size + b + space + b;
    let share = (width - natural_width) / 2.;
    let xs = glyph_xs(&layout, 0);
    nearly_eq(xs[0], 0.);
    nearly_eq(xs[2], a + space + share);
    nearly_eq(xs[3], tab_size + share);
    nearly_eq(xs[5], width - b);

    // Trailing whitespace hangs (in full under `Collapse`) and isn't stretched, with a tab in it
    // or not; the one space inside the line takes up all of the free space.
    let text = "a a \t";
    let width = 2. * a + space + 0.5;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    builder.push_default(StyleProperty::WhiteSpaceCollapse(
        WhiteSpaceCollapse::Collapse,
    ));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(width));
    layout.align(Alignment::Justify, justify_all);
    assert_eq!(layout.len(), 1);
    let xs = glyph_xs(&layout, 0);
    nearly_eq(xs[0], 0.);
    nearly_eq(xs[2], width - a);
}
