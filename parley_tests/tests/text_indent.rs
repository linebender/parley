// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text indent tests.

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{Alignment, AlignmentOptions, IndentOptions};

fn build_indented_layout(
    env: &mut TestEnv,
    text: &str,
    indent_amount: f32,
    indent_options: IndentOptions,
    wrap_width: f32,
    alignment: Alignment,
) -> parley::Layout<ColorBrush> {
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.indent(indent_amount, indent_options);
    layout.break_all_lines(Some(wrap_width));
    layout.align(None, alignment, AlignmentOptions::default());
    layout
}

#[test]
fn text_indent_basic() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog and keeps on running far away.";
    let layout = build_indented_layout(
        &mut env,
        text,
        50.0,
        IndentOptions::default(),
        200.0,
        Alignment::Start,
    );

    assert!(layout.len() > 1, "Expected multiple lines");
    let first_line = layout.get(0).unwrap();
    assert_eq!(first_line.metrics().offset, 50.0);
    for i in 1..layout.len() {
        let line = layout.get(i).unwrap();
        assert_eq!(line.metrics().offset, 0.0, "Line {i} should have no indent");
    }

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_no_wrap() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "Short text.";
    let layout = build_indented_layout(
        &mut env,
        text,
        30.0,
        IndentOptions::default(),
        500.0,
        Alignment::Start,
    );

    assert_eq!(layout.len(), 1);
    let line = layout.get(0).unwrap();
    assert_eq!(line.metrics().offset, 30.0);

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_each_line() {
    let mut env = TestEnv::new(test_name!(), None);
    let text =
        "First paragraph line one.\nSecond paragraph also indented.\nThird paragraph here too.";
    let layout = build_indented_layout(
        &mut env,
        text,
        40.0,
        IndentOptions {
            each_line: true,
            hanging: false,
        },
        250.0,
        Alignment::Start,
    );

    for i in 0..layout.len() {
        let line = layout.get(i).unwrap();
        let is_first_or_after_hard_break = i == 0 || {
            let prev_text = &text[layout.get(i - 1).unwrap().text_range()];
            prev_text.ends_with('\n')
        };
        if is_first_or_after_hard_break {
            assert_eq!(
                line.metrics().offset,
                40.0,
                "Line {i} should be indented after hard break"
            );
        }
    }

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_hanging() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog and keeps on running far away.";
    let layout = build_indented_layout(
        &mut env,
        text,
        50.0,
        IndentOptions {
            each_line: false,
            hanging: true,
        },
        200.0,
        Alignment::Start,
    );

    assert!(layout.len() > 1, "Expected multiple lines");
    let first_line = layout.get(0).unwrap();
    assert_eq!(
        first_line.metrics().offset,
        0.0,
        "First line should NOT be indented with hanging"
    );
    for i in 1..layout.len() {
        let line = layout.get(i).unwrap();
        assert_eq!(
            line.metrics().offset,
            50.0,
            "Line {i} should be indented with hanging"
        );
    }

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_hanging_each_line() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "First paragraph wraps here.\nSecond paragraph also wraps around here too.";
    let layout = build_indented_layout(
        &mut env,
        text,
        40.0,
        IndentOptions {
            each_line: true,
            hanging: true,
        },
        200.0,
        Alignment::Start,
    );

    for i in 0..layout.len() {
        let line = layout.get(i).unwrap();
        let is_scope_line = i == 0 || {
            let prev_text = &text[layout.get(i - 1).unwrap().text_range()];
            prev_text.ends_with('\n')
        };
        if is_scope_line {
            assert_eq!(
                line.metrics().offset,
                0.0,
                "Line {i} (scope line) should NOT be indented with hanging"
            );
        } else {
            assert_eq!(
                line.metrics().offset,
                40.0,
                "Line {i} (continuation) should be indented with hanging"
            );
        }
    }

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_negative() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog and keeps on running.";
    let layout = build_indented_layout(
        &mut env,
        text,
        -20.0,
        IndentOptions::default(),
        200.0,
        Alignment::Start,
    );

    assert!(layout.len() > 1, "Expected multiple lines");
    let first_line = layout.get(0).unwrap();
    assert_eq!(
        first_line.metrics().offset,
        -20.0,
        "First line should have negative offset"
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_negative_hanging() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog and keeps on running.";
    let layout = build_indented_layout(
        &mut env,
        text,
        -20.0,
        IndentOptions {
            each_line: false,
            hanging: true,
        },
        200.0,
        Alignment::Start,
    );

    assert!(layout.len() > 1, "Expected multiple lines");
    let first_line = layout.get(0).unwrap();
    assert_eq!(
        first_line.metrics().offset,
        0.0,
        "First line should have no offset"
    );
    for i in 1..layout.len() {
        let line = layout.get(i).unwrap();
        assert_eq!(
            line.metrics().offset,
            -20.0,
            "Line {i} should have negative offset with hanging"
        );
    }

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_center_alignment() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog.";
    let layout = build_indented_layout(
        &mut env,
        text,
        50.0,
        IndentOptions::default(),
        300.0,
        Alignment::Center,
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_right_alignment() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog.";
    let layout = build_indented_layout(
        &mut env,
        text,
        50.0,
        IndentOptions::default(),
        300.0,
        Alignment::Right,
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_justify() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog and keeps on running far away.";
    let layout = build_indented_layout(
        &mut env,
        text,
        50.0,
        IndentOptions::default(),
        200.0,
        Alignment::Justify,
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_zero() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog.";
    let layout = build_indented_layout(
        &mut env,
        text,
        0.0,
        IndentOptions::default(),
        200.0,
        Alignment::Start,
    );

    for i in 0..layout.len() {
        let line = layout.get(i).unwrap();
        assert_eq!(
            line.metrics().offset,
            0.0,
            "Line {i} should have zero offset"
        );
    }

    env.check_layout_snapshot(&layout);
}

#[test]
fn text_indent_line_breaking() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "The quick brown fox jumps over the lazy dog.";

    let builder_no_indent = env.ranged_builder(text);
    let mut layout_no_indent = builder_no_indent.build(text);
    layout_no_indent.break_all_lines(Some(200.0));
    let lines_no_indent = layout_no_indent.len();

    let layout_with_indent = build_indented_layout(
        &mut env,
        text,
        80.0,
        IndentOptions::default(),
        200.0,
        Alignment::Start,
    );
    let lines_with_indent = layout_with_indent.len();

    assert!(
        lines_with_indent >= lines_no_indent,
        "Indent should cause same or more line breaks: without={lines_no_indent}, with={lines_with_indent}"
    );

    env.check_layout_snapshot(&layout_with_indent);
}
