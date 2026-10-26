// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stop tests.
//!
//! Tabs shift the following glyph to the next tab stop, following CSS Text 4 (`#tab-stop`). The
//! tests measure the shift against the advance of the space character, as [`TabSize::Spaces`] is
//! a multiple of it.

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{
    Alignment, AlignmentOptions, IndentOptions, Layout, PositionedLayoutItem, StyleProperty,
    TabSize, TextWrapMode, WhiteSpaceCollapse,
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
fn build(env: &mut TestEnv, text: &str, tab_size: TabSize) -> Layout<ColorBrush> {
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

#[test]
fn tab_advances_the_line_to_the_next_stop() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let b = advance(&mut env, "b");
    assert!(a < 8. * space, "the tab stop is past the first letter");

    // The default tab size is eight spaces.
    let layout = build(&mut env, "a\tb", TabSize::default());
    nearly_eq(layout.width(), 8. * space + b);

    let layout = build(&mut env, "a\tb", TabSize::Spaces(4.));
    nearly_eq(layout.width(), 4. * space + b);

    // Once the text is past the first stop, the tab goes to the second.
    let aa = advance(&mut env, "aa");
    assert!(aa > 4. * space && aa < 8. * space - space / 2.);
    let layout = build(&mut env, "aa\tb", TabSize::Spaces(4.));
    nearly_eq(layout.width(), 8. * space + b);

    // Consecutive tabs each advance a stop.
    let layout = build(&mut env, "a\t\t\tb", TabSize::Spaces(4.));
    nearly_eq(layout.width(), 12. * space + b);

    // A tab size of zero renders the tab as nothing.
    let layout = build(&mut env, "a\tb", TabSize::Spaces(0.));
    nearly_eq(layout.width(), advance(&mut env, "ab"));
}

#[test]
fn tab_default_is_eight_spaces() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    assert!(a < 8. * space, "the tab stop is past the first letter");

    let layout = build(&mut env, "a\tb", TabSize::default());
    let [a_x, b_x] = glyph_xs(&layout, 0)[..] else {
        panic!("expected two glyphs");
    };
    nearly_eq(a_x, 0.);
    nearly_eq(b_x, 8. * space);
}

#[test]
fn tab_size_in_spaces() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");

    let layout = build(&mut env, "a\tb", TabSize::Spaces(4.));
    let b_x = glyph_xs(&layout, 0)[1];
    nearly_eq(b_x, 4. * space);

    // Once the text is past the first stop, the tab goes to the second.
    let aa = advance(&mut env, "aa");
    assert!(aa > 4. * space && aa < 8. * space - space / 2.);
    let layout = build(&mut env, "aa\tb", TabSize::Spaces(4.));
    let b_x = glyph_xs(&layout, 0)[2];
    nearly_eq(b_x, 8. * space);
}

#[test]
fn tab_size_as_length() {
    let mut env = TestEnv::new(test_name!(), None);
    let a = advance(&mut env, "a");

    let layout = build(&mut env, "a\tb", TabSize::Length(a + 5.));
    let b_x = glyph_xs(&layout, 0)[1];
    nearly_eq(b_x, a + 5.);
}

#[test]
fn tab_skips_a_stop_less_than_half_a_space_away() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");

    // The first stop is a quarter space past "a", so the tab goes to the second one.
    let tab_size = a + space / 4.;
    let layout = build(&mut env, "a\tb", TabSize::Length(tab_size));
    let b_x = glyph_xs(&layout, 0)[1];
    nearly_eq(b_x, 2. * tab_size);

    // A stop exactly half a space away is used.
    let tab_size = a + space / 2.;
    let layout = build(&mut env, "a\tb", TabSize::Length(tab_size));
    let b_x = glyph_xs(&layout, 0)[1];
    nearly_eq(b_x, tab_size);
}

#[test]
fn tab_consecutive_tabs_each_advance_a_stop() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");

    let layout = build(&mut env, "a\t\t\tb", TabSize::Spaces(4.));
    let b_x = glyph_xs(&layout, 0)[1];
    nearly_eq(b_x, 12. * space);
}

#[test]
fn tab_size_zero_removes_the_tabs_advance() {
    let mut env = TestEnv::new(test_name!(), None);
    let a = advance(&mut env, "a");

    let layout = build(&mut env, "a\tb", TabSize::Spaces(0.));
    let b_x = glyph_xs(&layout, 0)[1];
    nearly_eq(b_x, a);
    nearly_eq(layout.width(), advance(&mut env, "ab"));

    // The tab gets no letter-spacing gap of its own: "b" follows the gap after "a" only.
    let text = "a\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(0.)));
    builder.push_default(StyleProperty::LetterSpacing(2.));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(glyph_xs(&layout, 0)[1], a + 2.);
}

