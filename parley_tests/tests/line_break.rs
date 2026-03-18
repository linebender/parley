// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Line breaking tests.
//!
//! These tests validate `break_next_with_length`, which breaks lines by character count
//! rather than advance width.

use crate::test_name;
use crate::util::TestEnv;
use parley::style::FontFamily;
use parley::{Alignment, AlignmentOptions, InlineBox, PositionedLayoutItem, StyleProperty};

#[test]
fn break_by_length_basic() {
    let mut env = TestEnv::new(test_name!(), None);

    // 6 characters "ABCDEF", break every 2 clusters -> 3 lines
    let text = "ABCDEF";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);

    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(2);
    breaker.break_next_with_length(2);
    breaker.break_next_with_length(2);
    breaker.finish();

    assert_eq!(layout.len(), 3, "Expected 3 lines");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_varying_lengths() {
    let mut env = TestEnv::new(test_name!(), None);

    // 10 characters "ABCDEFGHIJ", break at 3, 4, 3 clusters
    let text = "ABCDEFGHIJ";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);

    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(3); // ABC
    breaker.break_next_with_length(4); // DEFG
    breaker.break_next_with_length(3); // HIJ
    breaker.finish();

    assert_eq!(layout.len(), 3, "Expected 3 lines");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_with_spaces() {
    let mut env = TestEnv::new(test_name!(), None);

    // "AB CD" = 5 clusters (space counts)
    let text = "AB CD";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);

    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(3); // "AB " (including space)
    breaker.break_next_with_length(2); // "CD"
    breaker.finish();

    assert_eq!(layout.len(), 2, "Expected 2 lines");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_with_newline() {
    let mut env = TestEnv::new(test_name!(), None);

    // "AB\nCD" = 5 clusters, newline counts as 1 and does NOT cause automatic break
    let text = "AB\nCD";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);

    // Break at 4 clusters: should get "AB\nC" on first line, "D" on second
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(4);
    breaker.break_next_with_length(10); // remaining
    breaker.finish();

    assert_eq!(layout.len(), 2, "Expected 2 lines");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_with_inline_box() {
    let mut env = TestEnv::new(test_name!(), None);

    // "A[box]BC" where box counts as 1 cluster
    let text = "ABC";
    let mut builder = env.ranged_builder(text);
    builder.push_inline_box(InlineBox {
        id: 0,
        index: 1, // After 'A'
        width: 10.0,
        height: 10.0,
    });
    let mut layout = builder.build(text);

    // Break at 2 clusters: "A[box]" on first line, "BC" on second
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(2);
    breaker.break_next_with_length(10);
    breaker.finish();

    assert_eq!(layout.len(), 2, "Expected 2 lines");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_multiple_inline_boxes() {
    let mut env = TestEnv::new(test_name!(), None);

    // "[box][box][box]ABC" - 3 boxes + 3 chars = 6 clusters
    let text = "ABC";
    let mut builder = env.ranged_builder(text);
    for id in 0..3 {
        builder.push_inline_box(InlineBox {
            id,
            index: 0, // All at the start
            width: 10.0,
            height: 10.0,
        });
    }
    let mut layout = builder.build(text);

    // Break at 2 clusters each -> 3 lines
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(2);
    breaker.break_next_with_length(2);
    breaker.break_next_with_length(2);
    breaker.finish();

    assert_eq!(layout.len(), 3, "Expected 3 lines");
    env.check_layout_snapshot(&layout);
}

/// This test verifies that breaking in the middle of a ligature does NOT produce valid glyphs.
///
/// When "abfi" is broken after 3 characters, the "fi" ligature is split with "f" on line 1
/// and "i" on line 2. For proper rendering, the "i" cluster on line 2 should contain a
/// valid glyph. However, parley does not currently support re-shaping after layout, so
/// the ligature continuation cluster ("i") has no glyphs - they all belong to the ligature
/// start cluster ("f") which is on the previous line.
#[test]
#[should_panic(expected = "no item on line 2")]
fn break_by_length_with_ligature() {
    let mut env = TestEnv::new(test_name!(), None);

    // "abfi" has ligature "fi" which should count as 2 clusters (matching character count)
    let text = "abfi";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);

    // Break at 3 clusters: "abf" on first line, "i" on second
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(3);
    breaker.break_next_with_length(10);
    breaker.finish();

    assert_eq!(layout.len(), 2, "Expected 2 lines");

    // Get the second line and verify the "i" cluster has a valid glyph
    let line2 = layout.get(1).expect("Expected line 2 to exist");

    // Line 2 should have an item containing the "i" cluster
    let item = line2.items().next();
    let glyph_run = match item {
        Some(PositionedLayoutItem::GlyphRun(glyph_run)) => glyph_run,
        Some(PositionedLayoutItem::InlineBox(_)) => panic!("unexpected inline box"),
        None => panic!("no item on line 2"),
    };

    // The "i" cluster should have at least one glyph for proper rendering.
    // This assertion will fail because parley doesn't re-shape after breaking a ligature.
    let cluster = glyph_run.run().clusters().next();
    match cluster {
        Some(c) if c.glyphs().count() > 0 => {
            // Success - the ligature was properly broken and reshaped
        }
        _ => panic!("ligature was not properly broken"),
    }

    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_exact_fit() {
    let mut env = TestEnv::new(test_name!(), None);

    // "ABCD" = 4 clusters, break at exactly 4
    let text = "ABCD";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);

    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(4);
    breaker.finish();

    assert_eq!(layout.len(), 1, "Expected 1 line");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_single_cluster_lines() {
    let mut env = TestEnv::new(test_name!(), None);

    // "ABC" = 3 clusters, break every 1 cluster -> 3 lines
    let text = "ABC";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);

    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(1);
    breaker.break_next_with_length(1);
    breaker.break_next_with_length(1);
    breaker.finish();

    assert_eq!(layout.len(), 3, "Expected 3 lines");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_with_emoji() {
    let mut env = TestEnv::new(test_name!(), None);
    env.set_tolerance(5.0);

    let text = "✅👀🎉🤠✅👀";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontFamily(FontFamily::named(
        "Noto Color Emoji",
    )));
    let mut layout = builder.build(text);

    // Break at 1, 3, 2 clusters -> expect 3 lines
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(1);
    breaker.break_next_with_length(3);
    breaker.break_next_with_length(2);
    breaker.finish();

    assert_eq!(layout.len(), 3, "Expected 3 lines");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_with_emoji_only() {
    let mut env = TestEnv::new(test_name!(), None);
    env.set_tolerance(5.0);

    let text = "✅👀🎉🤠";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontFamily(FontFamily::named(
        "Noto Color Emoji",
    )));
    let mut layout = builder.build(text);

    // Break at 2 clusters each -> expect 2 lines
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(2);
    breaker.break_next_with_length(2);
    breaker.finish();

    assert_eq!(layout.len(), 2, "Expected 2 lines");
    env.check_layout_snapshot(&layout);
}

