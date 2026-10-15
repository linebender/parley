// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ops::Range;

use crate::{
    test_name,
    util::{ColorBrush, TestEnv},
};
use parley::{
    Alignment, AlignmentOptions, BaseDirection, BreakReason, Cluster, InlineBox, InlineBoxKind,
    Layout, OverflowWrap, StyleProperty, TextWrapMode, VerticalAlign, WhiteSpaceCollapse,
    WordBreak,
};

fn nearly_eq(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-3, "{actual} != {expected}");
}

/// A layout of `text` with the given whitespace and wrapping modes.
fn build(
    env: &mut TestEnv,
    text: &str,
    collapse: WhiteSpaceCollapse,
    wrap: TextWrapMode,
) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::WhiteSpaceCollapse(collapse));
    builder.push_default(StyleProperty::TextWrapMode(wrap));
    builder.build(text)
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

    // This tests changing `white-space-collapse` inside the trailing whitespace before a forced
    // break. Collapsible whitespace at the end hangs unconditionally, and the preserved whitespace
    // before it also hangs (CSS Text 4 § 4.3.2 says to hang conditionally only if the preserved
    // whitespace sequence is followed by a forced line break).
    //
    // Preserved whitespace at the end hangs conditionally, and the collapsible whitespace before it
    // hangs only if the preserved whitespace hangs in full (CSS Text 4 § 9.2).
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

#[test]
fn break_spaces_preserves_source_across_spans() {
    let mut env = TestEnv::new(test_name!(), None);
    let source = " \ta  \r\n b\u{2028} \u{3000}";
    for split in source
        .char_indices()
        .map(|(index, _)| index)
        .chain([source.len()])
    {
        let mut builder = env.tree_builder();
        builder.push_style_modification_span(&[StyleProperty::WhiteSpaceCollapse(
            WhiteSpaceCollapse::BreakSpaces,
        )]);
        builder.push_text(&source[..split]);
        builder.push_style_modification_span(&[StyleProperty::FontSize(24.)]);
        builder.push_text(&source[split..]);
        builder.pop_style_span();
        builder.pop_style_span();
        let (mut layout, text) = builder.build();
        assert_eq!(text, source);
        layout.break_all_lines(None);
        // A style boundary between the `\r` and the `\n` splits the CRLF into two atoms, each of
        // which forces its own break.
        let crlf_split = source[..split].ends_with('\r');
        assert_eq!(layout.len(), if crlf_split { 4 } else { 3 });
        for line in layout.lines() {
            nearly_eq(line.metrics().hanging_advance, 0.);
        }
    }
}

#[test]
fn break_spaces_wraps_each_space_without_hanging() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    for text in ["a   b", "    "] {
        for width in [0., space + 0.1] {
            let mut layout = build(
                &mut env,
                text,
                WhiteSpaceCollapse::BreakSpaces,
                TextWrapMode::Wrap,
            );
            layout.break_all_lines(Some(width));
            let lines: Vec<_> = layout
                .lines()
                .map(|line| &text[line.text_range()])
                .collect();
            assert_eq!(
                lines,
                if text.starts_with('a') {
                    vec!["a ", " ", " ", "b"]
                } else {
                    vec![" ", " ", " ", " "]
                },
            );
            for line in layout.lines() {
                nearly_eq(line.metrics().hanging_advance, 0.);
            }
            let widths = layout.calculate_content_widths();
            nearly_eq(
                widths.min,
                advance(&mut env, if text.starts_with('a') { "a " } else { " " }),
            );
            nearly_eq(widths.max, advance(&mut env, text));
        }
    }

    // A style span inside the whitespace doesn't move the break opportunities, and `NoWrap`
    // suppresses those of the spaces in its span.
    let text = "a   b";
    for wrap in [TextWrapMode::Wrap, TextWrapMode::NoWrap] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::WhiteSpaceCollapse(
            WhiteSpaceCollapse::BreakSpaces,
        ));
        builder.push(StyleProperty::FontSize(24.), 2..3);
        builder.push(StyleProperty::TextWrapMode(wrap), 2..);
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(space + 0.1));
        let lines: Vec<_> = layout
            .lines()
            .map(|line| &text[line.text_range()])
            .collect();
        assert_eq!(
            lines,
            if wrap == TextWrapMode::Wrap {
                vec!["a ", " ", " ", "b"]
            } else {
                vec!["a ", "  b"]
            },
        );
        let max = layout.calculate_content_widths().max;
        layout.break_all_lines(None);
        nearly_eq(layout.full_width(), max);
    }

    // Whitespace in a `Preserve` span still hangs, after breaking at the preceding break-space.
    let mut builder = env.ranged_builder("a   ");
    builder.push_default(StyleProperty::WhiteSpaceCollapse(
        WhiteSpaceCollapse::BreakSpaces,
    ));
    builder.push(
        StyleProperty::WhiteSpaceCollapse(WhiteSpaceCollapse::Preserve),
        2..,
    );
    let mut layout = builder.build("a   ");
    layout.break_all_lines(Some(advance(&mut env, "a ") + 0.1));
    nearly_eq(
        layout.get(0).unwrap().metrics().hanging_advance,
        2. * space - 0.1,
    );
    nearly_eq(
        layout.calculate_content_widths().min,
        advance(&mut env, "a "),
    );
}

