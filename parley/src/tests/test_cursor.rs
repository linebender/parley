// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::tests::utils::CursorTest;
use crate::{Cursor, FontContext, LayoutContext, Selection};

#[test]
fn cursor_previous_visual() {
    let (mut lcx, mut fcx) = (LayoutContext::new(), FontContext::new());
    let text = "Lorem ipsum dolor sit amet";
    let layout = CursorTest::single_line(text, &mut lcx, &mut fcx);

    let mut cursor: Cursor = layout.cursor_after("ipsum");
    layout.print_cursor(cursor);
    cursor = cursor.previous_visual(layout.layout());

    layout.assert_cursor_is_before("m dolor", cursor);
}

#[test]
fn cursor_next_visual() {
    let (mut lcx, mut fcx) = (LayoutContext::new(), FontContext::new());
    let text = "Lorem ipsum dolor sit amet";
    let layout = CursorTest::single_line(text, &mut lcx, &mut fcx);

    let mut cursor: Cursor = layout.cursor_before("dolor");
    layout.print_cursor(cursor);
    cursor = cursor.next_visual(layout.layout());

    layout.assert_cursor_is_after("ipsum d", cursor);
}

#[test]
fn cursor_ligature_selection() {
    use crate::tests::utils::ColorBrush;
    use crate::{Affinity, Cursor};
    let (mut lcx, mut fcx): (LayoutContext<ColorBrush>, _) =
        (LayoutContext::new(), FontContext::new());

    // Test with ligature text "fi" using serif font which should support ligatures
    let text = "fi";
    let mut builder = lcx.ranged_builder(&mut fcx, text, 1.0, true);
    builder.push_default(crate::style::GenericFamily::Serif);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    // Test cursor positioning at the end of the text (byte index 2)
    // This should position the cursor at the end, not at the start of the cluster
    let cursor_end = Cursor::from_byte_index(&layout, 2, Affinity::Upstream);

    let selection: Selection = cursor_end.into();

    let focus = selection.focus();

    let clusters = focus.logical_clusters(&layout);

    assert_eq!(clusters[0].as_ref().map(|c| c.text_range()), Some(1..2));
}
