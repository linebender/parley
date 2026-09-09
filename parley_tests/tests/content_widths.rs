// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Content width tests.

use crate::util::TestEnv;
use crate::{test_name, util::ColorBrush};
use parley::{
    Alignment, AlignmentOptions, ContentWidths, InlineBox, InlineBoxKind, Layout, StyleProperty,
};

/// Checks that calculated content widths agree with actual line breaking.
///
/// Leaves the layout broken at the min-content width.
fn assert_content_widths_match_layout(layout: &mut Layout<ColorBrush>) -> ContentWidths {
    let widths = layout.calculate_content_widths();

    // Note: if the line breaker and content width calculation performed the same floating point
    // operations in the same order, we could in theory guarantee exact equality.
    layout.break_all_lines(None);
    assert!(
        (layout.width() - widths.max).abs() < 1e-3,
        "Unconstrained layout width {} should equal the max content width {}",
        layout.width(),
        widths.max
    );

    layout.break_all_lines(Some(widths.min));
    assert!(
        (layout.width() - widths.min).abs() < 1e-3,
        "Layout width {} at the min content width should equal it ({})",
        layout.width(),
        widths.min
    );

    widths
}

/// The width of `text` laid out on a single line.
fn single_line_width(env: &mut TestEnv, text: &str) -> f32 {
    let mut layout = env.ranged_builder(text).build(text);
    layout.break_all_lines(None);
    layout.width()
}

#[test]
fn content_widths_multiple_trailing_spaces() {
    let mut env = TestEnv::new(test_name!(), None);

    // The widest word is followed by three spaces. When the line breaks after them, they all hang,
    // so none of them should count towards the min-content width. The middle space has a
    // different font size, so the three spaces are spread over three runs.
    let text = "AA BB CCC   DD EE";
    let mut builder = env.ranged_builder(text);
    builder.push(StyleProperty::FontSize(15.9), 10..11);
    let mut layout = builder.build(text);

    let widths = assert_content_widths_match_layout(&mut layout);
    assert_eq!(
        layout.len(),
        5,
        "one word per line at the min content width"
    );

    let word_width = single_line_width(&mut env, "CCC");
    assert!(
        (widths.min - word_width).abs() < 1e-3,
        "Min content width {} should equal the widest word's width {}",
        widths.min,
        word_width
    );
}

#[test]
fn content_widths_partially_hanging_atom() {
    let mut env = TestEnv::new(test_name!(), None);

    // A prepend character followed by a space forms a single grapheme, and so a single atom, of
    // two shaped clusters. Only the space's cluster hangs.
    let text = "a\u{0D4E} b c";
    let mut layout = env.ranged_builder(text).build(text);

    let widths = assert_content_widths_match_layout(&mut layout);
    assert_eq!(
        layout.len(),
        3,
        "one word per line at the min content width"
    );

    let word_width = single_line_width(&mut env, "a\u{0D4E}");
    assert!(
        (widths.min - word_width).abs() < 1e-3,
        "Min content width {} should equal the widest word's width {} (excluding the space)",
        widths.min,
        word_width
    );
}

#[test]
fn content_widths_mixed_direction() {
    let mut env = TestEnv::new(test_name!(), None);

    // Whitespace only hangs if it ends up at the line's end edge after bidi reordering. The
    // content widths should agree with the line breaker on which whitespace hangs.
    for text in [
        "abc ااا ببب def",
        "ااا abc def ببب",
        "abc ااا\u{a0}ببب   def",
    ] {
        let mut layout = env.ranged_builder(text).build(text);
        assert_content_widths_match_layout(&mut layout);
    }
}

#[test]
fn inbox_content_width() {
    let mut env = TestEnv::new(test_name!(), None);

    {
        let text = "Hello world!";
        let mut builder = env.ranged_builder(text);
        builder.push_inline_box(InlineBox {
            id: 0,
            kind: InlineBoxKind::InFlow,
            index: 3,
            width: 100.0,
            height: 10.0,
            baseline: None,
        });
        let mut layout = builder.build(text);
        let ContentWidths {
            min: min_content_width,
            ..
        } = layout.calculate_content_widths();
        layout.break_all_lines(Some(min_content_width));
        layout.align(Alignment::Start, AlignmentOptions::default());

        env.with_name("full_width").check_layout_snapshot(&layout);
    }

    {
        let text = "A ";
        let mut builder = env.ranged_builder(text);
        builder.push_inline_box(InlineBox {
            id: 0,
            kind: InlineBoxKind::InFlow,
            index: 2,
            width: 10.0,
            height: 10.0,
            baseline: None,
        });
        let mut layout = builder.build(text);
        let ContentWidths {
            max: max_content_width,
            ..
        } = layout.calculate_content_widths();
        layout.break_all_lines(Some(max_content_width));
        layout.align(Alignment::Start, AlignmentOptions::default());

        assert!(
            layout.width() <= max_content_width,
            "Layout should never be wider than the max content width"
        );

        env.with_name("trailing_whitespace")
            .check_layout_snapshot(&layout);
    }
}
