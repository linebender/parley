// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stop tests.
//!
//! Tabs shift the following glyph to the next tab stop, following CSS `tab-size`. Each test says
//! which failure it guards against and where its expected numbers come from: CSS Text 4, a Chrome
//! measurement, or an invariant of Parley's own. The snapshot tests carry most of the value; the
//! numeric tests pin down one rule each.
//!
//! The tab size is a length. The tests express it as a multiple of the advance of a space, as CSS
//! does, so that the expected positions can be read as "n spaces in".

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{
    Alignment, AlignmentOptions, Cluster, ClusterSide, IndentOptions, InlineBox, InlineBoxKind,
    Layout, PositionedLayoutItem, StyleProperty, TabSize, TextWrapMode, VerticalAlign,
    WhiteSpaceCollapse, YieldData,
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

/// Tab stops every `tab_size`, with a skip threshold of half a space as in Chrome, so that the
/// tests can reason in spaces: the tab after text `n` spaces in goes to the stop past `n`.
fn stops(tab_size: f32, space: f32) -> TabSize {
    TabSize::new(tab_size).with_skip_threshold(space / 2.)
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

/// Guards against the tab's advance getting lost between the line breaker and the glyph
/// positions, and against stops not being multiples of the tab size. Source: CSS Text 4, tab
/// stops are at multiples of the tab size from the line's start edge.
#[test]
fn tab_advances_the_line_to_the_next_stop() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let b = advance(&mut env, "b");
    assert!(a < 4. * space, "the first stop is past the first letter");

    let layout = build(&mut env, "a\tb", stops(8. * space, space));
    nearly_eq(layout.width(), 8. * space + b);
    let [a_x, b_x] = glyph_xs(&layout, 0)[..] else {
        panic!("expected two glyphs");
    };
    nearly_eq(a_x, 0.);
    nearly_eq(b_x, 8. * space);

    let layout = build(&mut env, "a\tb", stops(4. * space, space));
    nearly_eq(layout.width(), 4. * space + b);
    nearly_eq(glyph_xs(&layout, 0)[1], 4. * space);

    // Once the text is past the first stop, the tab goes to the second.
    let aa = advance(&mut env, "aa");
    assert!(aa > 4. * space && aa < 8. * space - space / 2.);
    let layout = build(&mut env, "aa\tb", stops(4. * space, space));
    nearly_eq(layout.width(), 8. * space + b);
    nearly_eq(glyph_xs(&layout, 0)[2], 8. * space);

    // Consecutive tabs each advance a stop.
    let layout = build(&mut env, "a\t\t\tb", stops(4. * space, space));
    nearly_eq(layout.width(), 12. * space + b);
    nearly_eq(glyph_xs(&layout, 0)[1], 12. * space);
}

/// Guards against the default tab size changing silently, and against a tab size that isn't
/// positive breaking the arithmetic in `tab_advance`. Source: the `StyleProperty::TabSize` docs.
#[test]
fn tab_default_size_gives_no_advance() {
    let mut env = TestEnv::new(test_name!(), None);
    let a = advance(&mut env, "a");

    // Until a tab size is set, a tab takes no space.
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
        let layout = build(&mut env, "a\tb", TabSize::new(tab_size));
        nearly_eq(glyph_xs(&layout, 0)[1], a);
    }
}

