// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for runtime segmenter model loading.
//!
//! These tests verify that loading LSTM segmenter models at runtime improves
//! word/line breaking for languages like Thai, Lao, Khmer, and Burmese.

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{Alignment, AlignmentOptions, SegmenterModelData, StyleProperty};
use parley_data::bundled_models;
use peniko::color::palette::css;

/// "Hello, welcome to Thailand"
const THAI_TEXT: &str = "สวัสดีครับ ยินดีต้อนรับสู่ประเทศไทย";

fn load_thai_model() -> SegmenterModelData {
    SegmenterModelData::from_static(bundled_models::THAI_LSTM)
        .expect("Failed to load Thai LSTM model")
}

fn load_burmese_model() -> SegmenterModelData {
    SegmenterModelData::from_static(bundled_models::BURMESE_LSTM)
        .expect("Failed to load Burmese LSTM model")
}

/// Tests that Thai text line breaking improves with LSTM model.
#[test]
fn thai_line_break_without_model() {
    let mut env = TestEnv::new(test_name!(), None);

    let mut builder = env.ranged_builder(THAI_TEXT);
    builder.push_default(StyleProperty::Brush(ColorBrush::new(css::BLACK)));
    builder.push_default(StyleProperty::FontSize(24.0));

    let mut layout = builder.build(THAI_TEXT);
    // Use a narrow width to force line wrapping
    layout.break_all_lines(Some(150.0));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
}

/// Tests Thai text line breaking with LSTM model loaded.
#[test]
fn thai_line_break_with_model() {
    let mut env = TestEnv::new(test_name!(), None);

    // Load the Thai LSTM model
    env.layout_context_mut()
        .load_segmenter_models_auto([load_thai_model()]);

    let mut builder = env.ranged_builder(THAI_TEXT);
    builder.push_default(StyleProperty::Brush(ColorBrush::new(css::BLACK)));
    builder.push_default(StyleProperty::FontSize(24.0));

    let mut layout = builder.build(THAI_TEXT);
    // Use the same narrow width as the test without model
    layout.break_all_lines(Some(150.0));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
}

const MIXED_THAI_BURMESE: &str = "สวัสดีครับ\n(Hello in Thai)\nမင်္ဂလာပါ\n(Hello in Burm-ese)";

/// Tests that loading multiple LSTM models works correctly for mixed-script text.
#[test]
fn mixed_script_line_break_with_no_models() {
    let mut env = TestEnv::new(test_name!(), None);

    let mut builder = env.ranged_builder(MIXED_THAI_BURMESE);
    builder.push_default(StyleProperty::FontSize(20.0));

    let mut layout = builder.build(MIXED_THAI_BURMESE);
    // Use a narrow width to force line wrapping
    layout.break_all_lines(Some(70.0));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
}

/// Tests that loading multiple LSTM models works correctly for mixed-script text.
#[test]
fn mixed_script_line_break_with_multiple_models() {
    let mut env = TestEnv::new(test_name!(), None);

    // Load both Thai and Burmese LSTM models
    env.layout_context_mut()
        .load_segmenter_models_auto([load_thai_model(), load_burmese_model()]);

    let mut builder = env.ranged_builder(MIXED_THAI_BURMESE);
    builder.push_default(StyleProperty::FontSize(20.0));

    let mut layout = builder.build(MIXED_THAI_BURMESE);
    // Use a narrow width to force line wrapping
    layout.break_all_lines(Some(70.0));
    layout.align(None, Alignment::Start, AlignmentOptions::default());

    env.check_layout_snapshot(&layout);
}
