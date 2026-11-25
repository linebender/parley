// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Cursor navigation tests.

// TODO: these should use the create_font_context helper to avoid introducing
// accidental dependencies on system fonts

use core::{iter::successors, ops::Range};

use crate::{
    test_name,
    util::{ColorBrush, CursorTest, TestEnv, env::create_font_context},
};
use parley::{
    Affinity, BaseDirection, Cluster, ClusterPath, Cursor, LayoutContext, Selection, StyleProperty,
    WhiteSpaceCollapse,
};

#[test]
fn cursor_previous_visual() {
    let (mut lcx, mut fcx) = (LayoutContext::new(), create_font_context());
    let text = "Lorem ipsum dolor sit amet";
    let layout = CursorTest::single_line(text, &mut lcx, &mut fcx);

    let mut cursor: Cursor = layout.cursor_after("ipsum");
    layout.print_cursor(cursor);
    cursor = cursor.previous_visual(layout.layout());

    layout.assert_cursor_is_before("m dolor", cursor);
}

#[test]
fn cursor_next_visual() {
    let (mut lcx, mut fcx) = (LayoutContext::new(), create_font_context());
    let text = "Lorem ipsum dolor sit amet";
    let layout = CursorTest::single_line(text, &mut lcx, &mut fcx);

    let mut cursor: Cursor = layout.cursor_before("dolor");
    layout.print_cursor(cursor);
    cursor = cursor.next_visual(layout.layout());

    layout.assert_cursor_is_after("ipsum d", cursor);
}

#[test]
fn cursor_ligature_selection() {
    let mut env = TestEnv::new(test_name!(), None);
    // Test with ligature text "fi" using a font which has that ligature
    let text = "fi";
    let builder = env.ranged_builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    // Make sure there's actually only one glyph (the ligature)
    let line = layout.lines().next().unwrap();
    let run = line.runs().next().unwrap();
    let cluster = run.clusters().next().unwrap();
    let glyphs: Vec<_> = cluster.glyphs().collect();
    assert_eq!(glyphs.len(), 1);

    // Test cursor positioning at the end of the text (byte index 2)
    // This should position the cursor at the end, not at the start of the cluster
    let cursor_end = Cursor::from_byte_index(&layout, 2, Affinity::Upstream);

    let selection: Selection = cursor_end.into();

    let focus = selection.focus();

    let clusters = focus.logical_clusters(&layout);

    assert_eq!(clusters[0].as_ref().map(|c| c.text_range()), Some(1..2));
}

#[test]
fn cursor_geometry_on_shifted_line() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "abc";
    let mut layout = env.ranged_builder(text).build(text);
    let mut breaker = layout.break_lines();
    breaker.state_mut().set_line_x(13.);
    breaker.break_remaining(f32::MAX);

    let cursor = Cursor::from_byte_index(&layout, 0, Affinity::Downstream);
    assert_eq!(cursor.geometry(&layout, 0.).x0 as f32, 13.);
}

