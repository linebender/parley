// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `PlainEditor` tests.

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};
use parley::{Affinity, Cluster, StyleProperty};
use peniko::Color;

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
fn editor_style_overlay_applies_ranges() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("Hello world");
    let red = ColorBrush::new(Color::from_rgb8(255, 0, 0));
    editor.set_style_overlay(vec![
        (StyleProperty::Underline(true), 0..5),
        (StyleProperty::Brush(red), 6..11),
    ]);
    let mut drv = env.driver(&mut editor);
    let layout = drv.layout();
    let style_at = |index: usize| {
        Cluster::from_byte_index(layout, index)
            .unwrap()
            .style()
            .clone()
    };
    assert!(style_at(0).underline.is_some());
    assert!(style_at(4).underline.is_some());
    // The space at byte 5 is outside both ranges.
    assert!(style_at(5).underline.is_none());
    assert_ne!(style_at(5).brush, red);
    assert_eq!(style_at(6).brush, red);
    assert_eq!(style_at(10).brush, red);
}

#[test]
fn editor_style_overlay_invalidates_layout() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("Hello");
    env.driver(&mut editor).refresh_layout();
    assert!(editor.try_layout().is_some());

    let overlay = vec![(StyleProperty::<ColorBrush>::Underline(true), 0..5)];
    editor.set_style_overlay(overlay.clone());
    assert!(editor.try_layout().is_none());
    env.driver(&mut editor).refresh_layout();
    assert!(editor.try_layout().is_some());

    // Setting an equal overlay is a no-op.
    editor.set_style_overlay(overlay);
    assert!(editor.try_layout().is_some());

    // Clearing invalidates again.
    editor.set_style_overlay(Vec::new());
    assert!(editor.try_layout().is_none());
}

#[test]
fn editor_style_overlay_preedit_underline_wins() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("Hello");
    // The overlay tries to force underline off; the preedit push comes later and wins.
    editor.set_style_overlay(vec![(StyleProperty::<ColorBrush>::Underline(false), 0..5)]);
    env.driver(&mut editor).set_compose("XY", Some((0, 0)));
    let mut drv = env.driver(&mut editor);
    let layout = drv.layout();
    let style_at = |index: usize| {
        Cluster::from_byte_index(layout, index)
            .unwrap()
            .style()
            .clone()
    };
    // Preedit occupies bytes 0..2 and must keep its underline.
    assert!(style_at(0).underline.is_some());
    assert!(style_at(1).underline.is_some());
    assert!(style_at(3).underline.is_none());
}

#[test]
fn editor_style_overlay_survives_edits() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("abcd");
    editor.set_style_overlay(vec![(StyleProperty::<ColorBrush>::Underline(true), 0..1)]);
    {
        let mut drv = env.driver(&mut editor);
        drv.select_all();
        // The edit rebuilds with the retained overlay; "é" makes byte 1 a
        // non-boundary, so the stale entry is skipped instead of panicking.
        drv.insert_or_replace_selection("é");
    }
    assert_eq!(editor.raw_text(), "é");
    {
        let mut drv = env.driver(&mut editor);
        let layout = drv.layout();
        let style = Cluster::from_byte_index(layout, 0).unwrap().style().clone();
        assert!(style.underline.is_none());
    }
    // A fresh, aligned overlay applies again.
    editor.set_style_overlay(vec![(StyleProperty::<ColorBrush>::Underline(true), 0..2)]);
    let mut drv = env.driver(&mut editor);
    let layout = drv.layout();
    assert!(
        Cluster::from_byte_index(layout, 0)
            .unwrap()
            .style()
            .underline
            .is_some()
    );
}

#[test]
fn editor_style_overlay_clamps_out_of_bounds() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("");
    // Far out of bounds on empty text: clamped to empty, skipped, no panic.
    editor.set_style_overlay(vec![(StyleProperty::<ColorBrush>::Underline(true), 0..10)]);
    env.driver(&mut editor).refresh_layout();
    assert!(editor.try_layout().is_some());
    assert_eq!(editor.get_style_overlay().len(), 1);

    let mut editor = env.editor("abcd");
    editor.set_style_overlay(vec![(StyleProperty::<ColorBrush>::Underline(true), 2..100)]);
    let mut drv = env.driver(&mut editor);
    let layout = drv.layout();
    let style_at = |index: usize| {
        Cluster::from_byte_index(layout, index)
            .unwrap()
            .style()
            .clone()
    };
    assert!(style_at(1).underline.is_none());
    assert!(style_at(2).underline.is_some());
    assert!(style_at(3).underline.is_some());
}
