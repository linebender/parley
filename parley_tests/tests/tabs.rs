// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stop tests.
//!
//! Tabs shift the following glyph to the next tab stop, following CSS Text 4 § 4.1.3. The tests
//! measure the shift against the advance of the space character, as [`TabSize::Spaces`] is a
//! multiple of it.

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
fn tab_size_zero_renders_nothing() {
    let mut env = TestEnv::new(test_name!(), None);
    let a = advance(&mut env, "a");

    let layout = build(&mut env, "a\tb", TabSize::Spaces(0.));
    let b_x = glyph_xs(&layout, 0)[1];
    nearly_eq(b_x, a);
    nearly_eq(layout.width(), advance(&mut env, "ab"));
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
    // The tab itself gets letter spacing after it, like any other character.
    nearly_eq(b_x, 4. * (space + 1. + 2.) + 1.);
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
