// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::testenv;

// TODO - Use CursorTest API for these tests

#[test]
fn editor_simple_move() {
    let mut env = testenv!();
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
    let mut env = testenv!();
    let mut editor = env.editor("Hi, all!\nNext");
    env.driver(&mut editor).select_all();
    env.check_editor_snapshot(&mut editor);
}

#[test]
fn editor_double_newline() {
    let mut env = testenv!();
    let mut editor = env.editor("Hi, all!\n\nNext");
    env.driver(&mut editor).select_all();
    env.check_editor_snapshot(&mut editor);
}
