// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Basic (and assorted) layout tests.

use crate::util::TestEnv;
use crate::{test_name, util::ColorBrush};
use parley::{
    Alignment, AlignmentOptions, BreakReason, ContentWidths, FontFamily, InlineBox, Layout,
    LineHeight, PositionedLayoutItem, StyleProperty, TextStyle, WhiteSpaceCollapse,
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
    layout.align(None, Alignment::Start, AlignmentOptions::default());

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
            index: position,
            width: 10.0,
            height: 10.0,
        });
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());
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
            index: 0,
            width: 10.0,
            height: 10.0,
        });
    }
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(40.0));
    layout.align(None, Alignment::Center, AlignmentOptions::default());

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
            index: 1,
            width: 10.,
            height: 10.0,
        });
        builder.push_inline_box(InlineBox {
            id: 1,
            index: 1,
            width,
            height: 10.0,
        });
        builder.push_inline_box(InlineBox {
            id: 2,
            index: 2,
            width,
            height: 10.0,
        });
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(100.));
        layout.align(None, Alignment::Start, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
fn inbox_separated_by_whitespace() {
    let mut env = TestEnv::new(test_name!(), None);

    let mut builder = env.tree_builder();
    builder.push_inline_box(InlineBox {
        id: 0,
        index: 0,
        width: 10.,
        height: 10.0,
    });
    builder.push_text(" ");
    builder.push_inline_box(InlineBox {
        id: 1,
        index: 1,
        width: 10.0,
        height: 10.0,
    });
    builder.push_text(" ");
    builder.push_inline_box(InlineBox {
        id: 2,
        index: 2,
        width: 10.0,
        height: 10.0,
    });
    builder.push_text(" ");
    builder.push_inline_box(InlineBox {
        id: 3,
        index: 3,
        width: 10.0,
        height: 10.0,
    });
    let (mut layout, _text) = builder.build();
    layout.break_all_lines(Some(100.));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
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
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.with_name("soft_wrap").check_layout_snapshot(&layout);
    }

    {
        let text = "AAA \nBBB";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

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
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.with_name("soft_wrap").check_layout_snapshot(&layout);
    }

    {
        let text = "بببب \nااااا";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

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
            layout.align(None, Alignment::Start, AlignmentOptions::default());

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
            layout.align(None, Alignment::Start, AlignmentOptions::default());

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
        layout.align(None, Alignment::Start, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
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
        layout.align(Some(150.0), alignment, AlignmentOptions::default());
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
        layout.align(None, alignment, AlignmentOptions::default());
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
    layout.align(Some(10.), Alignment::Center, AlignmentOptions::default());
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
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    env.with_name("min").check_layout_snapshot(&layout);

    layout.break_all_lines(Some(max_content_width));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
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
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    env.with_name("min").check_layout_snapshot(&layout);

    layout.break_all_lines(Some(max_content_width));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
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
            index: 3,
            width: 100.0,
            height: 10.0,
        });
        let mut layout = builder.build(text);
        let ContentWidths {
            min: min_content_width,
            ..
        } = layout.calculate_content_widths();
        layout.break_all_lines(Some(min_content_width));
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.with_name("full_width").check_layout_snapshot(&layout);
    }

    {
        let text = "A ";
        let mut builder = env.ranged_builder(text);
        builder.push_inline_box(InlineBox {
            id: 0,
            index: 2,
            width: 10.0,
            height: 10.0,
        });
        let mut layout = builder.build(text);
        let ContentWidths {
            max: max_content_width,
            ..
        } = layout.calculate_content_widths();
        layout.break_all_lines(Some(max_content_width));
        layout.align(None, Alignment::Start, AlignmentOptions::default());

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
    layout.align(None, Alignment::Start, AlignmentOptions::default());

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
        layout.align(Some(150.), Alignment::Justify, AlignmentOptions::default());
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
                layout.align(Some(150.), alignment, opts);
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
                        top_layout.align(Some(150.), top_alignment, top_opts);

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
