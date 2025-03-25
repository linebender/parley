// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Alignment, AlignmentOptions, OverflowWrap, StyleProperty, WordBreak, testenv};

use super::utils::{ColorBrush, TestEnv};

fn test_wrap(
    env: &mut TestEnv,
    pattern: Option<&str>,
    wrap_property: StyleProperty<'_, ColorBrush>,
    color: ColorBrush,
    wrap_width: f32,
) {
    let text = "Most words are short. But Antidisestablishmentarianism is long and needs to wrap.";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::Brush(ColorBrush::new(255, 0, 0, 255)));

    if let Some(pattern) = pattern {
        let start = text.find(pattern).unwrap();
        let range = start..start + pattern.len();
        builder.push(StyleProperty::Brush(color), range.clone());
        builder.push(StyleProperty::Underline(true), range.clone());
        builder.push(wrap_property, range.clone());
    }

    let mut layout = builder.build(text);
    layout.break_all_lines(Some(wrap_width));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
}

#[test]
fn overflow_wrap_off() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        None,
        StyleProperty::OverflowWrap(Default::default()),
        ColorBrush::default(),
        120.0,
    );
}

#[test]
fn overflow_wrap_first_half() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("Antidis"),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(0, 0, 255, 255),
        120.0,
    );
}

#[test]
fn overflow_wrap_second_half() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("anism"),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(0, 0, 255, 255),
        120.0,
    );
}

#[test]
fn overflow_wrap_during() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("establishment"),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(0, 0, 255, 255),
        120.0,
    );
}

#[test]
fn overflow_wrap_everywhere() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("Most words are short. But Antidisestablishmentarianism is long and needs to wrap."),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(0, 0, 255, 255),
        120.0,
    );
}

#[test]
fn overflow_wrap_narrow() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("Most words are short. But Antidisestablishmentarianism is long and needs to wrap."),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(0, 0, 255, 255),
        5.0,
    );
}

#[test]
fn overflow_wrap_anywhere_min_content_width() {
    let mut env = testenv!();

    let text = "Hello world!\nLonger line with a looooooooong word.";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));

    let mut layout = builder.build(text);

    layout.break_all_lines(Some(layout.min_content_width()));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

#[test]
fn overflow_wrap_break_word_min_content_width() {
    let mut env = testenv!();

    let text = "Hello world!\nLonger line with a looooooooong word.";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));

    let mut layout = builder.build(text);

    layout.break_all_lines(Some(layout.min_content_width()));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

#[test]
fn word_break_break_all_first_half() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("Antidis"),
        StyleProperty::WordBreak(WordBreak::BreakAll),
        ColorBrush::new(0, 128, 0, 255),
        120.0,
    );
}

#[test]
fn word_break_break_all_second_half() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("anism"),
        StyleProperty::WordBreak(WordBreak::BreakAll),
        ColorBrush::new(0, 128, 0, 255),
        120.0,
    );
}

#[test]
fn word_break_break_all_during() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("establishment"),
        StyleProperty::WordBreak(WordBreak::BreakAll),
        ColorBrush::new(0, 128, 0, 255),
        120.0,
    );
}

#[test]
fn word_break_break_all_everywhere() {
    let mut env = testenv!();

    test_wrap(
        &mut env,
        Some("Most words are short. But Antidisestablishmentarianism is long and needs to wrap."),
        StyleProperty::WordBreak(WordBreak::BreakAll),
        ColorBrush::new(0, 128, 0, 255),
        120.0,
    );
}

#[test]
fn word_break_break_all_min_content_width() {
    let mut env = testenv!();

    let text = "Hello world!\nLonger line with a looooooooong word.";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::WordBreak(WordBreak::BreakAll));

    let mut layout = builder.build(text);

    layout.break_all_lines(Some(layout.min_content_width()));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    // This snapshot will have slightly different line wrapping than the corresponding overflow-wrap test. This is to be
    // expected and matches browser/CSS behavior.
    env.check_layout_snapshot(&layout);
}

#[test]
fn word_break_keep_all() {
    let mut env = testenv!();

    let mut test_text = |text, name, wrap_width| {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::WordBreak(WordBreak::KeepAll));

        let mut layout = builder.build(text);

        layout.break_all_lines(Some(wrap_width));
        layout.align(None, Alignment::Start, AlignmentOptions::default());
        env.with_name(name).check_layout_snapshot(&layout);
    };

    test_text("Latin latin latin latin", "latin", 120.0);
    // These will all show up as boxes because CJK fonts are quite large (several megabytes per language) and could
    // bloat the repository. Line break analysis should work the same regardless of font, however.
    test_text("日本語 日本語 日本語", "japanese", 60.0);
    test_text("한글이 한글이 한글이", "korean", 60.0);
    // TODO: we fail this test; so does Safari
    // https://wpt.fyi/results/css/css-text/word-break/word-break-keep-all-003.html
    // test_text("และ และและ", "thai", 65.0);
    test_text("フォ フォ", "ID_and_CJ", 30.0);
    test_text("애기판다 애기판다", "korean_hangul_jamos", 90.0);
}
