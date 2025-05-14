// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use peniko::kurbo::Size;

use crate::data::LayoutData;
use crate::{
    Alignment, AlignmentOptions, Brush, ContentWidths, InlineBox, WhiteSpaceCollapse, test_name,
};

use super::utils::TestEnv;

#[test]
fn plain_multiline_text() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Hello world!\nLine 2\nLine 4";
    let mut builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
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
    layout.align(None, Alignment::Middle, AlignmentOptions::default());

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
fn trailing_whitespace() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "AAA BBB";
    let mut builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(45.));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    assert!(
        layout.width() < layout.full_width(),
        "Trailing whitespace should cause a difference between width and full_width"
    );

    env.check_layout_snapshot(&layout);
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
fn base_level_alignment_ltr() {
    let mut env = TestEnv::new(test_name!(), None);

    for (alignment, test_case_name) in [
        (Alignment::Start, "start"),
        (Alignment::End, "end"),
        (Alignment::Middle, "middle"),
        (Alignment::Justified, "justified"),
    ] {
        let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
        let mut builder = env.ranged_builder(text);
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
        (Alignment::Middle, "middle"),
        (Alignment::Justified, "justified"),
    ] {
        let text = "عند برمجة أجهزة الكمبيوتر، قد تجد نفسك فجأة في مواقف غريبة، مثل الكتابة بلغة لا تتحدثها فعليًا.";
        let mut builder = env.ranged_builder(text);
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
    let mut builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(1000.0));
    layout.align(Some(10.), Alignment::Middle, AlignmentOptions::default());
    env.rendering_config().size = Some(Size::new(10., layout.height().into()));
    env.check_layout_snapshot(&layout);
}

#[test]
fn content_widths() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Hello world!\nLonger line with a looooooooong word.";
    let mut builder = env.ranged_builder(text);

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
    let mut builder = env.ranged_builder(text);

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
/// Layouts can be re-line-breaked and re-aligned.
fn realign() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
    let mut builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(150.0));
    for idx in 0..8 {
        if [2, 3, 4].contains(&idx) {
            layout.break_all_lines(Some(150.0));
        }
        layout.align(
            Some(150.),
            Alignment::Justified,
            AlignmentOptions::default(),
        );
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
        (Alignment::Middle, "middle"),
        (Alignment::Right, "right"),
        (Alignment::Justified, "justified"),
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
                let mut builder = env.ranged_builder(text);
                let mut layout = builder.build(text);
                layout.break_all_lines(max_advance);
                layout.align(Some(150.), alignment, opts);
                layouts.push(layout);
            }
        }
    }

    // Loop over all the base truths ..
    let mut idx = 0;
    for (text_idx, (_, text_name)) in texts.iter().enumerate() {
        for (_, align_name) in alignments {
            for (max_advance, _, ma_name, opts_name) in all_opts {
                let layout = &layouts[idx];
                idx += 1;

                let base_name = format!("{text_name}_{align_name}_{opts_name}_{ma_name}");
                //env.with_name(&base_name).check_layout_snapshot(&layout);

                // .. and make sure every combination can be applied on top without issues
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

                        assert_eq_layout_alignments(
                            &top_layout_truth.data,
                            &top_layout.data,
                            &format!("{base_name} -> {top_name}"),
                        );
                    }
                }
            }
        }
    }
}

/// Assert that the two provided layouts are equal in terms of alignment metrics.
fn assert_eq_layout_alignments<B: Brush>(a: &LayoutData<B>, b: &LayoutData<B>, case: &str) {
    assert_eq!(
        a.lines.len(),
        b.lines.len(),
        "line count mismatch with {case}"
    );

    for (line_a, line_b) in a.lines.iter().zip(b.lines.iter()) {
        assert_eq!(
            line_a.metrics.offset, line_b.metrics.offset,
            "line offset mismatch with {case}"
        );
    }

    assert_eq!(
        a.clusters.len(),
        b.clusters.len(),
        "cluster count mismatch with {case}"
    );

    for (cluster_a, cluster_b) in a.clusters.iter().zip(b.clusters.iter()) {
        assert_eq!(
            cluster_a.advance, cluster_b.advance,
            "cluster advance mismatch with {case}"
        );
    }
}
