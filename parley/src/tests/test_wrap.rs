// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use peniko::color::palette::css;

use crate::{
    Alignment, AlignmentOptions, LineBreakWordOption, OverflowWrap, StyleProperty, test_name,
};

use super::utils::{ColorBrush, TestEnv};

fn test_wrap(
    env: &mut TestEnv,
    pattern: Option<&str>,
    wrap_property: StyleProperty<'_, ColorBrush>,
    color: ColorBrush,
    wrap_width: f32,
) {
    test_wrap_with_custom_text(
        env,
        "Most words are short. But Antidisestablishmentarianism is long and needs to wrap.",
        pattern,
        wrap_property,
        color,
        wrap_width,
    );
}

fn test_wrap_with_custom_text(
    env: &mut TestEnv,
    text: &str,
    pattern: Option<&str>,
    wrap_property: StyleProperty<'_, ColorBrush>,
    color: ColorBrush,
    wrap_width: f32,
) {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::Brush(ColorBrush::new(css::RED)));

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
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        None,
        StyleProperty::OverflowWrap(OverflowWrap::default()),
        ColorBrush::default(),
        120.0,
    );
}

#[test]
fn overflow_wrap_first_half() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("Antidis"),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(css::BLUE),
        120.0,
    );
}

#[test]
fn overflow_wrap_second_half() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("anism"),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(css::BLUE),
        120.0,
    );
}

#[test]
fn overflow_wrap_during() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("establishment"),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(css::BLUE),
        120.0,
    );
}

#[test]
fn overflow_wrap_everywhere() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("Most words are short. But Antidisestablishmentarianism is long and needs to wrap."),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(css::BLUE),
        120.0,
    );
}

#[test]
fn overflow_wrap_narrow() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("Most words are short. But Antidisestablishmentarianism is long and needs to wrap."),
        StyleProperty::OverflowWrap(OverflowWrap::Anywhere),
        ColorBrush::new(css::BLUE),
        5.0,
    );
}

#[test]
fn overflow_wrap_anywhere_min_content_width() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Hello world!\nLonger line with a looooooooong word.";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));

    let mut layout = builder.build(text);

    layout.break_all_lines(Some(layout.calculate_content_widths().min));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

#[test]
fn overflow_wrap_break_word_min_content_width() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Hello world!\nLonger line with a looooooooong word.";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::BreakWord));

    let mut layout = builder.build(text);

    layout.break_all_lines(Some(layout.calculate_content_widths().min));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    env.check_layout_snapshot(&layout);
}

#[test]
fn word_break_break_all_first_half() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("Antidis"),
        StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
        ColorBrush::new(css::GREEN),
        120.0,
    );
}

#[test]
fn word_break_break_all_second_half() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("anism"),
        StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
        ColorBrush::new(css::GREEN),
        120.0,
    );
}

#[test]
fn word_break_break_all_during() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("establishment"),
        StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
        ColorBrush::new(css::GREEN),
        120.0,
    );
}

#[test]
fn word_break_break_all_everywhere() {
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap(
        &mut env,
        Some("Most words are short. But Antidisestablishmentarianism is long and needs to wrap."),
        StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
        ColorBrush::new(css::GREEN),
        120.0,
    );
}

#[test]
fn word_break_break_all_min_content_width() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "Hello world!\nLonger line with a looooooooong word.";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::WordBreak(LineBreakWordOption::BreakAll));

    let mut layout = builder.build(text);

    layout.break_all_lines(Some(layout.calculate_content_widths().min));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    // This snapshot will have slightly different line wrapping than the corresponding overflow-wrap test. This is to be
    // expected and matches browser/CSS behavior.
    env.check_layout_snapshot(&layout);
}

#[test]
fn word_break_wpt007() {
    // See http://wpt.live/css/css-text/word-break/word-break-break-all-inline-007.tentative.html
    //
    // All browsers fail this currently, but we pass it. This means that word_break_break_all_first_half doesn't match
    // what any browsers do currently, but should be theoretically correct.
    let mut env = TestEnv::new(test_name!(), None);

    test_wrap_with_custom_text(
        &mut env,
        "aaaaaaabbbbbbbcccccc",
        Some("bbbbbbb"),
        StyleProperty::WordBreak(LineBreakWordOption::BreakAll),
        ColorBrush::new(css::GREEN),
        55.0,
    );
}

#[test]
fn word_break_keep_all() {
    let mut env = TestEnv::new(test_name!(), None);

    let mut test_text = |text, name, wrap_width| {
        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::WordBreak(LineBreakWordOption::KeepAll));

        let mut layout = builder.build(text);

        layout.break_all_lines(Some(wrap_width));
        layout.align(None, Alignment::Start, AlignmentOptions::default());
        env.with_name(name).check_layout_snapshot(&layout);
    };

    // These are the word-break-keep-all tests from WPT:
    // https://wpt.fyi/results/css/css-text/word-break?label=experimental&label=master&aligned
    test_text("Latin latin latin latin", "latin", 120.0);
    // These will all show up as boxes because CJK fonts are quite large (several megabytes per language) and could
    // bloat the repository. Line break analysis should work the same regardless of font, however.
    test_text("日本語 日本語 日本語", "japanese", 60.0);
    // Jamo decomposed on purpose
    test_text("한글이 한글이 한글이", "korean", 60.0);
    // TODO: we fail this test; so does Safari
    // https://wpt.fyi/results/css/css-text/word-break/word-break-keep-all-003.html
    // test_text("และ และและ", "thai", 65.0);
    test_text("フォ フォ", "ID_and_CJ", 30.0);
    // Jamo decomposed on purpose
    test_text("애기판다 애기판다", "korean_hangul_jamos", 90.0);
}
