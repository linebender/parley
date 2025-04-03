// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::tests::utils::CursorTest;
use crate::{Cursor, FontContext, LayoutContext};

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

// Currently fails
#[ignore]
#[test]
fn cursor_rtl_newline_by_character() {
    let (mut lcx, mut fcx) = (LayoutContext::new(), FontContext::new());
    let text = "abcאבג\nde";
    let layout = CursorTest::single_line(text, &mut lcx, &mut fcx);

    let mut cursor: Cursor = layout.cursor_after("אבג");
    layout.print_cursor(cursor);
    cursor = cursor.previous_visual(layout.layout());

    layout.assert_cursor_is_after("abcא", cursor);
    cursor = cursor.previous_visual(layout.layout());
    cursor = cursor.previous_visual(layout.layout());
    layout.assert_cursor_is_after("abc", cursor);
}

// Currently goes into an infinite loop
#[ignore]
#[test]
fn cursor_rtl_newline_by_word() {
    let (mut lcx, mut fcx) = (LayoutContext::new(), FontContext::new());
    let text = "abcאבג\nde";
    let layout = CursorTest::single_line(text, &mut lcx, &mut fcx);

    let mut cursor: Cursor = layout.cursor_after("אבג");
    layout.print_cursor(cursor);
    cursor = cursor.previous_visual_word(layout.layout());
    layout.assert_cursor_is_after("abc", cursor);
}
