// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    test_name,
    util::{ColorBrush, TestEnv},
};
use parley::{
    Alignment, AlignmentOptions, BaseDirection, BreakReason, IndentOptions, InlineBox,
    InlineBoxKind, Layout, StyleProperty, TextWrapMode, WhiteSpaceCollapse,
};

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-3, "{actual} != {expected}");
}

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

fn advance(env: &mut TestEnv, text: &str) -> f32 {
    let mut layout = build(
        env,
        text,
        WhiteSpaceCollapse::Preserve,
        TextWrapMode::NoWrap,
    );
    layout.break_all_lines(None);
    layout.full_width()
}

#[test]
fn hanging_at_block_end_and_forced_breaks() {
    let mut env = TestEnv::new(test_name!(), None);
    let word = advance(&mut env, "a");
    let full = advance(&mut env, "a   ");
    let spaces = full - word;
    for mode in [
        WhiteSpaceCollapse::Collapse,
        WhiteSpaceCollapse::PreserveBreaks,
        WhiteSpaceCollapse::Preserve,
    ] {
        for wrap in [TextWrapMode::Wrap, TextWrapMode::NoWrap] {
            for width in [word - 1., word, word + spaces / 2., full, full + 10.] {
                for text in [
                    "a   ",
                    "a   \na",
                    "a   \r\na",
                    "a   \u{2028}a",
                    "a   \u{2029}a",
                ] {
                    let mut layout = build(&mut env, text, mode, wrap);
                    layout.break_all_lines(Some(width));
                    assert_eq!(layout.len(), if text.ends_with('a') { 2 } else { 1 });
                    let line = layout.get(0).unwrap();
                    close(line.metrics().advance, full);
                    let expected = match (mode, wrap) {
                        (WhiteSpaceCollapse::Preserve, TextWrapMode::NoWrap) => 0.,
                        (WhiteSpaceCollapse::Preserve, TextWrapMode::Wrap) => {
                            (full - width).max(0.).min(spaces)
                        }
                        _ => spaces,
                    };
                    close(line.metrics().hanging_advance, expected);
                    let widths = layout.calculate_content_widths();
                    close(
                        widths.max,
                        if mode == WhiteSpaceCollapse::Preserve {
                            full
                        } else {
                            word
                        },
                    );
                    close(
                        widths.min,
                        if mode == WhiteSpaceCollapse::Preserve && wrap == TextWrapMode::NoWrap {
                            full
                        } else {
                            word
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn hanging_soft_wrap_consumes_the_whole_suffix() {
    let mut env = TestEnv::new(test_name!(), None);
    let word = advance(&mut env, "a");
    let full = advance(&mut env, "a    ");
    for mode in [
        WhiteSpaceCollapse::Collapse,
        WhiteSpaceCollapse::PreserveBreaks,
        WhiteSpaceCollapse::Preserve,
    ] {
        let mut layout = build(&mut env, "a    b", mode, TextWrapMode::Wrap);
        layout.break_all_lines(Some(word + 0.1));
        assert_eq!(layout.len(), 2);
        let line = layout.get(0).unwrap();
        assert_eq!(line.break_reason(), BreakReason::Regular);
        close(line.metrics().advance, full);
        close(line.metrics().hanging_advance, full - word);
    }
    let mut spaces = build(
        &mut env,
        "        ",
        WhiteSpaceCollapse::Preserve,
        TextWrapMode::Wrap,
    );
    let full = advance(&mut env, "        ");
    spaces.break_all_lines(Some(1.));
    assert_eq!(spaces.len(), 1);
    close(spaces.get(0).unwrap().metrics().hanging_advance, full - 1.);
    close(spaces.calculate_content_widths().min, 0.);
    close(spaces.calculate_content_widths().max, full);
}

#[test]
fn hanging_mixed_policies_and_shaped_runs() {
    let mut env = TestEnv::new(test_name!(), None);
    let word = advance(&mut env, "a");
    let space = advance(&mut env, " ");
    for index in 1..=3 {
        let mut builder = env.ranged_builder("a   ");
        builder.push_default(StyleProperty::WhiteSpaceCollapse(
            WhiteSpaceCollapse::Preserve,
        ));
        let unconditional = index..index + 1;
        builder.push(
            StyleProperty::WhiteSpaceCollapse(WhiteSpaceCollapse::Collapse),
            unconditional.clone(),
        );
        builder.push(StyleProperty::FontSize(24.), unconditional);
        let mut layout = builder.build("a   ");
        layout.break_all_lines(None);
        let full = layout.full_width();
        let after = (3 - index) as f32 * space;
        let terminal_always = if index == 3 { full - word } else { 0. };
        close(
            layout.get(0).unwrap().metrics().hanging_advance,
            terminal_always,
        );
        let widths = layout.calculate_content_widths();
        close(widths.max, full - terminal_always);
        close(widths.min, word);
        for overflow in [0., after / 2., after + 0.1, full - word - space / 2., full] {
            layout.break_all_lines(Some(full - overflow));
            let expected = if overflow >= after {
                full - word
            } else {
                overflow
            };
            close(layout.get(0).unwrap().metrics().hanging_advance, expected);
        }
    }
}

#[test]
fn preserved_whitespace_before_unconditional_hanging_is_not_terminal() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "X  \u{3000}\nX";
    let word = advance(&mut env, "X");
    let mut builder = env.ranged_builder(text);
    builder.push(
        StyleProperty::WhiteSpaceCollapse(WhiteSpaceCollapse::Collapse),
        3..6,
    );
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(200.));
    assert_eq!(layout.len(), 2);
    let line = layout.get(0).unwrap();
    assert_eq!(line.break_reason(), BreakReason::Explicit);
    close(
        line.metrics().advance - line.metrics().hanging_advance,
        word,
    );
    close(layout.calculate_content_widths().max, word);
    layout.align(Alignment::Right, AlignmentOptions::default());
    close(layout.get(0).unwrap().metrics().offset, 200. - word);
}

#[test]
fn hanging_separator_categories_and_partial_atoms() {
    let mut env = TestEnv::new(test_name!(), None);
    for suffix in [
        "\u{00a0}", "\u{000b}", "\u{000c}", "\u{0085}", "\u{1680}", "\u{2000}", "\u{2001}",
        "\u{2002}", "\u{2003}", "\u{2004}", "\u{2005}", "\u{2006}", "\u{2007}", "\u{2008}",
        "\u{2009}", "\u{200a}", "\u{202f}", "\u{205f}", "\t", "\u{3000}",
    ] {
        let text = format!("a{suffix}");
        let full = advance(&mut env, &text);
        let mut layout = build(
            &mut env,
            &text,
            WhiteSpaceCollapse::Collapse,
            TextWrapMode::NoWrap,
        );
        layout.break_all_lines(None);
        let hangs = matches!(suffix, "\t" | "\u{3000}");
        let expected = if hangs {
            full - advance(&mut env, "a")
        } else {
            0.
        };
        close(layout.get(0).unwrap().metrics().hanging_advance, expected);
    }
    let text = "a\u{0d4e} ";
    let full = advance(&mut env, text);
    let mut layout = build(
        &mut env,
        text,
        WhiteSpaceCollapse::Preserve,
        TextWrapMode::Wrap,
    );
    let widths = layout.calculate_content_widths();
    assert!(widths.min < full);
    close(widths.max, full);
    layout.break_all_lines(Some(widths.min + (full - widths.min) / 2.));
    assert_eq!(layout.len(), 1);
    close(layout.width(), (widths.min + full) / 2.);
}

#[test]
fn hanging_inline_box_barriers_and_nowrap_suffix() {
    let mut env = TestEnv::new(test_name!(), None);
    let full = advance(&mut env, "a   ");
    for kind in [
        InlineBoxKind::InFlow,
        InlineBoxKind::OutOfFlow,
        InlineBoxKind::CustomOutOfFlow,
    ] {
        let mut builder = env.ranged_builder("a   ");
        builder.push_default(StyleProperty::WhiteSpaceCollapse(
            WhiteSpaceCollapse::Collapse,
        ));
        builder.push_inline_box(InlineBox {
            id: 0,
            index: 4,
            width: 10.,
            height: 10.,
            baseline: None,
            kind,
        });
        let mut layout = builder.build("a   ");
        layout.break_all_lines(None);
        let expected = if kind == InlineBoxKind::InFlow {
            0.
        } else {
            full - advance(&mut env, "a")
        };
        close(layout.get(0).unwrap().metrics().hanging_advance, expected);
        close(layout.width(), layout.calculate_content_widths().max);
    }
    let mut builder = env.ranged_builder("a   ");
    builder.push_default(StyleProperty::WhiteSpaceCollapse(
        WhiteSpaceCollapse::Collapse,
    ));
    builder.push(
        StyleProperty::WhiteSpaceCollapse(WhiteSpaceCollapse::Preserve),
        3..,
    );
    builder.push(StyleProperty::TextWrapMode(TextWrapMode::NoWrap), 3..);
    let mut layout = builder.build("a   ");
    layout.break_all_lines(None);
    close(layout.get(0).unwrap().metrics().hanging_advance, 0.);
    close(layout.calculate_content_widths().max, full);
}

#[test]
fn hanging_alignment_bidi_and_indent() {
    let mut env = TestEnv::new(test_name!(), None);
    let full = advance(&mut env, "a  ");
    for direction in [BaseDirection::Ltr, BaseDirection::Rtl] {
        let mut builder = env.ranged_builder("a  ");
        builder.set_base_direction(direction);
        let mut layout = builder.build("a  ");
        for width in [full + 10., full - 1.] {
            layout.break_all_lines(Some(width));
            let hanging = layout.get(0).unwrap().metrics().hanging_advance;
            close(hanging, (full - width).max(0.));
            layout.align(Alignment::Center, AlignmentOptions::default());
            let expected = (width - full + hanging) / 2.
                - if direction == BaseDirection::Rtl {
                    hanging
                } else {
                    0.
                };
            close(layout.get(0).unwrap().metrics().offset, expected);
        }
    }
    let mut layout = build(
        &mut env,
        "a  ",
        WhiteSpaceCollapse::Preserve,
        TextWrapMode::Wrap,
    );
    layout.set_text_indent(10., IndentOptions::default());
    layout.break_all_lines(Some(full + 9.));
    close(layout.get(0).unwrap().metrics().hanging_advance, 1.);
}

#[test]
fn conditional_hanging_requires_positive_overflow() {
    let mut env = TestEnv::new(test_name!(), None);
    let word = advance(&mut env, "abc");
    let text = "abc ";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::WordSpacing(-20.));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let full = layout.full_width();
    assert!(full < word, "{full} >= {word}");
    // The space fits (it has a negative advance), so it doesn't hang.
    close(layout.get(0).unwrap().metrics().hanging_advance, 0.);
    close(layout.width(), full);
    let widths = layout.calculate_content_widths();
    close(widths.max, full);
    close(widths.min, word);
}

#[test]
fn zero_width_conditional_whitespace_blocks_preceding_unconditional() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "abc  ";
    let mut builder = env.ranged_builder(text);
    builder.push(
        StyleProperty::WhiteSpaceCollapse(WhiteSpaceCollapse::Collapse),
        3..4,
    );
    builder.push(StyleProperty::FontSize(0.), 4..5);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let full = layout.full_width();
    // The zero-width preserved space fits, so it doesn't hang, and the collapsible space
    // before it is not at the end of the line.
    close(layout.get(0).unwrap().metrics().hanging_advance, 0.);
    close(layout.width(), full);
    close(layout.calculate_content_widths().max, full);
}
