// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::testenv;

#[test]
fn editor_simple_move() {
    let mut env = testenv!();
    let mut editor = env.editor("Hi, all!\nNext");
    env.check_editor_snapshot(&editor);
    env.transact(&mut editor, |e| {
        e.move_right();
        e.move_right();
        e.move_right();
    });
    env.check_editor_snapshot(&editor);
    env.transact(&mut editor, |e| e.move_down());
    env.check_editor_snapshot(&editor);
    env.transact(&mut editor, |e| e.move_left());
    env.check_editor_snapshot(&editor);
    env.transact(&mut editor, |e| e.move_up());
    env.check_editor_snapshot(&editor);
}
