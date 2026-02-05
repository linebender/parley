// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `PlainEditor` tests.

use crate::test_name;
use crate::util::TestEnv;

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
