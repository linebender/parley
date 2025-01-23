// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{testenv, Alignment, InlineBox, WhiteSpaceCollapse};

#[test]
fn plain_multiline_text() {
    let mut env = testenv!();

    let text = "Hello world!\nLine 2\nLine 4";
    let mut builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(
        None,
        Alignment::Start,
        false, /* align_when_overflowing */
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn placing_inboxes() {
    let mut env = testenv!();

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
        layout.align(
            None,
            Alignment::Start,
            false, /* align_when_overflowing */
        );
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
fn only_inboxes_wrap() {
    let mut env = testenv!();

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
    layout.align(
        None,
        Alignment::Middle,
        false, /* align_when_overflowing */
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn full_width_inbox() {
    let mut env = testenv!();

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
        layout.align(
            None,
            Alignment::Start,
            false, /* align_when_overflowing */
        );
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
fn trailing_whitespace() {
    let mut env = testenv!();

    let text = "AAA BBB";
    let mut builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(45.));
    layout.align(None, Alignment::Start, false);

    assert!(
        layout.width() < layout.full_width(),
        "Trailing whitespace should cause a difference between width and full_width"
    );

    env.check_layout_snapshot(&layout);
}

#[test]
fn leading_whitespace() {
    let mut env = testenv!();

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
        layout.align(
            None,
            Alignment::Start,
            false, /* align_when_overflowing */
        );
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
fn base_level_alignment_ltr() {
    let mut env = testenv!();

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
        layout.align(
            Some(150.0),
            alignment,
            false, /* align_when_overflowing */
        );
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}

#[test]
fn base_level_alignment_rtl() {
    let mut env = testenv!();

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
        layout.align(
            Some(150.0),
            alignment,
            false, /* align_when_overflowing */
        );
        env.with_name(test_case_name).check_layout_snapshot(&layout);
    }
}
