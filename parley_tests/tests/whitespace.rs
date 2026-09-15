// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    test_name,
    util::{ColorBrush, TestEnv},
};
use parley::{
    Alignment, AlignmentOptions, BreakReason, Layout, StyleProperty, TextWrapMode,
    WhiteSpaceCollapse,
};

fn nearly_eq(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-3, "{actual} != {expected}");
}

/// The full advance of `text` on a single line, with none of it hanging.
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

#[test]
fn hanging_across_collapse_mode_boundary() {
    let mut env = TestEnv::new(test_name!(), None);

    // A `white-space-collapse` boundary inside the trailing whitespace before a forced break.
    // Collapsible whitespace at the end hangs unconditionally, and so does the preserved whitespace
    // before it (CSS Text 4 § 4.3.2 doesn't say). Preserved whitespace at the end hangs
    // conditionally, and the collapsible whitespace before it hangs only if the preserved
    // whitespace hangs in full (CSS Text 4 § 9.2).
    let word = advance(&mut env, "X");
    let space = advance(&mut env, " ");
    let ideographic_space = advance(&mut env, "\u{3000}");
    let whitespace = 2. * space + ideographic_space;
    let full = word + whitespace;
    for (text, collapse_range, conditional) in [
        ("X  \u{3000}\nX", 3..6, false),
        ("X\u{3000}  \nX", 1..4, true),
    ] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::WhiteSpaceCollapse(
            WhiteSpaceCollapse::Preserve,
        ));
        builder.push(
            StyleProperty::WhiteSpaceCollapse(WhiteSpaceCollapse::Collapse),
            collapse_range,
        );
        let mut layout = builder.build(text);
        let widths = layout.calculate_content_widths();
        nearly_eq(widths.min, word);
        nearly_eq(widths.max, if conditional { full } else { word });

        for overflow in [-5., space / 2., whitespace / 2., whitespace + 5.] {
            let width = full - overflow;
            layout.break_all_lines(Some(width));
            assert_eq!(layout.len(), 2, "{text:?} at {width}");
            let line = layout.get(0).unwrap();
            assert_eq!(line.break_reason(), BreakReason::Explicit);
            let hanging = if conditional && overflow < 2. * space {
                overflow.max(0.)
            } else {
                whitespace
            };
            nearly_eq(line.metrics().hanging_advance, hanging);
            nearly_eq(layout.width(), full - hanging);
            // Overflowing lines aren't aligned by default.
            layout.align(Alignment::Right, AlignmentOptions::default());
            nearly_eq(
                layout.get(0).unwrap().metrics().offset,
                (width - full + hanging).max(0.),
            );
        }
    }
}

#[test]
fn overflowing_whitespace_hangs_without_adding_a_break_opportunity() {
    let mut env = TestEnv::new(test_name!(), None);

    // Whitespace that overflows the line hangs, but whether the line breaks after it is up to the
    // line breaking analysis: e.g., there's no opportunity before "!" (UAX #14 LB13).
    //
    // (See `CHROMIUM_LINE_BREAK_OVERRIDE` to break in more positions.)
    let word = advance(&mut env, "aaa");
    for (text, lines) in [("aaa   !", 1), ("aaa   bbb", 2)] {
        let mut layout: Layout<ColorBrush> = env.ranged_builder(text).build(text);
        layout.break_all_lines(Some(word + 1.));
        assert_eq!(layout.len(), lines, "{text:?}");
        let line = layout.get(0).unwrap();
        if lines == 1 {
            assert_eq!(line.break_reason(), BreakReason::None);
        } else {
            assert_eq!(line.break_reason(), BreakReason::Regular);
            nearly_eq(
                line.metrics().hanging_advance,
                advance(&mut env, "aaa   ") - word,
            );
        }
    }
}