/// Check consistency of cluster paths, navigation, and lookups.
fn check_cluster_navigation(text: &str, max_advance: Option<f32>) {
    let mut env = TestEnv::new(test_name!(), None);
    let mut layout = env.ranged_builder(text).build(text);
    layout.break_all_lines(max_advance);

    fn key(cluster: &Cluster<'_, ColorBrush>) -> (ClusterPath, Range<usize>) {
        (cluster.path(), cluster.text_range())
    }

    /// Checks that stepping from the first of `expected` visits exactly `expected`, in order.
    fn walk<'a>(
        expected: &[Cluster<'a, ColorBrush>],
        step: impl FnMut(&Cluster<'a, ColorBrush>) -> Option<Cluster<'a, ColorBrush>>,
        name: &str,
    ) {
        let walked = successors(expected.first().cloned(), step);
        let walked: Vec<_> = walked.map(|cluster| key(&cluster)).collect();
        let expected: Vec<_> = expected.iter().map(key).collect();
        assert_eq!(walked, expected, "{name}");
    }

    // Every cluster in the layout, in logical and in visual order.
    let mut logical = Vec::new();
    let mut visual = Vec::new();
    for line in layout.lines() {
        let on_line: Vec<_> = line.runs().flat_map(|run| run.visual_clusters()).collect();
        for (i, cluster) in on_line.iter().enumerate() {
            assert_eq!(cluster.is_start_of_line(), i == 0, "line start");
            assert_eq!(cluster.is_end_of_line(), i + 1 == on_line.len(), "line end");
        }
        for run in line.runs() {
            let run_logical_clusters: Vec<_> = run.clusters().collect();
            for cluster in &run_logical_clusters {
                let expected = key(cluster);
                let by_path = cluster.path().cluster(&layout);
                assert_eq!(
                    by_path.as_ref().map(key).unwrap(),
                    expected,
                    "path round trip"
                );
                for byte in cluster.text_range() {
                    let by_byte = Cluster::from_byte_index(&layout, byte);
                    assert_eq!(by_byte.as_ref().map(key).unwrap(), expected, "byte {byte}");
                }
            }
            let run_visual_clusters: Vec<_> = run.visual_clusters().collect();
            logical.extend(run_logical_clusters);
            visual.extend(run_visual_clusters);
        }
    }
    logical.sort_by_key(|cluster| cluster.text_range().start);

    walk(&logical, Cluster::next_logical, "forward logical");
    walk(&visual, Cluster::next_visual, "forward visual");
    logical.reverse();
    visual.reverse();
    walk(&logical, Cluster::previous_logical, "backward logical");
    walk(&visual, Cluster::previous_visual, "backward visual");
}

#[test]
fn cluster_navigation_mixed_directions_and_ligatures() {
    check_cluster_navigation("Hello Ligature: fi, Arabic: حداً", Some(60.0));
}

#[test]
fn cluster_navigation_hard_breaks_and_trailing_newline() {
    check_cluster_navigation("AAA \nااااا\nfi\n", None);
}

#[test]
fn cluster_navigation_multi_character_graphemes() {
    check_cluster_navigation("e\u{0301}x 🇳🇱 a\r\nb", None);
}

#[test]
fn primary_caret_position() {
    let mut env = TestEnv::new(test_name!(), None);

    for text in ["aaa bbb", "aaa ببب"] {
        // These are some positions where the caret lands exactly on one edge of the line.
        //
        // Returns the start of the caret's line, and which edge of the line's content the caret is
        // at, if either.
        let mut position = |direction, index, wrap: bool| {
            let mut builder = env.ranged_builder(text);
            builder.set_base_direction(direction);
            let mut layout = builder.build(text);
            layout.break_all_lines(wrap.then(|| layout.calculate_content_widths().min));
            let (line, x) = layout.primary_caret(index).unwrap();
            let left = line.metrics().offset + line.metrics().inline_min_coord;
            let edges = [(left, "left"), (left + line.metrics().advance, "right")];
            let edge = edges.into_iter().find(|(edge, _)| (edge - x).abs() < 1e-3);
            (line.text_range().start, edge.map(|(_, side)| side))
        };
        assert_eq!(position(BaseDirection::Ltr, 0, false), (0, Some("left")));
        assert_eq!(
            position(BaseDirection::Ltr, text.len(), false),
            (0, Some("right"))
        );
        assert_eq!(position(BaseDirection::Rtl, 0, false), (0, Some("right")));
        assert_eq!(
            position(BaseDirection::Rtl, text.len(), false),
            (0, Some("left"))
        );
        assert_eq!(position(BaseDirection::Ltr, 4, true), (4, Some("left")));
    }

    // In an LTR paragraph, between an RTL word and a space, the caret sits against the space.
    let text = "ببب aaa";
    let mut builder = env.ranged_builder(text);
    builder.set_base_direction(BaseDirection::Ltr);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let space = Cluster::from_byte_index(&layout, 6).unwrap();
    assert_eq!(
        layout.primary_caret(6).unwrap().1,
        space.visual_offset().unwrap()
    );
}

fn collapsed_cursor_test(
    env: &mut TestEnv,
    text: &str,
    direction: BaseDirection,
    wrap_after: Option<&str>,
) -> CursorTest {
    let max_advance = wrap_after.map(|prefix| {
        let mut measure = env.ranged_builder(prefix).build(prefix);
        measure.break_all_lines(None);
        measure.full_width() + 0.5
    });
    let mut builder = env.ranged_builder(text);
    builder.set_base_direction(direction);
    builder.push_default(StyleProperty::WhiteSpaceCollapse(
        WhiteSpaceCollapse::Collapse,
    ));
    let mut layout = builder.build(text);
    layout.break_all_lines(max_advance);
    CursorTest::from_layout(text, layout)
}

#[test]
fn cursor_skips_removed_space_at_line_ends() {
    let mut env = TestEnv::new(test_name!(), None);

    // Soft wrap: removed space at end of line 0 is not a separate caret stop.
    let test = collapsed_cursor_test(&mut env, "one two", BaseDirection::Ltr, Some("one"));
    let before_space = test.cursor_after("one");
    let line1_start = before_space.next_visual(test.layout());
    test.assert_cursor_is_before("two", line1_start);

    let line0_end = line1_start.previous_visual(test.layout());
    // For all intents and purposes, this is the same place as after `one` (so long as you're not editing,
    // which you probably shouldn't be with WhitespaceCollapse set!)
    test.assert_cursor_is_after("one ", line0_end);
    assert_eq!(
        line0_end.geometry(test.layout(), 1.),
        before_space.geometry(test.layout(), 1.)
    );
    test.assert_cursor_is_before("two", line0_end.next_visual(test.layout()));
    test.assert_cursor_is_before("e two", line0_end.previous_visual(test.layout()));

    // Hard break: removed space before '\n' is skipped in both directions.
    let test = collapsed_cursor_test(&mut env, "one \ntwo", BaseDirection::Ltr, None);
    let line1_start = test.cursor_after("one").next_visual(test.layout());
    test.assert_cursor_is_after("one \n", line1_start);
    let before_newline = line1_start.previous_visual(test.layout());
    test.assert_cursor_is_before("\ntwo", before_newline);
    test.assert_cursor_is_before("e \ntwo", before_newline.previous_visual(test.layout()));

    // Layout end: stepping left from EOF skips the removed space and crosses 'e', while stepping
    // right from before the removed space stays put rather than silently crossing it.
    let test = collapsed_cursor_test(&mut env, "one ", BaseDirection::Ltr, None);
    let eof = test.cursor_after("one ");
    test.assert_cursor_is_before("e ", eof.previous_visual(test.layout()));
    let before_space = test.cursor_after("one");
    test.assert_cursor_is(before_space, before_space.next_visual(test.layout()));
}

#[test]
fn cursor_keeps_zero_width_space_before_removed_space() {
    let mut env = TestEnv::new(test_name!(), None);

    // The ZWSP isn't removed so it keeps its caret stop in both directions, while the removed
    // space immediately after it is skipped.
    let test = collapsed_cursor_test(&mut env, "one\u{200B} two", BaseDirection::Ltr, Some("one"));
    let after_zwsp = test.cursor_after("one").next_visual(test.layout());
    test.assert_cursor_is_after("one\u{200B}", after_zwsp);
    test.assert_cursor_is_before("two", after_zwsp.next_visual(test.layout()));

    let line0_end = test.cursor_before("two").previous_visual(test.layout());
    test.assert_cursor_is_after("one\u{200B} ", line0_end);
    let before_zwsp = line0_end.previous_visual(test.layout());
    test.assert_cursor_is_before("\u{200B}", before_zwsp);
    test.assert_cursor_is_before("e\u{200B}", before_zwsp.previous_visual(test.layout()));
}

#[test]
fn cursor_skips_removed_space_rtl() {
    let mut env = TestEnv::new(test_name!(), None);

    // RTL paragraph with an LTR word and a removed RTL space at a soft wrap.
    // Visual line 0: [removed ' '][a][b] — the removed space is at the visual start of the layout.
    let test = collapsed_cursor_test(&mut env, "ab גד", BaseDirection::Rtl, Some("ab"));
    for at_removed in [test.cursor_before(" גד"), test.cursor_after("ab ")] {
        test.assert_cursor_is(at_removed, at_removed.previous_visual(test.layout()));
        test.assert_cursor_is_after("a", at_removed.next_visual(test.layout()));
    }
}
