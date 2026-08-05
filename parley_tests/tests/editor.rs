// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `PlainEditor` tests.

use core::num::NonZeroUsize;

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

#[test]
fn editor_defer_layout_batches_rebuilds() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("");
    editor.set_defer_layout(true);
    {
        let mut drv = env.driver(&mut editor);
        drv.insert_or_replace_selection("Hello");
        drv.insert_or_replace_selection(", ");
        drv.insert_or_replace_selection("world");
    }
    // No rebuild has happened yet; the provisional selection is already correct.
    assert!(editor.try_layout().is_none());
    assert_eq!(editor.raw_selection().focus().index(), 12);
    env.driver(&mut editor).refresh_layout();
    assert!(editor.try_layout().is_some());
    assert_eq!(editor.raw_text(), "Hello, world");
    assert!(editor.raw_selection().is_collapsed());
    assert_eq!(editor.raw_selection().focus().index(), 12);
}

#[test]
fn editor_defer_layout_boundary_deletes_use_fresh_layout() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("");
    editor.set_defer_layout(true);
    {
        let mut drv = env.driver(&mut editor);
        drv.insert_or_replace_selection("a😀");
        // The layout is dirty here; backdelete refreshes before reading clusters
        // and must delete the whole emoji cluster.
        drv.backdelete();
        drv.refresh_layout();
    }
    assert_eq!(editor.raw_text(), "a");
    assert_eq!(editor.raw_selection().focus().index(), 1);
}

#[test]
fn editor_defer_layout_matches_eager() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut eager = env.editor("Hi 😀 all");
    let mut deferred = env.editor("Hi 😀 all");
    deferred.set_defer_layout(true);

    for editor in [&mut eager, &mut deferred] {
        let mut drv = env.driver(editor);
        drv.move_to_text_end();
        drv.insert_or_replace_selection("!");
        drv.backdelete_word();
        drv.insert_or_replace_selection("folks");
        drv.move_left();
        drv.backdelete();
        drv.set_compose("ne", Some((2, 2)));
        drv.finish_compose();
        drv.refresh_layout();
    }

    assert_eq!(eager.raw_text(), deferred.raw_text());
    let (e, d) = (eager.raw_selection(), deferred.raw_selection());
    assert_eq!(e.focus().index(), d.focus().index());
    assert_eq!(e.focus().affinity(), d.focus().affinity());
    assert_eq!(e.anchor().index(), d.anchor().index());
}

#[test]
fn editor_defer_layout_clamps_ime_cursor_offsets() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut eager = env.editor("");
    let mut deferred = env.editor("");
    deferred.set_defer_layout(true);

    for editor in [&mut eager, &mut deferred] {
        let mut drv = env.driver(editor);
        // Mid-cluster IME cursor offsets must not leave a mid-cluster
        // provisional selection behind.
        drv.set_compose("😀", Some((2, 2)));
        drv.insert_or_replace_selection("x");
        drv.refresh_layout();
    }

    assert_eq!(eager.raw_text(), deferred.raw_text());
    assert_eq!(
        eager.raw_selection().focus().index(),
        deferred.raw_selection().focus().index()
    );
}

#[test]
fn editor_defer_layout_delete_bytes_matches_eager() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut eager = env.editor("hello world");
    let mut deferred = env.editor("hello world");
    deferred.set_defer_layout(true);

    for editor in [&mut eager, &mut deferred] {
        let mut drv = env.driver(editor);
        drv.move_to_text_end();
        drv.insert_or_replace_selection("!!");
        drv.move_left();
        drv.delete_bytes_before_selection(NonZeroUsize::new(1).unwrap());
        drv.delete_bytes_after_selection(NonZeroUsize::new(1).unwrap());
        drv.refresh_layout();
    }

    assert_eq!(eager.raw_text(), "hello world");
    assert_eq!(eager.raw_text(), deferred.raw_text());
    let (e, d) = (eager.raw_selection(), deferred.raw_selection());
    assert_eq!(e.focus().index(), d.focus().index());
    assert_eq!(e.anchor().index(), d.anchor().index());
}
