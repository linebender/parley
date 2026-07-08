// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Basic (and assorted) layout tests.

use crate::util::TestEnv;
use crate::{test_name, util::ColorBrush};
use parley::{
    Alignment, AlignmentOptions, BreakReason, ContentWidths, FontFamily, InlineBox, InlineBoxKind,
    Layout, LineHeight, PositionedLayoutItem, StyleProperty, TextStyle, WhiteSpaceCollapse,
};
use peniko::color::{AlphaColor, Srgb, palette};
use peniko::kurbo::Size;

#[test]
fn plain_multiline_text() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Hello world!\nLine 2\nLine 4";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
}

#[test]
fn unicode_separators_break_lines() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "A\u{2028}B\u{2029}C";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    assert_eq!(layout.len(), 3, "expected 3 lines from U+2028 and U+2029");
    assert_eq!(
        layout.get(0).unwrap().break_reason(),
        BreakReason::Explicit,
        "expected U+2028 to cause an explicit break"
    );
    assert_eq!(
        layout.get(1).unwrap().break_reason(),
        BreakReason::Explicit,
        "expected U+2029 to cause an explicit break"
    );
    assert_ne!(
        layout.get(2).unwrap().break_reason(),
        BreakReason::Explicit,
        "did not expect a trailing explicit break"
    );
}

