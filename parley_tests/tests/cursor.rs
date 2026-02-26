// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Cursor navigation tests.

// TODO: these should use the create_font_context helper to avoid introducing
// accidental dependencies on system fonts

use crate::{
    test_name,
    util::{CursorTest, TestEnv, env::create_font_context},
};
use parley::{Affinity, Cursor, LayoutContext, Selection};

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
