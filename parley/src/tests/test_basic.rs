// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::borrow::Cow;

use peniko::{
    color::{AlphaColor, Srgb, palette},
    kurbo::Size,
};

use super::utils::{ColorBrush, FONT_STACK, TestEnv, asserts::assert_eq_layout_data_alignments};
use crate::setting::Setting;
use crate::{
    Alignment, AlignmentOptions, ContentWidths, FontFamily, FontSettings, FontStack, InlineBox,
    Layout, LineHeight, StyleProperty, TextStyle, WhiteSpaceCollapse, test_name,
};

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
fn trailing_whitespace() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "AAA BBB";
    let builder = env.ranged_builder(text);
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
fn nested_span_inheritance() {
    let ts = |c: AlphaColor<Srgb>| TextStyle {
        font_stack: FontStack::from(FONT_STACK),
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
fn issue_409_justified_text() {
    let mut env = TestEnv::new(test_name!(), None);

    let text_one_line = "One line justified.\n";
    let text_last_line_one_word = "The last word of this text falls on the last line.\n";
    let text_last_line_three_words = "Three words of this text will end up on the last line.\n";
    let paragraphs = r#"A sentence across two lines.

And another sentence that breaks across, hopefully, three lines.

And, finally, yet another sentence."#;

    for (text, test_case_name) in [
        (text_one_line, "one_line"),
        (text_last_line_one_word, "last_line_one_word"),
        (text_last_line_three_words, "last_line_three_words"),
        (paragraphs, "paragraphs"),
    ] {
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(150.0));
        layout.align(None, Alignment::Justify, AlignmentOptions::default());
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
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
fn ligatures_ltr() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "abfi";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(100.0));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    let line = layout.lines().next().unwrap();
    let item = line.items().next().unwrap();
    let glyph_run = match item {
        crate::PositionedLayoutItem::GlyphRun(glyph_run) => glyph_run,
        crate::PositionedLayoutItem::InlineBox(_) => unreachable!(),
    };
    let mut last_advance = f32::MAX;
    glyph_run.run().clusters().enumerate().for_each(|(i, c)| {
        match i % 4 {
            // 'a' and 'b' are not ligatures.
            0 | 1 => assert!(!c.is_ligature_start() && !c.is_ligature_continuation()),
            // "f" is the ligature start whose cluster contains the "fi" glyph.
            2 => {
                assert!(c.is_ligature_start());
                assert_eq!(c.glyphs().count(), 1);
                assert_eq!(c.text_range().len(), 1);
                assert_eq!(c.glyphs().next().unwrap().id, 444);
                // The glyph for this ligature lives in the start cluster and should
                // contain the whole ligature's advance.
                assert_eq!(c.glyphs().next().unwrap().advance, c.advance() * 2.0);
            }
            // "i" is the ligature continuation whose cluster shares the advance with
            // the ligature start.
            3 => {
                assert!(c.is_ligature_continuation());
                // A continuation shares its advance with the previous cluster.
                assert_eq!(c.advance(), last_advance);
                assert_eq!(c.text_range().len(), 1);
                assert_eq!(c.glyphs().count(), 0);
            }
            _ => unreachable!(),
        }
        last_advance = c.advance();
    });
    env.check_layout_snapshot(&layout);
}

#[test]
fn ligatures_rtl() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "احدً";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(100.0));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    let line = layout.lines().next().unwrap();
    let item = line.items().next().unwrap();
    let glyph_run = match item {
        crate::PositionedLayoutItem::GlyphRun(glyph_run) => glyph_run,
        crate::PositionedLayoutItem::InlineBox(_) => unreachable!(),
    };
    let mut last_advance = f32::MAX;
    glyph_run.run().clusters().enumerate().for_each(|(i, c)| {
        match i % 4 {
            // "ح" and "د" are not ligatures.
            0 | 1 => assert!(!c.is_ligature_start() && !c.is_ligature_continuation()),
            // "د" is the ligature continuation whose cluster shares the advance with
            // the ligature start.
            2 => {
                assert!(c.is_ligature_continuation());
                assert_eq!(c.glyphs().count(), 0);
                assert!(c.is_ligature_continuation());
                assert_eq!(c.text_range().len(), 2);
                assert_eq!(c.glyphs().count(), 0);
            }
            // The last visual character (i.e. the first logical character) is the ligature start.
            3 => {
                assert!(c.is_ligature_start());
                assert_eq!(c.glyphs().count(), 2);
                assert_eq!(c.text_range().len(), 2);
                // The advance should be shared with the previous cluster of the ligature.
                assert_eq!(c.advance(), last_advance);
                // This cluster should contain the one glyph of the ligature whose advance
                // is the sum of the advances of the component clusters.
                assert_eq!(c.glyphs().nth(1).unwrap().advance, c.advance() * 2.0);
            }
            _ => unreachable!(),
        }
        last_advance = c.advance();
    });
    env.check_layout_snapshot(&layout);
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
            if let crate::PositionedLayoutItem::GlyphRun(glyph_run) = item {
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
fn font_features() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "fi ".repeat(4);
    let mut builder = env.ranged_builder(&text);
    builder.push(
        StyleProperty::FontFeatures(FontSettings::List(Cow::Borrowed(&[Setting {
            tag: crate::setting::tag_from_bytes(*b"liga"),
            value: 1,
        }]))),
        0..5,
    );
    builder.push(
        StyleProperty::FontFeatures(FontSettings::List(Cow::Borrowed(&[Setting {
            tag: crate::setting::tag_from_bytes(*b"liga"),
            value: 0,
        }]))),
        5..10,
    );
    let mut layout = builder.build(&text);
    layout.break_all_lines(Some(100.0));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
}

#[test]
fn variable_fonts() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "Hello World";

    for wght in [100., 500., 1000.] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontStack(FontStack::Single(
            FontFamily::Named(Cow::Borrowed("Arimo")),
        )));
        builder.push_default(StyleProperty::FontVariations(FontSettings::List(
            Cow::Borrowed(&[Setting {
                tag: crate::setting::tag_from_bytes(*b"wght"),
                value: wght,
            }]),
        )));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(100.0));
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        env.check_layout_snapshot(&layout);
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

                        assert_eq_layout_data_alignments(
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

#[test]
fn layout_impl_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Layout<()>>();
}