/// Guards against the skip threshold being dropped, resolved against the wrong font size, or
/// not following the root style's font size. Source: CSS Text 4 (a stop less than half a `ch`
/// away is skipped) and Chrome, which measures its threshold with the block container's font.
#[test]
fn tab_skips_a_stop_closer_than_the_skip_threshold() {
    let mut env = TestEnv::new(test_name!(), None);
    let a = advance(&mut env, "a");

    // By default the skip threshold is half the root font size (8 here): a stop 7 units past "a"
    // is skipped for the one after it, a stop 9 units past "a" is used.
    let tab_size = a + 7.;
    let layout = build(&mut env, "a\tb", TabSize::new(tab_size));
    nearly_eq(glyph_xs(&layout, 0)[1], 2. * tab_size);
    let tab_size = a + 9.;
    let layout = build(&mut env, "a\tb", TabSize::new(tab_size));
    nearly_eq(glyph_xs(&layout, 0)[1], tab_size);

    // The threshold can be set explicitly.
    let tab_size = a + 7.;
    let layout = build(
        &mut env,
        "a\tb",
        TabSize::new(tab_size).with_skip_threshold(6.),
    );
    nearly_eq(glyph_xs(&layout, 0)[1], tab_size);
    let layout = build(
        &mut env,
        "a\tb",
        TabSize::new(tab_size).with_skip_threshold(7.5),
    );
    nearly_eq(glyph_xs(&layout, 0)[1], 2. * tab_size);

    // The default follows the root style's font size, not the tab's own font size, as Chrome
    // measures its half a space with the block container's font: a tab set at 64 units still
    // uses a stop 9 units away.
    let text = "a\tb";
    let tab_size = a + 9.;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::new(tab_size)));
    builder.push(StyleProperty::FontSize(64.), 1..2);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(glyph_xs(&layout, 0)[1], tab_size);

    // A root font size of 32 raises the default to 16, so a stop 12 units away is skipped.
    let a_32 = {
        let mut builder = env.ranged_builder("a");
        builder.push_default(StyleProperty::FontSize(32.));
        let mut layout: Layout<ColorBrush> = builder.build("a");
        layout.break_all_lines(None);
        layout.full_width()
    };
    let tab_size = a_32 + 12.;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontSize(32.));
    builder.push_default(StyleProperty::TabSize(TabSize::new(tab_size)));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(glyph_xs(&layout, 0)[1], 2. * tab_size);
}

/// Guards against `Spacing::gaps` giving a tab a letter or word spacing gap, which would move the
/// text after it off its stop. Source: Chrome, where `tab-size: 4; letter-spacing: 10px` puts the
/// glyph after a tab exactly on the stop.
#[test]
fn tab_gets_no_letter_or_word_spacing_of_its_own() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let tab_size = 8. * space;
    assert!(a + 10. < tab_size);

    // Letter spacing applies after "a", but not after the tab, so "b" lands exactly on the stop.
    let text = "a\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
    builder.push_default(StyleProperty::LetterSpacing(10.));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(glyph_xs(&layout, 0)[1], tab_size);

    // The same with word spacing: the space before the tab gets both gaps, the tab gets neither.
    let text = "a \tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
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

/// Guards against the line breaker sizing a tab from its position within the indented line
/// rather than from the line's start edge. Source: Chrome, where `text-indent: 30px; tab-size: 4`
/// puts the glyph after a tab on the first stop from the content edge, and `text-indent: -30px`
/// puts it at the content edge itself.
#[test]
fn tab_stops_are_measured_from_the_indented_line_start() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let b = advance(&mut env, "b");
    let tab_size = 4. * space;

    // With the indent, the first stop is already past "a", and the stop nearest to the tab is
    // less than half a space away, so the tab goes two stops along.
    let text = "a\tb";
    let indent = tab_size - space / 8.;
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
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
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
    let mut layout = builder.build(text);
    layout.set_text_indent(-space, IndentOptions::default());
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    nearly_eq(layout.get(0).unwrap().metrics().advance, space + b);
    nearly_eq(glyph_xs(&layout, 0)[0], 0.);
}

/// Guards against a layout keeping tab advances from an earlier line breaking. Source: an
/// invariant of this design, a tab's advance depends on where the tab lands.
#[test]
fn tab_resized_when_relayout_moves_it() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let aaa = advance(&mut env, "aaa");
    let aaa_aaa = advance(&mut env, "aaa aaa");
    assert!(aaa < 8. * space && aaa_aaa > 8. * space && aaa_aaa < 16. * space - space / 2.);

    let text = "aaa aaa\tb";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(stops(8. * space, space)));
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