#[test]
fn break_spaces_retains_advance_at_forced_breaks_and_block_end() {
    let mut env = TestEnv::new(test_name!(), None);
    for separator in [" ", "\t", "\u{3000}"] {
        let first_line = format!("a{separator}{separator}");
        let full = advance(&mut env, &first_line);
        for ending in ["", "\r\nb", "\u{2029}b"] {
            let text = format!("{first_line}{ending}");
            for wrap in [TextWrapMode::Wrap, TextWrapMode::NoWrap] {
                let mut layout = build(&mut env, &text, WhiteSpaceCollapse::BreakSpaces, wrap);
                layout.break_all_lines(Some(full + 20.));
                assert_eq!(layout.len(), if ending.is_empty() { 1 } else { 2 });
                let line = layout.get(0).unwrap();
                nearly_eq(line.metrics().advance, full);
                nearly_eq(line.metrics().hanging_advance, 0.);
                nearly_eq(layout.calculate_content_widths().max, full);
                layout.align(Alignment::Right, AlignmentOptions::default());
                nearly_eq(layout.get(0).unwrap().metrics().offset, 20.);
                layout.align(Alignment::Center, AlignmentOptions::default());
                nearly_eq(layout.get(0).unwrap().metrics().offset, 10.);
            }
        }
    }
}

#[test]
fn break_spaces_ideographic_space_and_nonbreaking_separators() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "aa\u{3000}\u{3000}b";
    for word_break in [WordBreak::Normal, WordBreak::BreakAll, WordBreak::KeepAll] {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::WhiteSpaceCollapse(
            WhiteSpaceCollapse::BreakSpaces,
        ));
        builder.push_default(StyleProperty::WordBreak(word_break));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(advance(&mut env, "aa") + 0.1));
        let first_line = layout.get(0).unwrap();
        assert_eq!(
            &text[first_line.text_range()],
            if word_break == WordBreak::BreakAll {
                "a"
            } else {
                "aa\u{3000}"
            },
        );
        nearly_eq(first_line.metrics().hanging_advance, 0.);
        layout.break_all_lines(Some(0.));
        nearly_eq(layout.full_width(), layout.calculate_content_widths().min);
    }
    for separator in ["\u{a0}", "\u{2007}", "\u{202f}"] {
        let text = format!("a{separator}{separator}b");
        let mut layout = build(
            &mut env,
            &text,
            WhiteSpaceCollapse::BreakSpaces,
            TextWrapMode::Wrap,
        );
        layout.break_all_lines(Some(0.));
        assert_eq!(layout.len(), 1);
        nearly_eq(layout.get(0).unwrap().metrics().hanging_advance, 0.);
        nearly_eq(layout.calculate_content_widths().min, layout.full_width());
    }
}

#[test]
fn break_spaces_emergency_wrap_and_spacing() {
    let mut env = TestEnv::new(test_name!(), None);
    for overflow_wrap in [
        OverflowWrap::Normal,
        OverflowWrap::Anywhere,
        OverflowWrap::BreakWord,
    ] {
        let text = "a  ";
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::WhiteSpaceCollapse(
            WhiteSpaceCollapse::BreakSpaces,
        ));
        builder.push_default(StyleProperty::OverflowWrap(overflow_wrap));
        builder.push_default(StyleProperty::LetterSpacing(2.));
        builder.push_default(StyleProperty::WordSpacing(3.));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(0.));
        let first_line = layout.get(0).unwrap();
        assert_eq!(
            &text[first_line.text_range()],
            if overflow_wrap == OverflowWrap::Normal {
                "a "
            } else {
                "a"
            },
        );
        if overflow_wrap != OverflowWrap::BreakWord {
            nearly_eq(layout.full_width(), layout.calculate_content_widths().min);
        }
        let max = layout.calculate_content_widths().max;
        layout.break_all_lines(None);
        nearly_eq(layout.full_width(), max);
    }
}

