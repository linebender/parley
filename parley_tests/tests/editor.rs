// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `PlainEditor` tests.

use crate::test_name;
use crate::util::TestEnv;
use parley::Affinity;

// TODO - Use CursorTest API for these tests

#[test]
fn editor_simple_move() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("Hi, all!\nNext");
    env.check_editor_snapshot(&mut editor);
    let mut drv = env.driver(&mut editor);
    drv.move_right();
    drv.move_right();
    drv.move_right();

    env.check_editor_snapshot(&mut editor);
    env.driver(&mut editor).move_down();
    env.check_editor_snapshot(&mut editor);
    env.driver(&mut editor).move_left();
    env.check_editor_snapshot(&mut editor);
    env.driver(&mut editor).move_up();
    env.check_editor_snapshot(&mut editor);
}

#[test]
fn editor_select_all() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("Hi, all!\nNext");
    env.driver(&mut editor).select_all();
    env.check_editor_snapshot(&mut editor);
}

#[test]
fn editor_select_hard_line() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("First\nNew Hard Line with soft break!\nLast");
    editor.set_width(Some(40.));
    env.driver(&mut editor).move_right();
    // We can select the first line.
    env.driver(&mut editor).select_to_hard_line_end();
    env.check_editor_snapshot(&mut editor);
    env.driver(&mut editor).move_to_hard_line_start();
    env.check_editor_snapshot(&mut editor);
    env.driver(&mut editor).move_down();
    env.driver(&mut editor).move_to_hard_line_end();
    env.check_editor_snapshot(&mut editor);
    env.driver(&mut editor).select_to_hard_line_start();
    env.check_editor_snapshot(&mut editor);
    env.driver(&mut editor).move_right();
    // Cursor is logically after the newline; there's not really any great answer here.
    env.driver(&mut editor).select_to_hard_line_start();
    env.check_editor_snapshot(&mut editor);

    // We can select the last line.
    env.driver(&mut editor).move_right();
    env.driver(&mut editor).move_right();
    env.driver(&mut editor).move_to_hard_line_end();
    env.check_editor_snapshot(&mut editor);
    env.driver(&mut editor).select_to_hard_line_start();
    env.check_editor_snapshot(&mut editor);
}

#[test]
fn editor_double_newline() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("Hi, all!\n\nNext");
    env.driver(&mut editor).select_all();
    env.check_editor_snapshot(&mut editor);
}

#[test]
fn editor_insert_line_endings_set_downstream_affinity() {
    let mut env = TestEnv::new(test_name!(), None);

    for (insert, expected_text) in [
        ("\n", "A\nB"),
        ("\r", "A\rB"),
        ("\u{2028}", "A\u{2028}B"),
        ("\u{2029}", "A\u{2029}B"),
        ("X\n", "AX\nB"),
        ("X\r", "AX\rB"),
        ("X\u{2028}", "AX\u{2028}B"),
        ("X\u{2029}", "AX\u{2029}B"),
    ] {
        let mut editor = env.editor("AB");
        env.driver(&mut editor).move_right(); // between A and B
        env.driver(&mut editor).insert_or_replace_selection(insert);

        assert_eq!(editor.raw_text(), expected_text);

        let sel = editor.raw_selection();
        assert!(sel.is_collapsed());
        assert_eq!(sel.focus().index(), expected_text.len() - 1);
        assert_eq!(sel.focus().affinity(), Affinity::Downstream);
    }
}

#[test]
fn editor_insert_regular_text_set_upstream_affinity() {
    let mut env = TestEnv::new(test_name!(), None);

    let mut editor = env.editor("AB");
    env.driver(&mut editor).move_right(); // between A and B
    env.driver(&mut editor).insert_or_replace_selection("X");

    assert_eq!(editor.raw_text(), "AXB");

    let sel = editor.raw_selection();
    assert!(sel.is_collapsed());
    assert_eq!(sel.focus().index(), 2);
    assert_eq!(sel.focus().affinity(), Affinity::Upstream);
}

/// Backspace must delete exactly one grapheme cluster — the unit a reader
/// perceives as a character — not one `char`.
///
/// The cases are deliberately not all emoji: `e` + a combining acute and a
/// Hangul syllable built from jamo are ordinary text that was equally affected,
/// which is why this is grapheme segmentation rather than an emoji special case.
#[test]
fn editor_backdelete_deletes_one_grapheme() {
    let mut env = TestEnv::new(test_name!(), None);
    for (name, text) in [
        // Family: man, ZWJ, woman, ZWJ, girl, ZWJ, boy — one grapheme of 7 chars.
        ("zwj family", "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}"),
        // Flag: two regional indicators.
        ("regional indicator flag", "\u{1F1EF}\u{1F1F5}"),
        // Heart plus the emoji presentation selector.
        ("emoji presentation selector", "\u{2764}\u{FE0F}"),
        // Waving hand plus a skin-tone modifier.
        ("skin tone modifier", "\u{1F44B}\u{1F3FD}"),
        // Not emoji: `e` plus a combining acute accent.
        ("combining mark", "e\u{301}"),
        // Not emoji: Hangul jamo composing one syllable.
        ("hangul jamo", "\u{1100}\u{1161}\u{11A8}"),
        // Not emoji: a CRLF pair is a single grapheme.
        ("crlf", "\r\n"),
    ] {
        let mut editor = env.editor(text);
        {
            let mut drv = env.driver(&mut editor);
            drv.move_to_text_end();
            drv.backdelete();
        }
        assert_eq!(editor.text().to_string(), "", "backdelete: {name}");
    }
}

/// Forward delete must likewise remove exactly one grapheme cluster, mirroring
/// [`editor_backdelete_deletes_one_grapheme`].
#[test]
fn editor_delete_deletes_one_grapheme() {
    let mut env = TestEnv::new(test_name!(), None);
    for (name, text) in [
        ("zwj family", "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}"),
        ("regional indicator flag", "\u{1F1EF}\u{1F1F5}"),
        ("emoji presentation selector", "\u{2764}\u{FE0F}"),
        ("skin tone modifier", "\u{1F44B}\u{1F3FD}"),
        ("combining mark", "e\u{301}"),
        ("hangul jamo", "\u{1100}\u{1161}\u{11A8}"),
        ("crlf", "\r\n"),
    ] {
        let mut editor = env.editor(text);
        {
            let mut drv = env.driver(&mut editor);
            drv.move_to_text_start();
            drv.delete();
        }
        assert_eq!(editor.text().to_string(), "", "delete: {name}");
    }
}

/// Deleting must still stop at each grapheme rather than swallowing the line:
/// two separate graphemes take two presses in either direction.
#[test]
fn editor_delete_stops_at_grapheme_boundaries() {
    let mut env = TestEnv::new(test_name!(), None);

    let mut editor = env.editor("ab");
    env.driver(&mut editor).move_to_text_end();
    env.driver(&mut editor).backdelete();
    assert_eq!(editor.text().to_string(), "a");

    let mut editor = env.editor("ab");
    env.driver(&mut editor).move_to_text_start();
    env.driver(&mut editor).delete();
    assert_eq!(editor.text().to_string(), "b");
}
