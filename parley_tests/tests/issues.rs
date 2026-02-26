// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Regression tests for specific issues.

use crate::test_name;
use crate::util::TestEnv;
use parley::{Alignment, AlignmentOptions, PositionedLayoutItem, StyleProperty, TextWrapMode};

/// Test that rendering RTL text doesn't affect subsequent LTR layouts.
/// See <https://github.com/linebender/parley/issues/489>.
#[test]
fn issue_489() {
    let mut env = TestEnv::new(test_name!(), None);

    // First, render some RTL text
    {
        let text = "مرحبا"; // Arabic "Hello"
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(200.0));
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        assert!(layout.is_rtl());
    }

    // Now render LTR text - this shouldn't be affected by the previous RTL layout
    {
        let text = "ABC";
        let builder = env.ranged_builder(text);
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(200.0));
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        assert!(!layout.is_rtl());

        let line = layout.lines().next().unwrap();
        let item = line.items().next().unwrap();
        let glyph_run = match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => glyph_run,
            PositionedLayoutItem::InlineBox(_) => unreachable!(),
        };

        assert!(!glyph_run.run().is_rtl());

        // For LTR text, positioned glyphs should have increasing x coordinates
        let positions: Vec<f32> = glyph_run.positioned_glyphs().map(|g| g.x).collect();
        for i in 1..positions.len() {
            assert!(
                positions[i] > positions[i - 1],
                "LTR positioned glyphs should have increasing x coordinates. Got: {:?}",
                positions
            );
        }
    }
}

/// Test that justified text is correctly aligned.
/// See <https://github.com/linebender/parley/issues/409>.
#[test]
fn issue_409() {
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

/// Test that mandatory line breaks don't zero-out the max content width.
///
/// When a mandatory line break occurs we should set the `running_max_width`
/// to 0 and start counting `running_max_width` again. However, we should first
/// accumulate the `running_max_width` into the overall `max_width`. Otherwise
/// the width of the line we have just ended is not accounted for.
///
/// This was causing text in Blitz to wrap to the min-content width in cases
/// where a paragraph ended with a newline as the max-content width was
/// entirely zeroed out!
#[test]
fn max_context_with_mandatory_breaks() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "First line of text.
Second line of text.
Third line that ends with newlines\n\n";

    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::TextWrapMode(TextWrapMode::NoWrap));
    let mut layout = builder.build(text);

    let content_widths = layout.calculate_content_widths();
    assert!(content_widths.max != 0.0);

    layout.break_all_lines(Some(content_widths.max));
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    env.with_name("max_context_with_mandatory_breaks")
        .check_layout_snapshot(&layout);
}
