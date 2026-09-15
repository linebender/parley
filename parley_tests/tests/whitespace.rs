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

fn close(actual: f32, expected: f32) {
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
    // Following CSS Text 4 § 4.3.2, whitespace hangs from the line's end inward: collapsible
    // whitespace unconditionally, preserved whitespace only as far as it still overflows. A
    // preserved space that fits ends the hanging sequence, so the whitespace before it doesn't
    // hang, whereas collapsible whitespace at the end is taken off first, so the preserved spaces
    // before it hang conditionally.
    let word = advance(&mut env, "X");
    let space = advance(&mut env, " ");
    let ideographic_space = advance(&mut env, "\u{3000}");
    let full = word + 2. * space + ideographic_space;
    let preserved = 2. * space;
    // The hanging advance for a line overflowing by `overflow`, with the preserved spaces logically
    // before or after the collapsible ideographic space.
    let expected_hanging = |preserved_first: bool, overflow: f32| {
        if preserved_first {
            ideographic_space + (overflow - ideographic_space).clamp(0., preserved)
        } else if overflow < preserved {
            overflow.max(0.)
        } else {
            preserved + ideographic_space
        }
    };
    for (text, collapse_range, preserved_first, max_content) in [
        ("X  \u{3000}\nX", 3..6, true, word + preserved),
        ("X\u{3000}  \nX", 1..4, false, full),
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
        close(widths.min, word);
        close(widths.max, max_content);

        for overflow in [
            -5.,
            space / 2.,
            preserved,
            preserved + ideographic_space / 2.,
            full - word + 5.,
        ] {
            let width = full - overflow;
            layout.break_all_lines(Some(width));
            assert_eq!(layout.len(), 2, "{text:?} at {width}");
            let line = layout.get(0).unwrap();
            assert_eq!(line.break_reason(), BreakReason::Explicit);
            let hanging = expected_hanging(preserved_first, overflow);
            close(line.metrics().hanging_advance, hanging);
            close(layout.width(), full - hanging);
            // Overflowing lines aren't aligned by default.
            layout.align(Alignment::Right, AlignmentOptions::default());
            close(
                layout.get(0).unwrap().metrics().offset,
                (width - full + hanging).max(0.),
            );
        }
    }
}

#[test]
fn overflowing_whitespace_hangs_without_adding_a_break_opportunity() {
    let mut env = TestEnv::new(test_name!(), None);

    // Whitespace that overflows the line hangs, but whether the line breaks after it is up to
    // the line breaking analysis: there's no opportunity before "!" (UAX #14 LB13), nor inside
    // or after the grapheme of a space followed by a combining mark.
    let word = advance(&mut env, "aaa");
    for (text, lines) in [("aaa   !", 1), ("aaa \u{301}bbb", 1), ("aaa   bbb", 2)] {
        let mut layout: Layout<ColorBrush> = env.ranged_builder(text).build(text);
        layout.break_all_lines(Some(word + 1.));
        assert_eq!(layout.len(), lines, "{text:?}");
        let line = layout.get(0).unwrap();
        if lines == 1 {
            assert_eq!(line.break_reason(), BreakReason::None);
        } else {
            assert_eq!(line.break_reason(), BreakReason::Regular);
            close(
                line.metrics().hanging_advance,
                advance(&mut env, "aaa   ") - word,
            );
        }
    }
}