#[test]
fn placing_inboxes() {
    let mut env = TestEnv::new(test_name!(), None);

    for (position, test_case_name) in [
        (0, "start"),
        (3, "in_word"),
        (12, "end_nl"),
        (13, "start_nl"),
    ] {
        let text = "Hello world!\nLine 2\nLine 4";
        let mut builder = env.ranged_builder(text);
        builder.push_inline_box(InlineBox {
            id: 0,
            kind: InlineBoxKind::InFlow,
            index: position,
            width: 10.0,
            height: 10.0,
            baseline: None,
        });
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
fn only_inboxes_wrap() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "";
    let mut builder = env.ranged_builder(text);
    for id in 0..10 {
        builder.push_inline_box(InlineBox {
            id,
            kind: InlineBoxKind::InFlow,
            index: 0,
            width: 10.0,
            height: 10.0,
            baseline: None,
        });
    }
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(40.0));
    layout.align(Alignment::Center, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
}

#[test]
fn full_width_inbox() {
    let mut env = TestEnv::new(test_name!(), None);

    for (width, test_case_name) in [(99., "smaller"), (100., "exact"), (101., "larger")] {
        let text = "ABC";
        let mut builder = env.ranged_builder(text);
        builder.push_inline_box(InlineBox {
            id: 0,
            kind: InlineBoxKind::InFlow,
            index: 1,
            width: 10.,
            height: 10.0,
            baseline: None,
        });
        builder.push_inline_box(InlineBox {
            id: 1,
            kind: InlineBoxKind::InFlow,
            index: 1,
            width,
            height: 10.0,
            baseline: None,
        });
        builder.push_inline_box(InlineBox {
            id: 2,
            kind: InlineBoxKind::InFlow,
            index: 2,
            width,
            height: 10.0,
            baseline: None,
        });
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(100.));
        layout.align(Alignment::Start, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
fn inbox_separated_by_whitespace() {
    let mut env = TestEnv::new(test_name!(), None);

    let mut builder = env.tree_builder();
    builder.push_inline_box(InlineBox {
        id: 0,
        kind: InlineBoxKind::InFlow,
        index: 0,
        width: 10.,
        height: 10.0,
        baseline: None,
    });
    builder.push_text(" ");
    builder.push_inline_box(InlineBox {
        id: 1,
        kind: InlineBoxKind::InFlow,
        index: 1,
        width: 10.0,
        height: 10.0,
        baseline: None,
    });
    builder.push_text(" ");
    builder.push_inline_box(InlineBox {
        id: 2,
        kind: InlineBoxKind::InFlow,
        index: 2,
        width: 10.0,
        height: 10.0,
        baseline: None,
    });
    builder.push_text(" ");
    builder.push_inline_box(InlineBox {
        id: 3,
        kind: InlineBoxKind::InFlow,
        index: 3,
        width: 10.0,
        height: 10.0,
        baseline: None,
    });
    let (mut layout, _text) = builder.build();
    layout.break_all_lines(Some(100.));
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

/// The `baseline` field of an inline box should align the box's baseline with the text baseline.
///
/// The rendered snapshots draw the box's baseline (when set) as a horizontal line, which should
/// always line up with the baseline of the surrounding text.
#[test]
fn inbox_with_baseline() {
    let mut env = TestEnv::new(test_name!(), None);

    // The box is 30px tall. We vary where its baseline sits within the box:
    // - `top`: baseline at the top, so the box hangs below the text baseline.
    // - `middle`: baseline in the middle of the box.
    // - `bottom`: baseline at the bottom, equivalent to not specifying a baseline.
    for (baseline, test_case_name) in [(0.0, "top"), (15.0, "middle"), (30.0, "bottom")] {
        let text = "Hello world!";
        let mut builder = env.ranged_builder(text);
        builder.push_inline_box(InlineBox {
            id: 0,
            kind: InlineBoxKind::InFlow,
            index: 5,
            width: 20.0,
            height: 30.0,
            baseline: Some(baseline),
        });
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

/// Inline boxes with differing heights but matching baselines should all align to the same text
/// baseline.
#[test]
fn inboxes_with_matching_baselines() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "AxByC";
    let mut builder = env.ranged_builder(text);
    // A short box and a tall box, both with their baselines 10px from the top.
    builder.push_inline_box(InlineBox {
        id: 0,
        kind: InlineBoxKind::InFlow,
        index: 1,
        width: 15.0,
        height: 15.0,
        baseline: Some(10.0),
    });
    builder.push_inline_box(InlineBox {
        id: 1,
        kind: InlineBoxKind::InFlow,
        index: 3,
        width: 15.0,
        height: 40.0,
        baseline: Some(10.0),
    });
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

/// A box that mostly sits above the baseline (large ascent) and a box that mostly sits below the
/// baseline (large descent) on the same line. The line height must grow to fit the combined
/// ascent of the first box and descent of the second box, even though neither box on its own is
/// as tall as that sum.
#[test]
fn inboxes_with_large_ascent_and_descent() {
    let mut env = TestEnv::new(test_name!(), None);

    // Surround the line containing the boxes with plain text lines, so we can see how the
    // adjacent lines are positioned relative to the (much taller) box line.
    let text = "Line above\nAxB\nLine below";
    let box_line_start = text.find("AxB").unwrap();
    let mut builder = env.ranged_builder(text);
    // Large ascent: the baseline is near the bottom of the box, so most of it is above the
    // baseline.
    builder.push_inline_box(InlineBox {
        id: 0,
        kind: InlineBoxKind::InFlow,
        index: box_line_start + 1,
        width: 15.0,
        height: 40.0,
        baseline: Some(38.0),
    });
    // Large descent: the baseline is near the top of the box, so most of it is below the baseline.
    builder.push_inline_box(InlineBox {
        id: 1,
        kind: InlineBoxKind::InFlow,
        index: box_line_start + 2,
        width: 15.0,
        height: 40.0,
        baseline: Some(2.0),
    });
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());

    // The middle line (containing the boxes) should be tall enough to contain the first box's
    // ascent (38px) plus the second box's descent (40 - 2 = 38px).
    let box_line = layout.get(1).unwrap();
    assert!(
        box_line.metrics().line_height >= 76.0,
        "expected line height of at least 76px to fit the combined ascent and descent, got {}",
        box_line.metrics().line_height
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn inbox_below_baseline_keeps_grid() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "AAAA\nBBBB\nCCCC";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(60.0)));
    builder.push_inline_box(InlineBox {
        id: 0,
        kind: InlineBoxKind::InFlow,
        index: 7, // between the B's on the middle line
        width: 20.0,
        height: 15.0,
        baseline: Some(0.0),
    });
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());

    // Every line has the same 60px line height, so consecutive baselines must be exactly 60px
    // apart. The inline box on the middle line fits within the line and must not disturb this.
    let baselines: Vec<f32> = (0..layout.len())
        .map(|i| layout.get(i).unwrap().metrics().baseline)
        .collect();
    assert_eq!(
        baselines[1] - baselines[0],
        60.0,
        "middle line baseline off-grid: {baselines:?}"
    );
    assert_eq!(
        baselines[2] - baselines[1],
        60.0,
        "bottom line baseline off-grid: {baselines:?}"
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn trailing_whitespace_ltr() {
    let mut env = TestEnv::new(test_name!(), None);

    {
        let text = "AAA BBB";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(45.));
        layout.align(Alignment::Start, AlignmentOptions::default());

        env.with_name("soft_wrap").check_layout_snapshot(&layout);
    }

    {
        let text = "AAA \nBBB";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());

        env.with_name("hard_wrap").check_layout_snapshot(&layout);
    }
}

#[test]
fn trailing_whitespace_rtl() {
    let mut env = TestEnv::new(test_name!(), None);

    {
        let text = "بببب ااااا";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(45.));
        layout.align(Alignment::Start, AlignmentOptions::default());

        env.with_name("soft_wrap").check_layout_snapshot(&layout);
    }

    {
        let text = "بببب \nااااا";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());

        env.with_name("hard_wrap").check_layout_snapshot(&layout);
    }
}

#[test]
fn trailing_whitespace_bidi() {
    let mut env = TestEnv::new(test_name!(), None);

    {
        for (text, test_case_name) in [
            ("AAA ااااا", "soft_wrap_ltr_rtl"),
            ("بببب BBB", "soft_wrap_rtl_ltr"),
        ] {
            let builder = env.ranged_builder(text);
            let mut layout = builder.build(text);
            layout.break_all_lines(Some(45.));
            layout.align(Alignment::Start, AlignmentOptions::default());

            env.with_name(test_case_name).check_layout_snapshot(&layout);
        }
    }

    {
        for (text, test_case_name) in [
            ("AAA \nااااا", "hard_wrap_ltr_rtl"),
            ("بببب \nBBB", "hard_wrap_rtl_ltr"),
        ] {
            let builder = env.ranged_builder(text);
            let mut layout = builder.build(text);
            layout.break_all_lines(None);
            layout.align(Alignment::Start, AlignmentOptions::default());

            env.with_name(test_case_name).check_layout_snapshot(&layout);
        }
    }
}

#[test]
fn leading_whitespace() {
    let mut env = TestEnv::new(test_name!(), None);

    for (mode, test_case_name) in [
        (WhiteSpaceCollapse::Preserve, "preserve"),
        (WhiteSpaceCollapse::Collapse, "collapse"),
    ] {
        let mut builder = env.tree_builder();
        builder.set_white_space_mode(mode);
        builder.push_text("Line 1");
        builder.push_style_modification_span(None);
        builder.set_white_space_mode(WhiteSpaceCollapse::Preserve);
        builder.push_text("\n");
        builder.pop_style_span();
        builder.set_white_space_mode(mode);
        builder.push_text("  Line 2");
        let (mut layout, _) = builder.build();
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

/// Each [`WhiteSpaceCollapse`] mode should preserve or collapse segment breaks (newlines) as
/// specified, which is observable as the number of hard-broken lines.
#[test]
fn white_space_collapse_hard_breaks() {
    let mut env = TestEnv::new(test_name!(), None);

    // (mode, text, expected number of lines)
    let cases = [
        // Segment breaks are preserved.
        (WhiteSpaceCollapse::Preserve, "a\nb", 2),
        (WhiteSpaceCollapse::BreakSpaces, "a\nb", 2),
        // Segment breaks are collapsed away.
        (WhiteSpaceCollapse::Collapse, "a\nb", 1),
        // Segment breaks are preserved, surrounding white space collapsed.
        (WhiteSpaceCollapse::PreserveBreaks, "a  \n  b", 2),
        // Segment breaks are converted to spaces.
        (WhiteSpaceCollapse::PreserveSpaces, "a\nb", 1),
    ];

    for (mode, text, expected_lines) in cases {
        let mut builder = env.tree_builder();
        builder.set_white_space_mode(mode);
        builder.push_text(text);
        let (mut layout, _) = builder.build();
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());
        assert_eq!(
            layout.len(),
            expected_lines,
            "unexpected line count for mode {mode:?} and text {text:?}"
        );
    }
}

/// The different `WhiteSpaceCollapse` modes hang trailing white space differently, which is
/// reflected in the min-content and max-content intrinsic sizes:
///
/// - `Collapse`/`PreserveBreaks` remove trailing white space: excluded from both sizes.
/// - `Preserve`/`PreserveSpaces` hang trailing white space: excluded from min-content, but counted
///   in max-content (it conditionally hangs at the forced break / end of the text).
/// - `BreakSpaces` never hangs: counted in both sizes.
#[test]
fn white_space_collapse_trailing_whitespace_intrinsic_sizes() {
    use WhiteSpaceCollapse::*;
    let mut env = TestEnv::new(test_name!(), None);

    let widths = |env: &mut TestEnv, mode| {
        let mut builder = env.tree_builder();
        builder.set_white_space_mode(mode);
        builder.push_text("xx yy ");
        let (layout, _) = builder.build();
        layout.calculate_content_widths()
    };

    let collapse = widths(&mut env, Collapse);
    let preserve = widths(&mut env, Preserve);
    let break_spaces = widths(&mut env, BreakSpaces);

    // max-content: the trailing space is removed for `Collapse` but counted for `Preserve` and
    // `BreakSpaces`.
    assert!(preserve.max > collapse.max);
    assert!((preserve.max - break_spaces.max).abs() < 0.01);

    // min-content: only `BreakSpaces` counts the trailing space (it never hangs).
    assert!(break_spaces.min > preserve.min);
    assert!((preserve.min - collapse.min).abs() < 0.01);
}

/// Preserved trailing white space *conditionally* hangs at a forced break / the end of the text: it
/// is counted when it fits, unlike collapsible white space (which is removed) and unlike white
/// space at a soft wrap (which always hangs).
#[test]
fn preserve_conditionally_hangs_trailing_whitespace_at_forced_break() {
    use WhiteSpaceCollapse::*;
    let mut env = TestEnv::new(test_name!(), None);

    let width = |env: &mut TestEnv, mode| {
        let mut builder = env.tree_builder();
        builder.set_white_space_mode(mode);
        // Trailing space before a forced break, on both lines.
        builder.push_text("xx \nxx");
        let (mut layout, _) = builder.build();
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());
        layout.width()
    };

    // `PreserveBreaks` keeps the forced break but removes the trailing space, so the line is only
    // as wide as "xx". `Preserve` and `BreakSpaces` keep the space (it fits, so it does not hang),
    // making the line wider. (`Collapse` is not comparable here as it also collapses the newline.)
    let preserve_breaks = width(&mut env, PreserveBreaks);
    let preserve = width(&mut env, Preserve);
    let break_spaces = width(&mut env, BreakSpaces);

    assert!(preserve > preserve_breaks);
    assert!((preserve - break_spaces).abs() < 0.01);
}

#[test]
fn nested_span_inheritance() {
    let ts = |c: AlphaColor<Srgb>| TextStyle {
        font_family: FontFamily::from(crate::util::env::FONT_FAMILY_LIST),
        font_size: 24.,
        line_height: LineHeight::Absolute(30.),
        brush: ColorBrush::new(c),
        ..TextStyle::default()
    };
    let sp = |c: AlphaColor<Srgb>| [StyleProperty::Brush(ColorBrush::new(c))];

    let mut env = TestEnv::new(test_name!(), None);
    let mut tb = env.tree_builder();
    tb.push_style_span(ts(palette::css::RED));
    tb.push_text("N"); // Red
    tb.push_style_span(ts(palette::css::RED));
    tb.push_text("e"); // Red
    tb.push_style_span(ts(palette::css::GREEN));
    tb.push_text("s"); // Green
    tb.push_style_modification_span(&sp(palette::css::GREEN));
    tb.push_text("t"); // Green
    tb.push_style_span(ts(palette::css::BLUE));
    tb.push_text("e"); // Blue
    tb.push_style_modification_span(None);
    tb.push_text("d"); // Blue
    tb.push_text(" ");
    tb.pop_style_span();
    tb.push_text("s"); // Blue
    tb.pop_style_span();
    tb.push_text("p"); // Green
    tb.pop_style_span();
    tb.push_text("a"); // Green
    tb.pop_style_span();
    tb.push_text("n"); // Red
    tb.pop_style_span();
    tb.push_text("s"); // Red
    tb.pop_style_span();
    tb.push_text(" w"); // Root style
    tb.push_style_span(ts(palette::css::GOLD));
    tb.push_style_span(ts(palette::css::GOLD));
    tb.push_style_span(ts(palette::css::GOLD));
    tb.push_text("o"); // Triple-nested gold
    tb.pop_style_span();
    tb.pop_style_span();
    tb.pop_style_span();
    tb.push_text("rk"); // Root style

    let (mut layout, _) = tb.build();
    layout.break_all_lines(None);
    env.check_layout_snapshot(&layout);
}

#[test]
fn base_level_alignment_ltr() {
    let mut env = TestEnv::new(test_name!(), None);

    for (alignment, test_case_name) in [
        (Alignment::Start, "start"),
        (Alignment::End, "end"),
        (Alignment::Center, "center"),
        (Alignment::Justify, "justify"),
    ] {
        let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(150.0));
        layout.align(alignment, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
fn base_level_alignment_rtl() {
    let mut env = TestEnv::new(test_name!(), None);

    for (alignment, test_case_name) in [
        (Alignment::Start, "start"),
        (Alignment::End, "end"),
        (Alignment::Center, "center"),
        (Alignment::Justify, "justify"),
    ] {
        let text = "عند برمجة أجهزة الكمبيوتر، قد تجد نفسك فجأة في مواقف غريبة، مثل الكتابة بلغة لا تتحدثها فعليًا.";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(150.0));
        layout.align(alignment, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
/// On overflow without alignment-on-overflow, RTL-text should be start-aligned (i.e., aligned to
/// the right edge, overflowing on the left).
fn overflow_alignment_rtl() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "عند برمجة أجهزة الكمبيوتر، قد تجد نفسك فجأة في مواقف غريبة، مثل الكتابة بلغة لا تتحدثها فعليًا.";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(1000.0));
    layout.align(Alignment::Center, AlignmentOptions::default());
    env.rendering_config().size = Some(Size::new(10., layout.height().into()));
    env.check_layout_snapshot(&layout);
}

#[test]
fn content_widths() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Hello world!\nLonger line with a looooooooong word.";
    let builder = env.ranged_builder(text);

    let mut layout = builder.build(text);

    let ContentWidths {
        min: min_content_width,
        max: max_content_width,
    } = layout.calculate_content_widths();

    layout.break_all_lines(Some(min_content_width));
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.with_name("min").check_layout_snapshot(&layout);

    layout.break_all_lines(Some(max_content_width));
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.with_name("max").check_layout_snapshot(&layout);
}

#[test]
fn content_widths_rtl() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "بببب ااااا";
    let builder = env.ranged_builder(text);

    let mut layout = builder.build(text);

    let ContentWidths {
        min: min_content_width,
        max: max_content_width,
    } = layout.calculate_content_widths();

    layout.break_all_lines(Some(min_content_width));
    layout.align(Alignment::Start, AlignmentOptions::default());
    env.with_name("min").check_layout_snapshot(&layout);

    layout.break_all_lines(Some(max_content_width));
    layout.align(Alignment::Start, AlignmentOptions::default());
    assert!(
        layout.width() <= max_content_width,
        "Layout should never be wider than the max content width"
    );
    env.with_name("max").check_layout_snapshot(&layout);
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

#[test]
fn test_cluster_info() {
    let test_name = test_name!();
    let mut env = TestEnv::new(test_name, Size::new(400.0, 200.0));

    let test_cluster_layout = |test: &str, text: &str, env: &mut TestEnv| {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontSize(24.0));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(400.0));
        env.with_name(test)
            .check_cluster_snapshot(&layout, text, 12.0);
    };

    // Latin LTR with ligature
    let text = "Hello Ligature: fi";
    test_cluster_layout("latin", text, &mut env);

    // Arabic RTL with ligature
    let text = "حداً ";
    test_cluster_layout("arabic", text, &mut env);

    // Mixed content with ligature
    let text = "Hello Ligature: fi, Arabic: حداً";
    test_cluster_layout("ltr_rtl_mixed", text, &mut env);

    // Newlines
    let text = "Hello\nLigature:\nfi";
    test_cluster_layout("newlines", text, &mut env);
}