#[test]
fn break_by_length_with_multi_codepoint_emoji() {
    let mut env = TestEnv::new(test_name!(), None);

    // Family emoji (ZWJ sequence): 👨‍👩‍👧‍👦 = 7 codepoints
    // Flag emoji: 🇺🇸 = 2 codepoints
    // Simple emoji: ✅ = 1 codepoint (in the bundled subset)
    // Total = 10 clusters
    let text = "👨‍👩‍👧‍👦🇺🇸✅";
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontFamily(FontFamily::named(
        "Noto Color Emoji",
    )));
    let mut layout = builder.build(text);

    // Break at visual emoji boundaries: 7 (family), 2 (flag), 1 (simple)
    let mut breaker = layout.break_lines();
    breaker.break_next_with_length(7); // 👨‍👩‍👧‍👦
    breaker.break_next_with_length(2); // 🇺🇸
    breaker.break_next_with_length(1); // ✅
    breaker.finish();

    assert_eq!(layout.len(), 3, "Expected 3 lines");

    // Verify cluster counts per line match codepoint counts
    let clusters_per_line: Vec<usize> = layout
        .lines()
        .map(|line| line.runs().map(|run| run.clusters().count()).sum())
        .collect();
    assert_eq!(clusters_per_line, vec![7, 2, 1]);
}