#[test]
fn tab_measures_spaces_of_its_own_font_size() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");

    // The tab is set at twice the font size, so its stops are twice as far apart as the
    // surrounding text's would be.
    let text = "a\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(4.)));
    builder.push(StyleProperty::FontSize(32.), 1..2);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let b_x = glyph_xs(&layout, 0)[1];
    nearly_eq(b_x, 8. * space);
}

#[test]
fn tab_spaces_include_letter_and_word_spacing() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");

    let text = "\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(4.)));
    builder.push_default(StyleProperty::LetterSpacing(1.));
    builder.push_default(StyleProperty::WordSpacing(2.));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let b_x = glyph_xs(&layout, 0)[0];
    // The spacing is part of the unit, not a gap after the tab: "b" sits exactly on the stop.
    nearly_eq(b_x, 4. * (space + 1. + 2.));
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
    builder.push_default(StyleProperty::TabSize(TabSize::Length(tab_size)));
    builder.push_default(StyleProperty::LetterSpacing(10.));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(glyph_xs(&layout, 0)[1], tab_size);

    // The same with word spacing: the space before the tab gets both gaps, the tab gets neither.
    let text = "a \tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Length(tab_size)));
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
}

#[test]
fn tab_stops_are_measured_from_the_indented_line_start() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");

    // With the indent, the first stop is already past "a", and the stop nearest to the tab is
    // less than half a space before it, so the tab goes two stops along.
    let text = "a\tb";
    let indent = 4. * space - 1.;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(4.)));
    let mut layout = builder.build(text);
    layout.set_text_indent(indent, IndentOptions::default());
    layout.break_all_lines(None);
    // Alignment applies the indent to the glyph positions.
    layout.align(Alignment::Start, AlignmentOptions::default());

    let [a_x, b_x] = glyph_xs(&layout, 0)[..] else {
        panic!("expected two glyphs");
    };
    nearly_eq(a_x, indent);
    nearly_eq(b_x, 8. * space);
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
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(8.)));
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
fn tab_sized_when_breaking_by_length() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let ab = advance(&mut env, "ab");
    assert!(ab < 8. * space);

    let text = "ab\tc\tab";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(8.)));
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
fn tab_trailing_hangs_like_a_space() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");

    // Under `WhiteSpaceCollapse::Collapse`, trailing whitespace hangs unconditionally, so the tab
    // is excluded from the layout's width.
    let text = "a\t";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(4.)));
    builder.push_default(StyleProperty::WhiteSpaceCollapse(
        WhiteSpaceCollapse::Collapse,
    ));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(layout.width(), a);
    nearly_eq(layout.full_width(), 4. * space);
    nearly_eq(
        layout.get(0).unwrap().metrics().hanging_advance,
        4. * space - a,
    );

    // Under `WhiteSpaceCollapse::Preserve`, whitespace before the end of the layout hangs only as
    // far as it overflows, like a space would.
    let layout = build(&mut env, "a\t", TabSize::Spaces(4.));
    nearly_eq(layout.width(), 4. * space);
    nearly_eq(layout.full_width(), 4. * space);
}

#[test]
fn tab_columns_align() {
    let mut env = TestEnv::new(test_name!(), None);

    // Each line's second column starts at the first stop a tab reaches: the same stop for the
    // first three lines, and the next one for the line whose first word is wider.
    let text = "1\tfoo\tx\n12\tfoo\tx\n123\tfoo\tx\n1234567\tfoo\tx";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(4.)));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
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
fn tab_stops_in_rtl_text_are_measured_from_the_right_edge() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let base = 4. * space;
    let alefs = advance(&mut env, "\u{627}\u{627}");
    let bas = advance(&mut env, "\u{628}\u{628}\u{628}\u{628}");
    assert!(alefs < base - space / 2. && bas > base && bas < 2. * base - space / 2.);

    // In a right-to-left line the start edge is the right edge, so the logically first word sits
    // at the right and the tab fills out to the first stop measured from there. Visually, the
    // glyph run after the tab must then end exactly one tab size short of the right edge.
    let mut layout = build(
        &mut env,
        "\u{627}\u{627}\t\u{628}\u{628}\u{628}\u{628}",
        TabSize::Spaces(4.),
    );
    layout.align(Alignment::Start, AlignmentOptions::default());
    nearly_eq(layout.width(), base + bas);
    let [(bas_offset, bas_advance), (alefs_offset, alefs_advance)] =
        glyph_run_extents(&layout, 0)[..]
    else {
        panic!("expected two glyph runs split by the tab");
    };
    nearly_eq(bas_offset, 0.);
    nearly_eq(bas_advance, bas);
    nearly_eq(alefs_offset, layout.width() - alefs);
    nearly_eq(alefs_advance, alefs);
    nearly_eq(alefs_offset - (bas_offset + bas_advance), base - alefs);

    // A first word wider than one tab size pushes the tab to the second stop from the right edge.
    let mut layout = build(
        &mut env,
        "\u{628}\u{628}\u{628}\u{628}\t\u{627}\u{627}",
        TabSize::Spaces(4.),
    );
    layout.align(Alignment::Start, AlignmentOptions::default());
    nearly_eq(layout.width(), 2. * base + alefs);
    let [(alefs_offset, _), (bas_offset, _)] = glyph_run_extents(&layout, 0)[..] else {
        panic!("expected two glyph runs split by the tab");
    };
    nearly_eq(alefs_offset, 0.);
    nearly_eq(bas_offset, layout.width() - bas);
}