/// Guards against `BreakLines::revert_to` dropping the tabs placed before the saved state, or
/// keeping the ones placed after it, when the state was saved in the middle of a line (here at an
/// out-of-flow box, which yields control). Source: an invariant of the side table, each line owns
/// exactly the entries of its own tabs; the same text laid out without the detour is the
/// reference.
#[test]
fn tab_advances_kept_up_to_a_revert_in_the_middle_of_a_line() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "a	b	c";
    let make_layout = |env: &mut TestEnv| {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::TabSize(TabSize::new(80.)));
        builder.push_inline_box(InlineBox {
            id: 1,
            kind: InlineBoxKind::CustomOutOfFlow,
            index: 3,
            width: 0.,
            height: 0.,
            baseline: None,
            vertical_align: VerticalAlign::BASELINE,
        });
        builder.build(text)
    };
    let mut expected = make_layout(&mut env);
    expected.break_all_lines(None);

    let mut layout = make_layout(&mut env);
    let mut breaker = layout.break_lines();
    breaker.state_mut().set_layout_max_advance(f32::INFINITY);
    breaker.state_mut().set_line_max_advance(f32::INFINITY);
    // The breaker yields at the box, after the first tab.
    let Some(YieldData::InlineBoxBreak(data)) = breaker.break_next() else {
        panic!("expected the breaker to yield at the out-of-flow box");
    };
    // Save the state there, finish the line (placing the second tab), then go back to it.
    let saved = breaker.state().clone();
    breaker.state_mut().append_inline_box_to_line(
        data.advance,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    breaker.break_next();
    breaker.revert_to(saved);
    let Some(YieldData::InlineBoxBreak(data)) = breaker.break_next() else {
        panic!("expected the breaker to yield at the out-of-flow box again");
    };
    breaker.state_mut().append_inline_box_to_line(
        data.advance,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    breaker.break_remaining(f32::INFINITY);

    let (actual, expected_xs) = (glyph_xs(&layout, 0), glyph_xs(&expected, 0));
    assert_eq!(actual.len(), expected_xs.len());
    for (actual, expected) in actual.into_iter().zip(expected_xs) {
        nearly_eq(actual, expected);
    }
    nearly_eq(layout.width(), expected.width());
}

/// Guards against a tab being recorded twice when a maximum line height yield interrupts the
/// breaker at the tab and the caller retries it. Source: the same text laid out without the
/// interruption is the reference (in debug builds, the side table also asserts it is sorted).
#[test]
fn tab_sized_once_when_retried_after_a_max_height_yield() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "a	b";
    let make_layout = |env: &mut TestEnv| {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::TabSize(TabSize::new(80.)));
        // A tall "a" makes the line exceed the maximum height set below.
        builder.push(StyleProperty::FontSize(64.), 0..1);
        builder.build(text)
    };
    let mut expected = make_layout(&mut env);
    expected.break_all_lines(None);

    let mut layout = make_layout(&mut env);
    let mut breaker = layout.break_lines();
    breaker.state_mut().set_layout_max_advance(f32::INFINITY);
    breaker.state_mut().set_line_max_advance(f32::INFINITY);
    breaker.state_mut().set_line_max_height(20.);
    assert!(matches!(
        breaker.break_next(),
        Some(YieldData::MaxHeightExceeded(_))
    ));
    // Give the line room and retry.
    breaker.state_mut().set_line_max_height(f32::INFINITY);
    breaker.break_remaining(f32::INFINITY);

    let (actual, expected_xs) = (glyph_xs(&layout, 0), glyph_xs(&expected, 0));
    assert_eq!(actual.len(), expected_xs.len());
    for (actual, expected) in actual.into_iter().zip(expected_xs) {
        nearly_eq(actual, expected);
    }
    nearly_eq(layout.width(), expected.width());
}

