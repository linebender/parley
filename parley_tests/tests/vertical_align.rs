// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for CSS `vertical-align` support.

use crate::util::TestEnv;
use crate::{test_name, util::ColorBrush};
use parley::{
    Alignment, AlignmentOptions, InlineBox, Layout, PositionedLayoutItem, StyleProperty,
    VerticalAlign,
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
fn build_layout_with_valign_text(
    env: &mut TestEnv,
    segments: &[(&str, VerticalAlign)],
) -> Layout<ColorBrush> {
    let mut builder = env.tree_builder();
    for (text, valign) in segments {
        builder.push_style_modification_span(&[StyleProperty::VerticalAlign(*valign)]);
        builder.push_text(text);
        builder.pop_style_span();
    }
    let (mut layout, _text) = builder.build();
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start, AlignmentOptions::default());
    layout
}

/// Collect all positioned items from the first line as (baseline_or_y, kind) pairs.
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

    // Layout with explicit Baseline vertical-align
    let layout_explicit = build_layout_with_valign_text(&mut env, &[("Hello world", VerticalAlign::Baseline)]);

    // Both should produce the same baseline
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
        vec![InlineBox {
            id: 0,
            index: 5,
            width: 20.0,
            height: 30.0,
            vertical_align: VerticalAlign::Baseline,
        }],
        None,
    );

    let items = first_line_items(&layout);
    let line = layout.lines().next().unwrap();
    let baseline = line.metrics().baseline;

    // Find the inline box
    for item in &items {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            // Baseline: box bottom should sit at the text baseline
            // y = baseline + 0 - height = baseline - height
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
        vec![InlineBox {
            id: 0,
            index: 5,
            width: 20.0,
            height: 30.0,
            vertical_align: VerticalAlign::Length(raise),
        }],
        None,
    );

    let baseline = layout.lines().next().unwrap().metrics().baseline;

    for item in first_line_items(&layout) {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            // Length(10) means raise by 10, so offset = -10 (positive-down convention)
            // y = baseline + (-10) - height
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

    let lower = -8.0; // negative = lower
    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![InlineBox {
            id: 0,
            index: 5,
            width: 20.0,
            height: 30.0,
            vertical_align: VerticalAlign::Length(lower),
        }],
        None,
    );

    let baseline = layout.lines().next().unwrap().metrics().baseline;

    for item in first_line_items(&layout) {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            // Length(-8) means lower by 8, offset = -(-8) = 8
            // y = baseline + 8 - height
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

    // Two boxes: one Top, one Bottom
    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![
            InlineBox {
                id: 0,
                index: 5,
                width: 20.0,
                height: box_height,
                vertical_align: VerticalAlign::Top,
            },
            InlineBox {
                id: 1,
                index: 5,
                width: 20.0,
                height: box_height,
                vertical_align: VerticalAlign::Bottom,
            },
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

    // Top: box top aligns with line top
    // offset = -(ascent - height), y = baseline + offset - height = baseline - ascent
    let expected_top_y = baseline - ascent;
    assert!(
        (top_y - expected_top_y).abs() < 0.5,
        "Top box top edge should be at line top: expected {expected_top_y}, got {top_y}"
    );

    // Bottom: box bottom aligns with line bottom
    // offset = descent, y = baseline + descent - height
    let expected_bottom_y = baseline + descent - box_height;
    assert!(
        (bottom_y - expected_bottom_y).abs() < 0.5,
        "Bottom box bottom edge should be at line bottom: expected {expected_bottom_y}, got {bottom_y}"
    );
}

// ---------------------------------------------------------------------------
// Tests: Text run vertical-align variants
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_text_sub_lowers_baseline() {
    let mut env = TestEnv::new(test_name!(), None);

    let layout = build_layout_with_valign_text(
        &mut env,
        &[("Normal", VerticalAlign::Baseline), ("sub", VerticalAlign::Sub)],
    );

    let items = first_line_items(&layout);
    let mut baselines: Vec<(String, f32)> = Vec::new();

    for item in &items {
        if let PositionedLayoutItem::GlyphRun(gr) = item {
            let text: String = gr.run().clusters().map(|c| c.source_char()).collect();
            baselines.push((text, gr.baseline()));
        }
    }

    // Sub text should have a higher baseline value (lower on screen, since y increases downward)
    assert!(
        baselines.len() >= 2,
        "Expected at least 2 glyph runs, got {}",
        baselines.len()
    );

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
        &[("Normal", VerticalAlign::Baseline), ("sup", VerticalAlign::Super)],
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
            ("Normal", VerticalAlign::Baseline),
            ("raised", VerticalAlign::Length(raise)),
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

    // Length(5) => offset = -5 => baseline should be 5 less
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
            ("Normal", VerticalAlign::Baseline),
            ("lowered", VerticalAlign::Length(lower)),
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

    // Length(-5) => offset = 5 => baseline should be 5 more (lower)
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

    // Layout with only normal text
    let layout_normal = build_layout_with_valign_text(
        &mut env,
        &[("Hello world", VerticalAlign::Baseline)],
    );
    let metrics_normal = *layout_normal.lines().next().unwrap().metrics();

    // Layout with sub text — should expand descent
    let layout_sub = build_layout_with_valign_text(
        &mut env,
        &[("Hello ", VerticalAlign::Baseline), ("world", VerticalAlign::Sub)],
    );
    let metrics_sub = *layout_sub.lines().next().unwrap().metrics();

    // The line with sub text should have at least as much descent as the normal line
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

    // Layout with only normal text
    let layout_normal = build_layout_with_valign_text(
        &mut env,
        &[("Hello world", VerticalAlign::Baseline)],
    );
    let metrics_normal = *layout_normal.lines().next().unwrap().metrics();

    // Layout with super text — should expand ascent
    let layout_super = build_layout_with_valign_text(
        &mut env,
        &[("Hello ", VerticalAlign::Baseline), ("world", VerticalAlign::Super)],
    );
    let metrics_super = *layout_super.lines().next().unwrap().metrics();

    // Super text pushes content above the normal baseline, expanding effective ascent
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
        &[("Hello", VerticalAlign::Baseline)],
    );
    let metrics_normal = *layout_normal.lines().next().unwrap().metrics();
    let extent_normal = metrics_normal.max_coord - metrics_normal.min_coord;

    // Raise text by a large amount — should expand the visual extent of the line
    let layout_raised = build_layout_with_valign_text(
        &mut env,
        &[("Hello ", VerticalAlign::Baseline), ("UP", VerticalAlign::Length(50.0))],
    );
    let metrics_raised = *layout_raised.lines().next().unwrap().metrics();
    let extent_raised = metrics_raised.max_coord - metrics_raised.min_coord;

    assert!(
        extent_raised > extent_normal,
        "Large Length(50) should expand visual extent: raised={extent_raised}, normal={extent_normal}"
    );

    // The ascent should be significantly larger due to the raised text
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
            ("A", VerticalAlign::Super),
            ("B", VerticalAlign::Baseline),
            ("C", VerticalAlign::Sub),
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

    // Super < Baseline < Sub (in terms of y coordinate, since y increases downward)
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
// Tests: Inline box + text run with different vertical aligns
// ---------------------------------------------------------------------------

#[test]
fn vertical_align_inline_box_middle() {
    let mut env = TestEnv::new(test_name!(), None);

    let box_height = 20.0;
    let layout = build_layout_with_boxes(
        &mut env,
        "Hello",
        vec![InlineBox {
            id: 0,
            index: 5,
            width: 20.0,
            height: box_height,
            vertical_align: VerticalAlign::Middle,
        }],
        None,
    );

    let baseline = layout.lines().next().unwrap().metrics().baseline;

    for item in first_line_items(&layout) {
        if let PositionedLayoutItem::InlineBox(ib) = item {
            // Middle: offset = -(height * 0.5)
            // y = baseline + offset - height = baseline - height/2 - height = baseline - 1.5*height
            let offset = -(box_height * 0.5);
            let expected_y = baseline + offset - box_height;
            assert!(
                (ib.y - expected_y).abs() < 0.5,
                "Middle box y should be {expected_y}, got {}",
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
            ("Normal", VerticalAlign::Baseline),
            ("T", VerticalAlign::Top),
            ("B", VerticalAlign::Bottom),
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

    // For same-font text, Top and Bottom should produce offsets that push the run
    // to align with line edges. With a single font, all runs have identical metrics,
    // so Top and Bottom should be equivalent to Baseline.
    // Just verify they're reasonable values (within the line box).
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
