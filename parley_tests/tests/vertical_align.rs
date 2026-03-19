// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for CSS `vertical-align` support (CSS Inline Level 3 decomposition).

use crate::util::TestEnv;
use crate::{test_name, util::ColorBrush};
use parley::{
    Alignment, AlignmentBaseline, AlignmentOptions, BaselineShift, BaselineSource, InlineBox,
    Layout, PositionedLayoutItem, StyleProperty,
};

/// Helper: build a single-line layout with the given text and inline boxes,
/// break and align it, and return the layout.
fn build_layout_with_boxes(
    env: &mut TestEnv,
    text: &str,
    boxes: Vec<InlineBox>,
    max_width: Option<f32>,
) -> Layout<ColorBrush> {
    let mut builder = env.ranged_builder(text);
    for b in boxes {
        builder.push_inline_box(b);
    }
    let mut layout = builder.build(text);
    layout.break_all_lines(max_width);
    layout.align(max_width, Alignment::Start, AlignmentOptions::default());
    layout
}

/// Helper: build a layout with mixed vertical-align text runs using the tree builder.
/// Each segment specifies alignment_baseline and baseline_shift independently.
fn build_layout_with_valign_text(
    env: &mut TestEnv,
    segments: &[(&str, AlignmentBaseline, BaselineShift)],
) -> Layout<ColorBrush> {
    let mut builder = env.tree_builder();
    for (text, ab, bs) in segments {
        builder.push_style_modification_span(&[
            StyleProperty::AlignmentBaseline(*ab),
            StyleProperty::BaselineShift(*bs),
        ]);
        builder.push_text(text);
        builder.pop_style_span();
    }
    let (mut layout, _text) = builder.build();
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    layout
}

/// Shorthand: default alignment (baseline, no shift).
const DEFAULT: (AlignmentBaseline, BaselineShift) =
    (AlignmentBaseline::Baseline, BaselineShift::None);

/// Collect all positioned items from the first line.
fn first_line_items(layout: &Layout<ColorBrush>) -> Vec<PositionedLayoutItem<'_, ColorBrush>> {
    layout
        .lines()
        .next()
        .unwrap()
        .items()
        .collect::<Vec<_>>()
}

/// Get the baseline of the first glyph run on the first line.
fn first_glyph_run_baseline(layout: &Layout<ColorBrush>) -> f32 {
    for item in first_line_items(layout) {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            return gr.baseline();
        }
    }
    panic!("no glyph run found");
}

/// Helper: create a default InlineBox with overrides for alignment.
fn make_box(
    id: u64,
    index: usize,
    width: f32,
    height: f32,
    alignment_baseline: AlignmentBaseline,
    baseline_shift: BaselineShift,
) -> InlineBox {
    InlineBox {
        id,
        index,
        width,
        height,
        alignment_baseline,
        baseline_shift,
        baseline_source: BaselineSource::default(),
        first_baseline: None,
    }
}

// ---------------------------------------------------------------------------
// Tests: Baseline (default) — should produce identical output to no vertical-align
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_baseline_is_default() {
    let mut env = TestEnv::new(test_name!(), None);

    // Layout with no vertical-align set
    let text = "Hello world";
    let builder_default = env.ranged_builder(text);
    let mut layout_default = builder_default.build(text);
    layout_default.break_all_lines(None);
    layout_default.align(None, Alignment::Start, AlignmentOptions::default());

    // Layout with explicit Baseline + no shift
    let layout_explicit = build_layout_with_valign_text(
        &mut env,
        &[("Hello world", DEFAULT.0, DEFAULT.1)],
    );

    let baseline_default = first_glyph_run_baseline(&layout_default);
    let baseline_explicit = first_glyph_run_baseline(&layout_explicit);
    assert!(
        (baseline_default - baseline_explicit).abs() < 0.01,
        "Baseline vertical-align should match default: {baseline_default} vs {baseline_explicit}"
    );
}