#[test]
fn tab_sized_after_newline_when_breaking_by_length() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let ab_cd = advance(&mut env, "abcd");
    let base = 20. * space;
    assert!(ab_cd < base - space / 2.);

    // `break_next_with_length` does not break at newlines, so the newline and the tab share a
    // line. The newline has no advance, so the tab starts after "ab" + "cd" and must reach the
    // first stop.
    let text = "ab\ncd\tef";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(20.)));
    let mut layout = builder.build(text);
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(100);
    breaker.finish();

    assert_eq!(layout.len(), 1);
    // Glyphs in order: a b c d e f; "e" shows where the tab landed.
    nearly_eq(glyph_xs(&layout, 0)[4], base);
}

#[test]
fn tab_stops_survive_justification() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let b = advance(&mut env, "b");
    let base = 8. * space;
    assert!(2. * a + space < base - space / 2.);
    // Justify every line, including the last, so a single line suffices.
    let justify_all = AlignmentOptions {
        last_line_alignment: Some(Alignment::Justify),
        ..AlignmentOptions::default()
    };

    // The space before the tab isn't a justification opportunity, as stretching it would move
    // "b" off its stop. The space after the tab takes up all of the free space instead.
    // Glyphs in order: a, space, a, b, space, b.
    let text = "a a\tb b";
    let width = 200.;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(8.)));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(width));
    layout.align(Alignment::Justify, justify_all);
    assert_eq!(layout.len(), 1);
    let xs = glyph_xs(&layout, 0);
    nearly_eq(xs[0], 0.);
    nearly_eq(xs[2], a + space);
    nearly_eq(xs[3], base);
    nearly_eq(xs[5], width - b);

    // With no opportunity after the tab, the line isn't justified at all.
    let text = "a a\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(8.)));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(width));
    layout.align(Alignment::Justify, justify_all);
    let xs = glyph_xs(&layout, 0);
    nearly_eq(xs[0], 0.);
    nearly_eq(xs[2], a + space);
    nearly_eq(xs[3], base);
}

#[test]
fn justification_keeps_every_tab_on_its_stop() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let b = advance(&mut env, "b");
    let c = advance(&mut env, "c");
    let base = 8. * space;
    assert!(a < base - space / 2. && base + b + space + b < 2. * base - space / 2.);
    let justify_all = AlignmentOptions {
        last_line_alignment: Some(Alignment::Justify),
        ..AlignmentOptions::default()
    };

    // Only the space after the last tab stretches, so both tabs stay on their stops and the
    // space between them keeps its width. Glyphs: a, b, space, b, c, space, c.
    let text = "a\tb b\tc c";
    let width = 200.;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(8.)));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(width));
    layout.align(Alignment::Justify, justify_all);
    let xs = glyph_xs(&layout, 0);
    nearly_eq(xs[1], base);
    nearly_eq(xs[3], base + b + space);
    nearly_eq(xs[4], 2. * base);
    nearly_eq(xs[6], width - c);
}

#[test]
fn justification_with_a_hanging_tab() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let justify_all = AlignmentOptions {
        last_line_alignment: Some(Alignment::Justify),
        ..AlignmentOptions::default()
    };

    // The trailing space and tab hang past the line's end. The space precedes the line's last tab,
    // so it was never a justification opportunity and must not be subtracted as a hanging one.
    let text = "a a \t";
    let width = 2. * a + space + 0.5;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(8.)));
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(width));
    layout.align(Alignment::Justify, justify_all);
    assert_eq!(layout.len(), 1);
    let xs = glyph_xs(&layout, 0);
    nearly_eq(xs[0], 0.);
    nearly_eq(xs[2], a + space);
}