/// This test verifies that `break_next_with_length` produces the same layout metrics as
/// `break_all_lines` when breaking at the same cluster positions.
///
/// The test applies letter spacing to text with newlines and compares:
/// 1. A layout broken with `break_all_lines(None)` (breaks on newlines only)
/// 2. A layout broken with `break_next_with_length` using cluster counts from (1)
#[test]
fn break_by_length_matches_max_advance_with_letter_spacing() {
    let mut env = TestEnv::new(test_name!(), None);

    // Text with newlines and multiple spaces as the user specified
    let text = "01 2  \n\n  34\n 5";

    // Create first layout with letter spacing, broken by max_advance (respects newlines)
    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::LetterSpacing(2.0));
    let mut layout_max_advance = builder.build(text);
    layout_max_advance.break_all_lines(None);
    layout_max_advance.align(None, Alignment::Start, AlignmentOptions::default());

    // Count clusters per line from the max_advance layout using public API
    let cluster_counts: Vec<u32> = layout_max_advance
        .lines()
        .map(|line| line.runs().map(|run| run.len() as u32).sum())
        .collect();

    // Create second layout with the same letter spacing, broken by length
    let mut builder2 = env.ranged_builder(text);
    builder2.push_default(StyleProperty::LetterSpacing(2.0));
    let mut layout_by_length = builder2.build(text);

    let mut breaker = layout_by_length.break_lines();
    for count in &cluster_counts {
        breaker.break_next_with_length(*count);
    }
    breaker.finish();
    layout_by_length.align(None, Alignment::Start, AlignmentOptions::default());

    // Compare the layouts - they should have the same line count
    assert_eq!(
        layout_max_advance.len(),
        layout_by_length.len(),
        "Line count mismatch: max_advance={}, by_length={}",
        layout_max_advance.len(),
        layout_by_length.len()
    );

    // Compare line-by-line metrics
    for (i, (line_a, line_b)) in layout_max_advance
        .lines()
        .zip(layout_by_length.lines())
        .enumerate()
    {
        assert_eq!(
            line_a.metrics().offset,
            line_b.metrics().offset,
            "Line {i} offset mismatch: max_advance={}, by_length={}",
            line_a.metrics().offset,
            line_b.metrics().offset
        );
        assert_eq!(
            line_a.metrics().advance,
            line_b.metrics().advance,
            "Line {i} advance mismatch: max_advance={}, by_length={}",
            line_a.metrics().advance,
            line_b.metrics().advance
        );
        assert_eq!(
            line_a.text_range(),
            line_b.text_range(),
            "Line {i} text_range mismatch"
        );
    }

    // Compare run cluster ranges between the two layouts
    for (i, (run_a, run_b)) in layout_max_advance
        .lines()
        .flat_map(|line| line.runs())
        .zip(layout_by_length.lines().flat_map(|line| line.runs()))
        .enumerate()
    {
        assert_eq!(
            run_a.cluster_range(),
            run_b.cluster_range(),
            "Run {i} cluster_range mismatch"
        );
    }

    // Compare overall layout dimensions
    assert_eq!(
        layout_max_advance.width(),
        layout_by_length.width(),
        "Layout width mismatch"
    );
    assert_eq!(
        layout_max_advance.height(),
        layout_by_length.height(),
        "Layout height mismatch"
    );

    // Check both snapshots
    env.with_name("max_advance")
        .check_layout_snapshot(&layout_max_advance);
    env.with_name("by_length")
        .check_layout_snapshot(&layout_by_length);
}

/// This test verifies that the last line of justified text is start-aligned when using
/// `break_next_with_length`, matching the behavior of `break_all_lines`.
#[test]
fn break_by_length_justified_last_line_start_aligned() {
    let mut env = TestEnv::new(test_name!(), None);

    let text = "AAA BBB CCC";

    // Create layout with break_all_lines and justify
    let builder = env.ranged_builder(text);
    let mut layout_max_advance = builder.build(text);
    layout_max_advance.break_all_lines(Some(60.0)); // Force wrapping
    layout_max_advance.align(Some(100.0), Alignment::Justify, AlignmentOptions::default());

    // Get cluster counts per line using public API
    let cluster_counts: Vec<u32> = layout_max_advance
        .lines()
        .map(|line| line.runs().map(|run| run.len() as u32).sum())
        .collect();

    // Create layout with break_next_with_length and justify
    let builder2 = env.ranged_builder(text);
    let mut layout_by_length = builder2.build(text);
    let mut breaker = layout_by_length.break_lines();
    for count in &cluster_counts {
        breaker.break_next_with_length(*count);
    }
    breaker.finish();
    layout_by_length.align(Some(100.0), Alignment::Justify, AlignmentOptions::default());

    // Verify both layouts have the same number of lines
    assert_eq!(layout_max_advance.len(), layout_by_length.len());

    // Verify the break reasons match (important for justification behavior)
    for (i, (line_a, line_b)) in layout_max_advance
        .lines()
        .zip(layout_by_length.lines())
        .enumerate()
    {
        assert_eq!(
            line_a.break_reason(),
            line_b.break_reason(),
            "Line {i} break_reason mismatch: max_advance={:?}, by_length={:?}",
            line_a.break_reason(),
            line_b.break_reason()
        );
    }

    // Check both snapshots
    env.with_name("max_advance")
        .check_layout_snapshot(&layout_max_advance);
    env.with_name("by_length")
        .check_layout_snapshot(&layout_by_length);
}