// ---------------------------------------------------------------------------
// Tests: InlineBox vertical-align variants
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_inline_box_baseline() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![make_box(0, 5, 20.0, 30.0, AlignmentBaseline::Baseline, BaselineShift::None)],
        None,
    );

    let items = first_line_items(&layout);
    let line = layout.lines().next().unwrap();
    let baseline = line.metrics().baseline;

    for item in &items {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            let expected_y = baseline - 30.0;
            assert!(
                (ib.y - expected_y).abs() < 0.01,
                "Baseline box y should be {expected_y}, got {}",
                ib.y
            );
        }
    }
}

#[test]
fn vertical_align_inline_box_length_raises() {
    let mut env = TestEnv::new(test_name!(), None);

    let raise = 10.0;
    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![make_box(0, 5, 20.0, 30.0, AlignmentBaseline::Baseline, BaselineShift::Length(raise))],
        None,
    );

    let baseline = layout.lines().next().unwrap().metrics().baseline;

    for item in first_line_items(&layout) {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            let expected_y = baseline - raise - 30.0;
            assert!(
                (ib.y - expected_y).abs() < 0.01,
                "Length({raise}) box y should be {expected_y}, got {}",
                ib.y
            );
        }
    }
}

#[test]
fn vertical_align_inline_box_length_lowers() {
    let mut env = TestEnv::new(test_name!(), None);

    let lower = -8.0;
    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![make_box(0, 5, 20.0, 30.0, AlignmentBaseline::Baseline, BaselineShift::Length(lower))],
        None,
    );

    let baseline = layout.lines().next().unwrap().metrics().baseline;

    for item in first_line_items(&layout) {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            let expected_y = baseline - lower - 30.0;
            assert!(
                (ib.y - expected_y).abs() < 0.01,
                "Length({lower}) box y should be {expected_y}, got {}",
                ib.y
            );
        }
    }
}

#[test]
fn vertical_align_inline_box_top_bottom() {
    let mut env = TestEnv::new(test_name!(), None);

    let box_height = 10.0;

    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![
            make_box(0, 5, 20.0, box_height, AlignmentBaseline::Baseline, BaselineShift::Top),
            make_box(1, 5, 20.0, box_height, AlignmentBaseline::Baseline, BaselineShift::Bottom),
        ],
        None,
    );

    let line = layout.lines().next().unwrap();
    let metrics = line.metrics();
    let baseline = metrics.baseline;
    let ascent = metrics.ascent;
    let descent = metrics.descent;

    let mut top_y = None;
    let mut bottom_y = None;

    for item in first_line_items(&layout) {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            match ib.id {
                0 => top_y = Some(ib.y),
                1 => bottom_y = Some(ib.y),
                _ => {}
            }
        }
    }

    let top_y = top_y.expect("Top box not found");
    let bottom_y = bottom_y.expect("Bottom box not found");

    let expected_top_y = baseline - ascent;
    assert!(
        (top_y - expected_top_y).abs() < 0.5,
        "Top box top edge should be at line top: expected {expected_top_y}, got {top_y}"
    );

    let expected_bottom_y = baseline + descent - box_height;
    assert!(
        (bottom_y - expected_bottom_y).abs() < 0.5,
        "Bottom box bottom edge should be at line bottom: expected {expected_bottom_y}, got {bottom_y}"
    );
}

// ---------------------------------------------------------------------------
// Tests: Text run baseline-shift variants
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_text_sub_lowers_baseline() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout = build_layout_with_valign_text(
        &mut env,
        &[
            ("Normal", DEFAULT.0, DEFAULT.1),
            ("sub", AlignmentBaseline::Baseline, BaselineShift::Sub),
        ],
    );

    let items = first_line_items(&layout);
    let mut baselines: Vec<(String, f32)> = Vec::new();

    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let text: String = gr.run().clusters().map(|c| c.source_char()).collect();
            baselines.push((text, gr.baseline()));
        }
    }

    assert!(baselines.len() >= 2, "Expected at least 2 glyph runs, got {}", baselines.len());

    let normal_baseline = baselines.iter().find(|(t, _)| t.starts_with('N')).unwrap().1;
    let sub_baseline = baselines.iter().find(|(t, _)| t.starts_with('s')).unwrap().1;

    assert!(
        sub_baseline > normal_baseline,
        "Sub text baseline ({sub_baseline}) should be lower (larger y) than normal ({normal_baseline})"
    );
}