#[test]
fn text_range_rtl() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "اللغة العربية";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(100.0));
    layout.align(Alignment::Start, AlignmentOptions::default());

    for line in layout.lines() {
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                glyph_run.run().clusters().for_each(|c| {
                    if !c.is_space_or_nbsp() {
                        assert_eq!(c.text_range().len(), 2);
                    }
                });
            }
        }
    }
}

#[test]
/// Layouts can be re-line-broken and re-aligned.
fn realign() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(150.0));
    for idx in 0..8 {
        if [2, 3, 4].contains(&idx) {
            layout.break_all_lines(Some(150.0));
        }
        layout.align(Alignment::Justify, AlignmentOptions::default());
    }
    env.check_layout_snapshot(&layout);
}

#[test]
/// Tests that all alignment option combinations can be applied to a dirty layout.
///
/// Rendering 684 snapshots takes over 10 seconds on a 5950X, so a simpler assert is used instead.
fn realign_all() {
    let mut env = TestEnv::new(test_name!(), None);

    let latin = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
    let arabic = "عند برمجة أجهزة الكمبيوتر، قد تجد نفسك فجأة في مواقف غريبة، مثل الكتابة بلغة لا تتحدثها فعليًا.";

    let texts = [(latin, "latin"), (arabic, "arabic")];

    let alignments = [
        (Alignment::Start, "start"),
        (Alignment::End, "end"),
        (Alignment::Left, "left"),
        (Alignment::Center, "center"),
        (Alignment::Right, "right"),
        (Alignment::Justify, "justify"),
    ];

    let all_opts = [
        (Some(150.), AlignmentOptions::default(), "150", "default"),
        (
            None,
            AlignmentOptions {
                align_when_overflowing: true,
            },
            "none",
            "awo_true",
        ),
        (
            None,
            AlignmentOptions {
                align_when_overflowing: false,
            },
            "none",
            "awo_false",
        ),
    ];

    // Build a collection of base truth
    let mut layouts = Vec::new();
    for (text, _) in texts {
        for (alignment, _) in alignments {
            for (max_advance, opts, _, _) in all_opts {
                let builder = env.ranged_builder(text);
                let mut layout = builder.build(text);
                layout.break_all_lines(max_advance);
                layout.align(alignment, opts);
                layouts.push(layout);
            }
        }
    }

    // Loop over all the base truths ...
    let mut idx = 0;
    for (text_idx, (_, text_name)) in texts.iter().enumerate() {
        for (_, align_name) in alignments {
            for (max_advance, _, ma_name, opts_name) in all_opts {
                let layout = &layouts[idx];
                idx += 1;

                let base_name = format!("{text_name}_{align_name}_{opts_name}_{ma_name}");
                //env.with_name(&base_name).check_layout_snapshot(&layout);

                // ... and make sure every combination can be applied on top without issues
                let mut jdx = text_idx * (layouts.len() / texts.len());
                for (top_alignment, align_name) in alignments {
                    for (top_max_advance, top_opts, ma_name, opts_name) in all_opts {
                        let mut top_layout = layout.clone();
                        // Only break lines again if the max advance differs from base,
                        // because otherwise we already have the correct line breaks.
                        // We want to specifically test the optimization of skipping it.
                        if max_advance != top_max_advance {
                            top_layout.break_all_lines(top_max_advance);
                        }
                        top_layout.align(top_alignment, top_opts);

                        let top_name = format!("{text_name}_{align_name}_{opts_name}_{ma_name}");
                        //env.with_name(&top_name).check_layout_snapshot(&top_layout);

                        let top_layout_truth = &layouts[jdx];
                        jdx += 1;

                        crate::util::assert_eq_layout_alignments(
                            top_layout_truth,
                            &top_layout,
                            &format!("{base_name} -> {top_name}"),
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn layout_impl_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Layout<()>>();
}
