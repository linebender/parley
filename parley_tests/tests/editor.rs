// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `PlainEditor` tests.

use crate::test_name;
use crate::util::TestEnv;
use parley::Affinity;
use parley::editing::Composition;

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
fn editor_document_selection_accounts_for_compose_gap() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("abcd");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(2..2));
        assert!(drv.update_composition("XYZ", None, Some(3..3)));
    }

    assert_eq!(editor.raw_text(), "abXYZcd");
    assert_eq!(editor.text(), "abcd");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(2..4));
    }

    assert_eq!(editor.raw_selection().text_range(), 5..7);
}

#[test]
fn editor_insert_or_replace_uses_document_ranges_while_composing() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("abcd");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(2..2));
        assert!(drv.update_composition("XYZ", None, Some(3..3)));
        assert!(drv.insert_or_replace("Q", Some(2..4), None));
    }

    assert_eq!(editor.raw_text(), "abXYZQ");
    assert_eq!(editor.text(), "abQ");
    assert_eq!(
        editor.composition(),
        Some(Composition {
            text: "XYZ",
            document_offset: 2,
        })
    );
}

#[test]
fn editor_delete_surrounding_uses_document_space_while_composing() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("abcd");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(2..2));
        assert!(drv.update_composition("XYZ", None, Some(3..3)));
        assert!(drv.set_document_selection(4..4));
        assert!(drv.delete_surrounding(2, 0));
    }

    assert_eq!(editor.raw_text(), "abXYZ");
    assert_eq!(editor.text(), "ab");
    assert_eq!(
        editor.composition(),
        Some(Composition {
            text: "XYZ",
            document_offset: 2,
        })
    );
    assert_eq!(editor.raw_selection().text_range(), 5..5);
}

#[test]
fn editor_update_and_clear_composition_are_atomic() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hello");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(5..5));
        assert!(drv.update_composition("にほ", None, Some(3..3)));
    }

    assert_eq!(editor.raw_text(), "helloにほ");
    assert_eq!(editor.text(), "hello");
    assert_eq!(
        editor.composition(),
        Some(Composition {
            text: "にほ",
            document_offset: 5,
        })
    );
    assert_eq!(editor.raw_selection().text_range(), 8..8);

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.commit_composition());
    }

    assert_eq!(editor.raw_text(), "helloにほ");
    assert_eq!(editor.text(), "helloにほ");
    assert_eq!(editor.composition(), None);
    assert_eq!(editor.raw_selection().text_range(), 8..8);
}

#[test]
fn editor_visible_composing_region_preserves_document_text() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hello");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_composing_region(1..4));
    }

    assert_eq!(editor.raw_text(), "hello");
    assert_eq!(editor.text(), "hello");
    assert_eq!(
        editor.composition(),
        Some(Composition {
            text: "ell",
            document_offset: 1,
        })
    );

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.clear_composition());
    }

    assert_eq!(editor.raw_text(), "hello");
    assert_eq!(editor.text(), "hello");
    assert_eq!(editor.composition(), None);
}

#[test]
fn editor_visible_composing_region_commit_only_clears_mark() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hello");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_composing_region(1..4));
        assert!(drv.commit_composition());
    }

    assert_eq!(editor.raw_text(), "hello");
    assert_eq!(editor.text(), "hello");
    assert_eq!(editor.composition(), None);
}

#[test]
fn editor_collapsed_visible_composing_region_is_preserved() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hello");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_composing_region(2..2));
    }

    assert_eq!(editor.raw_text(), "hello");
    assert_eq!(editor.text(), "hello");
    assert_eq!(
        editor.composition(),
        Some(Composition {
            text: "",
            document_offset: 2,
        })
    );

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.clear_composition());
    }

    assert_eq!(editor.raw_text(), "hello");
    assert_eq!(editor.text(), "hello");
    assert_eq!(editor.composition(), None);
}

#[test]
fn editor_visible_composing_region_tracks_overlapping_replacements() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hello");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_composing_region(1..4));
        assert!(drv.insert_or_replace("X", Some(0..2), None));
    }

    assert_eq!(editor.raw_text(), "Xllo");
    assert_eq!(editor.text(), "Xllo");
    assert_eq!(
        editor.composition(),
        Some(Composition {
            text: "Xll",
            document_offset: 0,
        })
    );
}

#[test]
fn editor_visible_composing_region_end_boundary_is_half_open() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hello");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_composing_region(1..4));
        assert!(drv.insert_or_replace("X", Some(4..4), None));
    }

    assert_eq!(editor.raw_text(), "hellXo");
    assert_eq!(editor.text(), "hellXo");
    assert_eq!(
        editor.composition(),
        Some(Composition {
            text: "ell",
            document_offset: 1,
        })
    );
}

#[test]
fn editor_selected_text_is_available_for_visible_composition() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hello");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(1..4));
        assert!(drv.set_composing_region(1..4));
    }

    assert_eq!(editor.selected_text(), Some("ell"));
}

#[test]
fn editor_selected_text_is_hidden_for_preedit_composition() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("hello");

    {
        let mut drv = env.driver(&mut editor);
        assert!(drv.set_document_selection(1..4));
        assert!(drv.update_composition("XY", Some(1..4), Some(0..0)));
        assert!(drv.set_document_selection(0..2));
    }

    assert_eq!(editor.selected_text(), None);
}

#[test]
fn editor_delete_to_line_end_removes_to_physical_line_edge() {
    let mut env = TestEnv::new(test_name!(), None);
    let mut editor = env.editor("ab\ncd");

    {
        let mut drv = env.driver(&mut editor);
        drv.move_right();
        drv.delete_to_line_end();
    }

    assert_eq!(editor.raw_text(), "a\ncd");
}