#[test]
fn vertical_align_text_super_raises_baseline() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout = build_layout_with_valign_text(
        &mut env,
        &[
            ("Normal", DEFAULT.0, DEFAULT.1),
            ("sup", AlignmentBaseline::Baseline, BaselineShift::Super),
        ],
    );

    let items = first_line_items(&layout);
    let mut baselines: Vec<(String, f32)> = Vec::new();

    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let text: String = gr.run().clusters().map(|c| c.source_char()).collect();
            baselines.push((text, gr.baseline()));
        }
    }

    let normal_baseline = baselines.iter().find(|(t, _)| t.starts_with('N')).unwrap().1;
    let super_baseline = baselines.iter().find(|(t, _)| t.starts_with('s')).unwrap().1;

    assert!(
        super_baseline < normal_baseline,
        "Super text baseline ({super_baseline}) should be higher (smaller y) than normal ({normal_baseline})"
    );
}

#[test]
fn vertical_align_text_length_positive_raises() {
    let mut env = TestEnv::new(test_name!(), None);

    let raise = 5.0;
    let layout = build_layout_with_valign_text(
        &mut env,
        &[
            ("Normal", DEFAULT.0, DEFAULT.1),
            ("raised", AlignmentBaseline::Baseline, BaselineShift::Length(raise)),
        ],
    );

    let items = first_line_items(&layout);
    let mut baselines: Vec<(String, f32)> = Vec::new();

    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let text: String = gr.run().clusters().map(|c| c.source_char()).collect();
            baselines.push((text, gr.baseline()));
        }
    }

    let normal_baseline = baselines.iter().find(|(t, _)| t.starts_with('N')).unwrap().1;
    let raised_baseline = baselines.iter().find(|(t, _)| t.starts_with('r')).unwrap().1;

    let expected_diff = raise;
    let actual_diff = normal_baseline - raised_baseline;
    assert!(
        (actual_diff - expected_diff).abs() < 0.01,
        "Length({raise}) should raise baseline by {expected_diff}, actual diff: {actual_diff}"
    );
}

#[test]
fn vertical_align_text_length_negative_lowers() {
    let mut env = TestEnv::new(test_name!(), None);

    let lower = -5.0;
    let layout = build_layout_with_valign_text(
        &mut env,
        &[
            ("Normal", DEFAULT.0, DEFAULT.1),
            ("lowered", AlignmentBaseline::Baseline, BaselineShift::Length(lower)),
        ],
    );

    let items = first_line_items(&layout);
    let mut baselines: Vec<(String, f32)> = Vec::new();

    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let text: String = gr.run().clusters().map(|c| c.source_char()).collect();
            baselines.push((text, gr.baseline()));
        }
    }

    let normal_baseline = baselines.iter().find(|(t, _)| t.starts_with('N')).unwrap().1;
    let lowered_baseline = baselines.iter().find(|(t, _)| t.starts_with('l')).unwrap().1;

    assert!(
        lowered_baseline > normal_baseline,
        "Length({lower}) should lower baseline: lowered={lowered_baseline}, normal={normal_baseline}"
    );
    let actual_diff = lowered_baseline - normal_baseline;
    let expected_diff = -lower;
    assert!(
        (actual_diff - expected_diff).abs() < 0.01,
        "Expected diff {expected_diff}, got {actual_diff}"
    );
}

// ---------------------------------------------------------------------------
// Tests: Line metrics expansion
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_sub_expands_line_descent() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout_normal = build_layout_with_valign_text(
        &mut env,
        &[("Hello world", DEFAULT.0, DEFAULT.1)],
    );
    let metrics_normal = *layout_normal.lines().next().unwrap().metrics();

    let layout_sub = build_layout_with_valign_text(
        &mut env,
        &[
            ("Hello ", DEFAULT.0, DEFAULT.1),
            ("world", AlignmentBaseline::Baseline, BaselineShift::Sub),
        ],
    );
    let metrics_sub = *layout_sub.lines().next().unwrap().metrics();

    assert!(
        metrics_sub.descent >= metrics_normal.descent - 0.01,
        "Sub text should not reduce descent: sub={}, normal={}",
        metrics_sub.descent,
        metrics_normal.descent
    );
}