#[test]
fn break_spaces_inline_boxes_and_rtl() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "a   b";
    let width = advance(&mut env, " ");
    for direction in [BaseDirection::Ltr, BaseDirection::Rtl] {
        for kind in [
            InlineBoxKind::InFlow,
            InlineBoxKind::OutOfFlow,
            InlineBoxKind::CustomOutOfFlow,
        ] {
            let mut builder = env.ranged_builder(text);
            builder.set_base_direction(direction);
            builder.push_default(StyleProperty::WhiteSpaceCollapse(
                WhiteSpaceCollapse::BreakSpaces,
            ));
            builder.push_inline_box(InlineBox {
                id: 0,
                index: 2,
                width,
                height: 10.,
                baseline: None,
                vertical_align: VerticalAlign::BASELINE,
                kind,
            });
            let mut layout = builder.build(text);
            layout.break_all_lines(Some(width + 0.1));
            assert_eq!(
                layout.len(),
                if kind == InlineBoxKind::InFlow { 5 } else { 4 }
            );
            for line in layout.lines() {
                nearly_eq(line.metrics().hanging_advance, 0.);
            }
            nearly_eq(layout.full_width(), layout.calculate_content_widths().min);
        }
    }
}

#[test]
fn trailing_whitespace_takes_paragraph_direction() {
    let mut env = TestEnv::new(test_name!(), None);

    // Following UAX #9 rule L1, the whitespace at the end of a line is reset to the paragraph's
    // level, so it ends up at the line's end edge, where it hangs (CSS Text 4 § 4.3.2). That
    // includes whitespace ending text of the opposite direction.
    let arabic = "\u{0627}\u{0628}\u{062A}";
    let space = advance(&mut env, " ");
    for (word, direction) in [
        ("aaaa", BaseDirection::Ltr),
        ("aaaa", BaseDirection::Rtl),
        (arabic, BaseDirection::Ltr),
        (arabic, BaseDirection::Rtl),
    ] {
        let text = format!("{word} {word}");
        let word_advance = advance(&mut env, word);
        let is_rtl = direction == BaseDirection::Rtl;
        for collapse in [
            WhiteSpaceCollapse::Collapse,
            WhiteSpaceCollapse::Preserve,
            WhiteSpaceCollapse::BreakSpaces,
        ] {
            let hangs = collapse != WhiteSpaceCollapse::BreakSpaces;
            // The first line's content without its hanging whitespace.
            let width = if hangs {
                word_advance
            } else {
                word_advance + space
            };
            let mut builder = env.ranged_builder(&text);
            builder.set_base_direction(direction);
            builder.push_default(StyleProperty::WhiteSpaceCollapse(collapse));
            let mut layout = builder.build(&text);
            nearly_eq(layout.calculate_content_widths().min, width);

            layout.break_all_lines(Some(width));
            assert_eq!(layout.len(), 2, "{text:?} {direction:?} {collapse:?}");
            let line = layout.get(0).unwrap();
            assert_eq!(&text[line.text_range()], format!("{word} "));
            nearly_eq(
                line.metrics().hanging_advance,
                if hangs { space } else { 0. },
            );
            nearly_eq(layout.width(), width);

            for alignment in [
                Alignment::Start,
                Alignment::End,
                Alignment::Left,
                Alignment::Center,
                Alignment::Right,
            ] {
                // The content fits the line exactly, so the word is in the same place under every
                // alignment, and the space is at the line's end edge.
                layout.align(alignment, AlignmentOptions::default());
                let extent = |range: Range<usize>| {
                    range
                        .filter(|&index| text.is_char_boundary(index))
                        .map(|index| {
                            let cluster = Cluster::from_byte_index(&layout, index).unwrap();
                            let offset = cluster.visual_offset().unwrap();
                            (offset, offset + cluster.advance())
                        })
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |a, b| {
                            (a.0.min(b.0), a.1.max(b.1))
                        })
                };
                let word_extent = extent(0..word.len());
                let space_extent = extent(word.len()..word.len() + 1);
                let space_cluster = Cluster::from_byte_index(&layout, word.len()).unwrap();
                assert_eq!(space_cluster.is_rtl(), is_rtl);
                let word_start = if hangs || !is_rtl { 0. } else { space };
                nearly_eq(word_extent.0, word_start);
                nearly_eq(word_extent.1, word_start + word_advance);
                let space_start = if is_rtl {
                    word_start - space
                } else {
                    word_advance
                };
                nearly_eq(space_extent.0, space_start);
                nearly_eq(space_extent.1, space_start + space);
            }
        }
    }
}