/// Guards against `break_next_with_length`, which has its own copy of the breaker's atom loop,
/// not sizing tabs, or sizing one after a newline as if the newline took space. Source:
/// `break_next`, which the length-based breaker must agree with.
#[test]
fn tab_sized_when_breaking_by_length() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let ab = advance(&mut env, "ab");
    let tab_size = 8. * space;
    assert!(ab < tab_size - space / 2.);

    let text = "ab\tc\tab";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
    let mut layout = builder.build(text);
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(3);
    breaker.break_next_with_length(4);
    breaker.finish();

    assert_eq!(layout.len(), 2);
    // The first line is "ab\t", with the tab hanging off the end of the wrapped line.
    let line = layout.get(0).unwrap();
    nearly_eq(line.metrics().advance, tab_size);
    nearly_eq(line.metrics().hanging_advance, tab_size - ab);
    // The second line starts with "c" at the line start, then a tab to the first stop.
    nearly_eq(glyph_xs(&layout, 1)[1], tab_size);

    // `break_next_with_length` does not break at newlines, so the newline and the tab share a
    // line. The newline has no advance, so the tab starts after "ab" + "cd" and must reach the
    // first stop.
    let abcd = advance(&mut env, "abcd");
    let tab_size = 20. * space;
    assert!(abcd < tab_size - space / 2.);
    let text = "ab\ncd\tef";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
    let mut layout = builder.build(text);
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(100);
    breaker.finish();

    assert_eq!(layout.len(), 1);
    // Glyphs in order: a b c d e f; "e" shows where the tab landed.
    nearly_eq(glyph_xs(&layout, 0)[4], tab_size);
}

/// Guards against a trailing tab's advance not hanging, which would widen the layout by a tab
/// that is never drawn. Source: CSS white-space processing treats a preserved tab at the end of a
/// line like a preserved space.
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
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
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
    let layout = build(&mut env, "a\t", stops(tab_size, space));
    nearly_eq(layout.width(), tab_size);
    nearly_eq(layout.full_width(), tab_size);
}

/// Guards against the `Cluster` API and hit testing reading the tab's shaped advance of zero.
/// Source: an invariant of this design, every reader of a line adds the tab's advance back
/// through `EffectiveSpacing`.
#[test]
fn tab_advance_is_visible_to_clusters_and_hit_testing() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let tab_size = 8. * space;

    let layout = build(&mut env, "a\tb", stops(tab_size, space));
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

/// The picture most of the value is in: four lines whose first words differ in width share the
/// positions of their second and third columns. Source: how an editor shows tab-separated text.
#[test]
fn tab_columns_align() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");

    // Each line's second column starts at the first stop a tab reaches: the same stop for the
    // first three lines, and the next one for the line whose first word is wider.
    let text = "1\tfoo\tx\n12\tfoo\tx\n123\tfoo\tx\n1234567\tfoo\tx";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(stops(4. * space, space)));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

/// The same picture for right-to-left text: the line's start edge is its right edge, so the
/// columns line up from the right. Source: CSS Text 4, tab stops are measured from the line's
/// start edge.
#[test]
fn tab_columns_align_rtl() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");

    // Alef, then a tab, then "foo"-like Arabic letters, then a tab, then a final letter. The first
    // column is one, two, three and seven alefs wide.
    let text = "\u{627}\t\u{641}\u{648}\u{648}\t\u{633}\n\
                \u{627}\u{627}\t\u{641}\u{648}\u{648}\t\u{633}\n\
                \u{627}\u{627}\u{627}\t\u{641}\u{648}\u{648}\t\u{633}\n\
                \u{627}\u{627}\u{627}\u{627}\u{627}\u{627}\u{627}\t\u{641}\u{648}\u{648}\t\u{633}";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(stops(4. * space, space)));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

/// Guards against justification moving a tab off the advance the breaker gave it, or counting a
/// hanging separator as an opportunity. Source: a deliberate deferral of tab-aware justification
/// (Chrome also stretches the tab itself; that is a follow-up).
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
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
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
    builder.push_default(StyleProperty::TabSize(stops(tab_size, space)));
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