#[test]
fn vertical_align_super_expands_line_ascent() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout_normal = build_layout_with_valign_text(
        &mut env,
        &[("Hello world", DEFAULT.0, DEFAULT.1)],
    );
    let metrics_normal = *layout_normal.lines().next().unwrap().metrics();

    let layout_super = build_layout_with_valign_text(
        &mut env,
        &[
            ("Hello ", DEFAULT.0, DEFAULT.1),
            ("world", AlignmentBaseline::Baseline, BaselineShift::Super),
        ],
    );
    let metrics_super = *layout_super.lines().next().unwrap().metrics();

    assert!(
        metrics_super.ascent >= metrics_normal.ascent - 0.01,
        "Super text should not reduce ascent: super={}, normal={}",
        metrics_super.ascent,
        metrics_normal.ascent
    );
}

#[test]
fn vertical_align_large_length_expands_line_visual_extent() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout_normal = build_layout_with_valign_text(
        &mut env,
        &[("Hello", DEFAULT.0, DEFAULT.1)],
    );
    let metrics_normal = *layout_normal.lines().next().unwrap().metrics();
    let extent_normal = metrics_normal.max_coord - metrics_normal.min_coord;

    let layout_raised = build_layout_with_valign_text(
        &mut env,
        &[
            ("Hello ", DEFAULT.0, DEFAULT.1),
            ("UP", AlignmentBaseline::Baseline, BaselineShift::Length(50.0)),
        ],
    );
    let metrics_raised = *layout_raised.lines().next().unwrap().metrics();
    let extent_raised = metrics_raised.max_coord - metrics_raised.min_coord;

    assert!(
        extent_raised > extent_normal,
        "Large Length(50) should expand visual extent: raised={extent_raised}, normal={extent_normal}"
    );

    assert!(
        metrics_raised.ascent > metrics_normal.ascent + 10.0,
        "Raised text should increase ascent: raised={}, normal={}",
        metrics_raised.ascent,
        metrics_normal.ascent
    );
}

// ---------------------------------------------------------------------------
// Tests: Multiple vertical-align values on same line
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_mixed_runs_all_different() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout = build_layout_with_valign_text(
        &mut env,
        &[
            ("A", AlignmentBaseline::Baseline, BaselineShift::Super),
            ("B", DEFAULT.0, DEFAULT.1),
            ("C", AlignmentBaseline::Baseline, BaselineShift::Sub),
        ],
    );

    let items = first_line_items(&layout);
    let mut baselines: Vec<(char, f32)> = Vec::new();

    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let ch = gr.run().clusters().next().unwrap().source_char();
            baselines.push((ch, gr.baseline()));
        }
    }

    let a_baseline = baselines.iter().find(|(c, _)| *c == 'A').unwrap().1;
    let b_baseline = baselines.iter().find(|(c, _)| *c == 'B').unwrap().1;
    let c_baseline = baselines.iter().find(|(c, _)| *c == 'C').unwrap().1;

    assert!(
        a_baseline < b_baseline,
        "Super baseline ({a_baseline}) should be above Baseline ({b_baseline})"
    );
    assert!(
        b_baseline < c_baseline,
        "Baseline ({b_baseline}) should be above Sub ({c_baseline})"
    );
}

// ---------------------------------------------------------------------------
// Tests: Inline box alignment_baseline variants
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_inline_box_middle() {
    let mut env = TestEnv::new(test_name!(), None);

    let box_height = 20.0;
    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![make_box(0, 5, 20.0, box_height, AlignmentBaseline::Middle, BaselineShift::None)],
        None,
    );

    let line = layout.lines().next().unwrap();
    let baseline = line.metrics().baseline;

    // Get x-height from the first text run on the line.
    // CSS spec fallback: font_size * 0.5
    let x_height = line
        .runs()
        .next()
        .map(|r| {
            r.metrics()
                .x_height
                .unwrap_or(r.font_size() * 0.5)
        })
        .unwrap_or(0.0);

    for item in first_line_items(&layout) {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            // CSS middle: center the box at baseline - x_height/2
            // offset = (height - x_height) / 2
            // y = baseline + offset - height
            let offset = (box_height - x_height) / 2.0;
            let expected_y = baseline + offset - box_height;
            assert!(
                (ib.y - expected_y).abs() < 0.5,
                "Middle box y should be {expected_y}, got {} (x_height={x_height})",
                ib.y
            );
        }
    }
}

