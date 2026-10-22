// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tab stop tests.
//!
//! Tabs shift the following glyph to the next tab stop, following CSS Text 4 § 4.1.3. The tests
//! measure the shift against the advance of the space character, as [`TabSize::Spaces`] is a
//! multiple of it.

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{Layout, StyleProperty, TabSize, TextWrapMode, WhiteSpaceCollapse};

fn nearly_eq(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-3, "{actual} != {expected}");
}

/// The advance of `text` on a single line, with none of it hanging.
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

/// Lays out `text` on one line, with the given tab size.
fn build(env: &mut TestEnv, text: &str, tab_size: TabSize) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(tab_size));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout
}

#[test]
fn tab_advances_the_line_to_the_next_stop() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");
    let b = advance(&mut env, "b");
    assert!(a < 8. * space, "the tab stop is past the first letter");

    // The default tab size is eight spaces.
    let layout = build(&mut env, "a\tb", TabSize::default());
    nearly_eq(layout.width(), 8. * space + b);

    let layout = build(&mut env, "a\tb", TabSize::Spaces(4.));
    nearly_eq(layout.width(), 4. * space + b);

    // Once the text is past the first stop, the tab goes to the second.
    let aa = advance(&mut env, "aa");
    assert!(aa > 4. * space && aa < 8. * space - space / 2.);
    let layout = build(&mut env, "aa\tb", TabSize::Spaces(4.));
    nearly_eq(layout.width(), 8. * space + b);

    // Consecutive tabs each advance a stop.
    let layout = build(&mut env, "a\t\t\tb", TabSize::Spaces(4.));
    nearly_eq(layout.width(), 12. * space + b);

    // A tab size of zero renders the tab as nothing.
    let layout = build(&mut env, "a\tb", TabSize::Spaces(0.));
    nearly_eq(layout.width(), advance(&mut env, "ab"));
}

#[test]
fn tab_sized_when_breaking_by_length() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let ab = advance(&mut env, "ab");
    let c = advance(&mut env, "c");
    assert!(ab < 8. * space);

    let text = "ab\tc\tab";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(8.)));
    let mut layout = builder.build(text);
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(3);
    breaker.break_next_with_length(4);
    breaker.finish();

    assert_eq!(layout.len(), 2);
    // The first line is "ab\t", with the tab hanging off the end of the wrapped line.
    let line = layout.get(0).unwrap();
    nearly_eq(line.metrics().advance, 8. * space);
    nearly_eq(line.metrics().hanging_advance, 8. * space - ab);
    // The second line starts with "c" at the line start, then a tab to the first stop.
    let line = layout.get(1).unwrap();
    nearly_eq(line.metrics().advance, 8. * space + ab);
    assert!(c < 8. * space);
}

#[test]
fn tab_trailing_hangs_like_a_space() {
    let mut env = TestEnv::new(test_name!(), None);
    let space = advance(&mut env, " ");
    let a = advance(&mut env, "a");

    // Under `WhiteSpaceCollapse::Collapse`, trailing whitespace hangs unconditionally, so the tab
    // is excluded from the layout's width.
    let text = "a\t";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TabSize(TabSize::Spaces(4.)));
    builder.push_default(StyleProperty::WhiteSpaceCollapse(
        WhiteSpaceCollapse::Collapse,
    ));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    nearly_eq(layout.width(), a);
    nearly_eq(layout.full_width(), 4. * space);
    nearly_eq(
        layout.get(0).unwrap().metrics().hanging_advance,
        4. * space - a,
    );

    // Under `WhiteSpaceCollapse::Preserve`, whitespace before the end of the layout hangs only as
    // far as it overflows, like a space would.
    let layout = build(&mut env, "a\t", TabSize::Spaces(4.));
    nearly_eq(layout.width(), 4. * space);
    nearly_eq(layout.full_width(), 4. * space);
}
