// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! In-flow inline boxes replace a U+FFFC OBJECT REPLACEMENT CHARACTER in the text (which a
//! `TreeBuilder` inserts for them), and are shaped and broken as part of it.

use parley::{
    Affinity, BaseDirection, Cursor, InlineBox, InlineBoxKind, Layout, PositionedLayoutItem,
    Selection, StyleProperty, VerticalAlign, WhiteSpaceCollapse, YieldData,
};

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};

fn inline_box(index: usize) -> InlineBox {
    InlineBox {
        id: 0,
        kind: InlineBoxKind::InFlow,
        index,
        width: 20.,
        height: 10.,
        baseline: None,
        vertical_align: VerticalAlign::BASELINE,
    }
}

#[test]
fn inline_object_can_be_resized_after_building() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "ab\u{FFFC}cd";
    let mut layout = |width| {
        let mut builder = env.ranged_builder(text);
        builder.push_inline_box(InlineBox {
            width,
            ..inline_box(2)
        });
        builder.build(text)
    };
    let widths = |mut layout: Layout<ColorBrush>| {
        layout.break_all_lines(None);
        (layout.calculate_content_widths().max, layout.width())
    };
    assert_ne!(widths(layout(20.)), widths(layout(50.)));
    // Editing a box gives the same widths as building with the edited box.
    let (mut edited, built) = (layout(20.), layout(50.));
    edited.edit_inline_box(0, |inline_box| inline_box.width = 50.);
    assert_eq!(widths(edited), widths(built));
}

#[test]
fn inline_object_breaks_as_a_css_atomic_inline() {
    let mut env = TestEnv::new(test_name!(), None);
    // CSS Text 3 § 5.5 breaks on both sides of an atomic inline "even when adjacent to a character
    // that would normally suppress them", such as a full stop under UAX #14 (LB15d) or U+00A0
    // NO-BREAK SPACE, but not next to WJ, ZWJ or other GL characters (even through a combining
    // mark, LB9). Nor does it break before a space, a zero width space (LB7) or a combining mark.
    // `break-spaces` keeps spaces from hanging, so that a break before one would show.
    for (text, line_starts) in [
        ("a\u{FFFC}.", &[0, 1, 4][..]),
        ("\u{A0}\u{FFFC}\u{A0}", &[0, 2, 5]),
        ("\u{2060}\u{FFFC}\u{202F}", &[0]),
        ("\u{202F}\u{301}\u{FFFC}", &[0]),
        ("a\u{200D}\u{FFFC}\u{200B}b", &[0, 10]),
        ("\u{FFFC} b\u{FFFC}\u{301}", &[0, 4, 5]),
    ] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::WhiteSpaceCollapse(
            WhiteSpaceCollapse::BreakSpaces,
        ));
        for (index, _) in text.match_indices('\u{FFFC}') {
            builder.push_inline_box(inline_box(index));
        }
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(0.));
        let starts: Vec<_> = layout.lines().map(|line| line.text_range().start).collect();
        assert_eq!(starts, line_starts, "{text:?}");
    }
}

#[test]
fn inline_object_taller_than_the_line_max_height_yields_before_it() {
    let mut env = TestEnv::new(test_name!(), None);
    let ab = env.ranged_builder("ab").build("ab");
    let ab = ab.calculate_content_widths().max;
    let text = "ab\u{FFFC}";
    let mut builder = env.ranged_builder(text);
    builder.push_inline_box(InlineBox {
        height: 100.,
        ..inline_box(2)
    });
    let mut layout = builder.build(text);
    let mut breaker = layout.break_lines();
    let state = breaker.state_mut();
    state.set_layout_max_advance(f32::MAX);
    state.set_line_max_advance(f32::MAX);
    state.set_line_max_height(50.);
    let Some(YieldData::MaxHeightExceeded(data)) = breaker.break_next() else {
        panic!("expected a max-height break");
    };
    assert_eq!((data.advance, data.line_height), (ab, 100.));
}

#[test]
fn inline_object_aligns_with_the_span_it_is_in() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "a\u{FFFC}";
    let mut builder = env.ranged_builder(text);
    builder.push(StyleProperty::FontSize(40.), 1..);
    builder.push_inline_box(InlineBox {
        vertical_align: VerticalAlign::SUPER,
        ..inline_box(1)
    });
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let line = layout.lines().next().unwrap();
    let Some(PositionedLayoutItem::InlineBox(object)) = line.items().nth(1) else {
        unreachable!()
    };
    // `super` raises the box by a third of its parent's font size, here that of the U+FFFC's span.
    let raise = (40_f32 / 3.).round();
    assert_eq!(object.y + object.height, line.metrics().baseline - raise);
}

/// U+FFFC is a bidi neutral, so an inline object's direction follows the text on both sides of it.
/// The box goes right of the Arabic word in the LTR paragraph, and left of "12" in the RTL one.
#[test]
fn inline_object_in_bidi_text() {
    let mut env = TestEnv::new(test_name!(), None);
    for (case, direction, before, after) in [
        ("ltr", BaseDirection::Ltr, "ab سلم", " cd"),
        ("rtl", BaseDirection::Rtl, "سلم 12", " 34"),
    ] {
        let mut builder = env.tree_builder();
        builder.set_base_direction(direction);
        builder.push_text(before);
        builder.push_inline_box(inline_box(0));
        builder.push_text(after);
        let (mut layout, text) = builder.build();
        layout.break_all_lines(None);
        // Select the box and the text after it.
        let selection = Selection::new(
            Cursor::from_byte_index(&layout, before.len(), Affinity::Downstream),
            Cursor::from_byte_index(&layout, text.len(), Affinity::Downstream),
        );
        let geometry = selection.geometry(&layout);
        env.with_name(case)
            .render_and_check_snapshot(&layout, None, &geometry);
    }
}