#[test]
fn vertical_align_text_top_bottom_align_with_line_edges() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout = build_layout_with_valign_text(
        &mut env,
        &[
            ("Normal", DEFAULT.0, DEFAULT.1),
            ("T", AlignmentBaseline::Baseline, BaselineShift::Top),
            ("B", AlignmentBaseline::Baseline, BaselineShift::Bottom),
        ],
    );

    let items = first_line_items(&layout);
    let metrics = *layout.lines().next().unwrap().metrics();
    let mut baselines: Vec<(char, f32)> = Vec::new();

    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let ch = gr.run().clusters().next().unwrap().source_char();
            baselines.push((ch, gr.baseline()));
        }
    }

    let normal_baseline = baselines.iter().find(|(c, _)| *c == 'N').unwrap().1;
    let top_baseline = baselines.iter().find(|(c, _)| *c == 'T').unwrap().1;
    let bottom_baseline = baselines.iter().find(|(c, _)| *c == 'B').unwrap().1;

    let line_top = metrics.baseline - metrics.ascent;
    let line_bottom = metrics.baseline + metrics.descent;

    assert!(
        top_baseline >= line_top && top_baseline <= line_bottom,
        "Top baseline {top_baseline} should be within line box [{line_top}, {line_bottom}]"
    );
    assert!(
        bottom_baseline >= line_top && bottom_baseline <= line_bottom,
        "Bottom baseline {bottom_baseline} should be within line box [{line_top}, {line_bottom}]"
    );

    // With same font, top/bottom/baseline should all be at the same position
    assert!(
        (top_baseline - normal_baseline).abs() < 0.5,
        "Same-font Top should match Baseline: {top_baseline} vs {normal_baseline}"
    );
    assert!(
        (bottom_baseline - normal_baseline).abs() < 0.5,
        "Same-font Bottom should match Baseline: {bottom_baseline} vs {normal_baseline}"
    );
}

// ---------------------------------------------------------------------------
// Tests: Combined alignment_baseline + baseline_shift (the key CSS3 feature)
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_combined_text_top_with_shift() {
    let mut env = TestEnv::new(test_name!(), None);

    // This is the case that was impossible with the single-enum model:
    // alignment-baseline: text-top; baseline-shift: 2px
    let layout = build_layout_with_valign_text(
        &mut env,
        &[
            ("Normal", DEFAULT.0, DEFAULT.1),
            ("shifted", AlignmentBaseline::TextTop, BaselineShift::Length(2.0)),
        ],
    );

    let items = first_line_items(&layout);
    let mut baselines: Vec<(String, f32)> = Vec::new();

    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let text: String = gr.run().clusters().map(|c| c.source_char()).collect();
            baselines.push((text, gr.baseline()));
        }
    }

    // With same font, TextTop aligns the run top with the line top (offset = 0 for same font).
    // Then Length(2.0) raises it by 2 additional units.
    // So the shifted run's baseline should be 2 units higher than the TextTop-only position.
    let normal_baseline = baselines.iter().find(|(t, _)| t.starts_with('N')).unwrap().1;
    let shifted_baseline = baselines.iter().find(|(t, _)| t.starts_with('s')).unwrap().1;

    // With same font, text-top alone = baseline, so combined = baseline - 2.0
    let expected_diff = 2.0;
    let actual_diff = normal_baseline - shifted_baseline;
    assert!(
        (actual_diff - expected_diff).abs() < 0.5,
        "TextTop + Length(2) should raise by ~{expected_diff}: actual diff {actual_diff}"
    );
}

