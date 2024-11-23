// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{testenv, Alignment, InlineBox};

#[test]
fn plain_multiline_text() {
    let mut env = testenv!();

    let text = "Hello world!\nLine 2\nLine 4";
    let mut builder = env.builder(text);
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    layout.align(None, Alignment::Start);

    env.check(&layout);
}

#[test]
fn placing_inboxes() {
    let mut env = testenv!();

    for position in [0, 3, 12, 13] {
        let text = "Hello world!\nLine 2\nLine 4";
        let mut builder = env.builder(text);
        builder.push_inline_box(InlineBox {
            id: 0,
            index: position,
            width: 10.0,
            height: 10.0,
        });
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start);
        env.check(&layout);
    }
}
