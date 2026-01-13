// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for spacing style properties.

use alloc::format;

use crate::layout::Alignment;
use crate::style::{LineHeight, StyleProperty};
use crate::test_name;
use crate::tests::utils::{ColorBrush, TestEnv, samples};
use crate::{AlignmentOptions, Layout};

/// Helper to build a layout with line height applied
fn build_with_line_height(
    env: &mut TestEnv,
    text: &str,
    line_height: LineHeight,
) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::LineHeight(line_height));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    layout
}

/// Helper to build a layout with letter spacing applied
fn build_with_letter_spacing(env: &mut TestEnv, text: &str, spacing: f32) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::LetterSpacing(spacing));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    layout
}

/// Helper to build a layout with word spacing applied
fn build_with_word_spacing(env: &mut TestEnv, text: &str, spacing: f32) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::WordSpacing(spacing));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    layout
}

// ============================================================================
// LineHeight Tests
// ============================================================================

#[test]
fn style_line_height_absolute() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN_MULTILINE;

    for height in [16.0, 24.0, 32.0, 48.0] {
        let layout = build_with_line_height(&mut env, text, LineHeight::Absolute(height));

        env.with_name(&format!("abs_{height}"))
            .check_layout_snapshot(&layout);
    }
}

#[test]
fn style_line_height_font_relative() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN_MULTILINE;

    for factor in [1.0, 1.2, 1.5, 2.0] {
        let layout = build_with_line_height(&mut env, text, LineHeight::FontSizeRelative(factor));

        // Use underscore in name to avoid dots in filename
        let name = format!("rel_{}", (factor * 10.0) as i32);
        env.with_name(&name).check_layout_snapshot(&layout);
    }
}

#[test]
fn style_line_height_metrics_relative() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN_MULTILINE;

    for factor in [0.8, 1.0, 1.2, 1.5] {
        let layout = build_with_line_height(&mut env, text, LineHeight::MetricsRelative(factor));

        let name = format!("metrics_{}", (factor * 10.0) as i32);
        env.with_name(&name).check_layout_snapshot(&layout);
    }
}

// ============================================================================
// LetterSpacing Tests
// ============================================================================

#[test]
fn style_letter_spacing_positive() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    for spacing in [1.0, 2.0, 4.0, 8.0] {
        let layout = build_with_letter_spacing(&mut env, text, spacing);

        env.with_name(&format!("pos_{spacing}"))
            .check_layout_snapshot(&layout);
    }
}

#[test]
fn style_letter_spacing_negative() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LATIN;

    for spacing in [-1.0, -2.0] {
        let layout = build_with_letter_spacing(&mut env, text, spacing);

        let name = format!("neg_{}", (-spacing) as i32);
        env.with_name(&name).check_layout_snapshot(&layout);
    }
}

#[test]
fn style_letter_spacing_with_ligatures() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::LIGATURES;

    // Without letter spacing - ligatures may form
    let layout_no_spacing = build_with_letter_spacing(&mut env, text, 0.0);
    env.with_name("no_spacing")
        .check_layout_snapshot(&layout_no_spacing);

    // With letter spacing - ligatures should break
    let layout_with_spacing = build_with_letter_spacing(&mut env, text, 2.0);
    env.with_name("with_spacing")
        .check_layout_snapshot(&layout_with_spacing);
}

// ============================================================================
// WordSpacing Tests
// ============================================================================

#[test]
fn style_word_spacing_positive() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::SPACED;

    for spacing in [2.0, 4.0, 8.0, 16.0] {
        let layout = build_with_word_spacing(&mut env, text, spacing);

        env.with_name(&format!("pos_{spacing}"))
            .check_layout_snapshot(&layout);
    }
}

#[test]
fn style_word_spacing_negative() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = samples::SPACED;

    for spacing in [-2.0, -4.0] {
        let layout = build_with_word_spacing(&mut env, text, spacing);

        let name = format!("neg_{}", (-spacing) as i32);
        env.with_name(&name).check_layout_snapshot(&layout);
    }
}

// ============================================================================
// Run Splitting Tests
// ============================================================================

#[test]
fn spacing_causes_style_run_breaks() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "foo bar";
    let mut builder = env.ranged_builder(text);
    builder.push(StyleProperty::WordSpacing(2.0), 3..text.len());
    builder.push(StyleProperty::LetterSpacing(1.5), 3..text.len());

    let layout = builder.build(text);
    assert_eq!(
        layout.data.runs.len(),
        2,
        "expected two runs after spacing property changes"
    );

    let first_run = &layout.data.runs[0];
    let second_run = &layout.data.runs[1];

    assert_eq!(&text[first_run.text_range.clone()], "foo");
    assert_eq!(first_run.word_spacing, 0.0);
    assert_eq!(first_run.letter_spacing, 0.0);

    assert_eq!(&text[second_run.text_range.clone()], " bar");
    assert_eq!(second_run.word_spacing, 2.0);
    assert_eq!(second_run.letter_spacing, 1.5);
}