// ---------------------------------------------------------------------------
// Tests: first_baseline on InlineBox
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_inline_box_first_baseline() {
    let mut env = TestEnv::new(test_name!(), None);

    let box_height = 30.0;
    let first_baseline_val = 10.0; // baseline is 10px from the top of the box

    // Box with first_baseline set — its internal baseline should align with the line baseline
    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![InlineBox {
            id: 0,
            index: 5,
            width: 20.0,
            height: box_height,
            alignment_baseline: AlignmentBaseline::Baseline,
            baseline_shift: BaselineShift::None,
            baseline_source: BaselineSource::default(),
            first_baseline: Some(first_baseline_val),
        }],
        None,
    );

    let line = layout.lines().next().unwrap();
    let baseline = line.metrics().baseline;

    for item in first_line_items(&layout) {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            // With first_baseline = 10, box top should be at baseline - 10
            // y = baseline + offset - height, where offset = -(height - first_baseline)
            let expected_y = baseline - first_baseline_val;
            assert!(
                (ib.y - expected_y).abs() < 0.5,
                "first_baseline box y should be {expected_y}, got {} (box top at baseline - {first_baseline_val})",
                ib.y
            );
        }
    }
}

#[test]
fn vertical_align_inline_box_first_baseline_none_matches_default() {
    let mut env = TestEnv::new(test_name!(), None);

    // Box without first_baseline (None) — bottom at baseline, same as original behavior
    let layout_none = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![make_box(0, 5, 20.0, 30.0, AlignmentBaseline::Baseline, BaselineShift::None)],
        None,
    );

    // Box with first_baseline = height — baseline at bottom, same as None
    let layout_full = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![InlineBox {
            id: 0,
            index: 5,
            width: 20.0,
            height: 30.0,
            alignment_baseline: AlignmentBaseline::Baseline,
            baseline_shift: BaselineShift::None,
            baseline_source: BaselineSource::default(),
            first_baseline: Some(30.0), // baseline at bottom = same as None
        }],
        None,
    );

    let y_none = first_line_items(&layout_none)
        .iter()
        .find_map(|item| {
            if let PositionedLayoutItem::InlineBox(ib) = item { Some(ib.y) } else { None }
        })
        .unwrap();

    let y_full = first_line_items(&layout_full)
        .iter()
        .find_map(|item| {
            if let PositionedLayoutItem::InlineBox(ib) = item { Some(ib.y) } else { None }
        })
        .unwrap();

    assert!(
        (y_none - y_full).abs() < 0.01,
        "first_baseline=height should match None: none={y_none}, full={y_full}"
    );
}

// ---------------------------------------------------------------------------
// Tests: Font-metric Sub/Super offsets
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_sub_super_use_font_metrics() {
    let mut env = TestEnv::new(test_name!(), None);

    // Build a layout with Sub and Super text to verify they produce different offsets
    // from each other and from the baseline. The exact values depend on the font's
    // OS/2 table, but we can verify the structural properties.
    let layout = build_layout_with_valign_text(
        &mut env,
        &[
            ("Normal", DEFAULT.0, DEFAULT.1),
            ("sub", AlignmentBaseline::Baseline, BaselineShift::Sub),
            ("sup", AlignmentBaseline::Baseline, BaselineShift::Super),
        ],
    );

    let items = first_line_items(&layout);
    let mut baselines: Vec<(String, f32)> = Vec::new();
    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let text: String = gr.run().clusters().map(|c| c.source_char()).collect();

            // Verify the run actually has font metrics available
            let metrics = gr.run().metrics();
            // subscript_offset and superscript_offset should be populated from the font
            // (Roboto has an OS/2 table, so these should be Some)
            if text.starts_with('s') {
                assert!(
                    metrics.subscript_offset.is_some() || metrics.superscript_offset.is_some(),
                    "Test font should provide OS/2 subscript/superscript metrics"
                );
            }

            baselines.push((text, gr.baseline()));
        }
    }

    let normal = baselines.iter().find(|(t, _)| t.starts_with('N')).unwrap().1;
    let sub = baselines.iter().find(|(t, _)| t == "sub").unwrap().1;
    let sup = baselines.iter().find(|(t, _)| t == "sup").unwrap().1;

    // Structural properties: super is above normal, sub is below normal
    assert!(sup < normal, "Super should be above normal: {sup} vs {normal}");
    assert!(sub > normal, "Sub should be below normal: {sub} vs {normal}");

    // The offsets should be non-trivial (at least 1px with 16px default font size)
    assert!(
        normal - sup > 1.0,
        "Super offset should be significant: {}", normal - sup
    );
    assert!(
        sub - normal > 1.0,
        "Sub offset should be significant: {}", sub - normal
    );
}
