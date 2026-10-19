// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use parley::{
    Affinity, BaseDirection, Cursor, InlineBox, InlineBoxKind, Layout, Selection, VerticalAlign,
};

use crate::test_name;
use crate::util::{ColorBrush, TestEnv};

fn inline_box(index: usize) -> InlineBox {
    InlineBox {
        id: 0,
        kind: InlineBoxKind::InFlow,
        index,
        width: 20.,
        height: 10.,
        baseline: None,
        vertical_align: VerticalAlign::BASELINE,
    }
}

#[test]
fn inline_object_can_be_resized_after_building() {
    let mut env = TestEnv::new(test_name!(), None);
    let text = "ab\u{FFFC}cd";
    let mut layout = |width| {
        let mut builder = env.ranged_builder(text);
        builder.push_inline_box(InlineBox {
            width,
            ..inline_box(2)
        });
        builder.build(text)
    };
    let widths = |mut layout: Layout<ColorBrush>| {
        layout.break_all_lines(None);
        (layout.calculate_content_widths().max, layout.width())
    };
    assert_ne!(widths(layout(20.)), widths(layout(50.)));
    // Editing a box gives the same widths as building with the edited box.
    let (mut edited, built) = (layout(20.), layout(50.));
    edited.edit_inline_box(0, |inline_box| inline_box.width = 50.);
    assert_eq!(widths(edited), widths(built));
}

#[test]
fn inline_object_in_bidi_text() {
    let mut env = TestEnv::new(test_name!(), None);
    for (case, direction, before, after) in [
        ("ltr", BaseDirection::Ltr, "ab سلم", " cd"),
        ("rtl", BaseDirection::Rtl, "سلم 12", " 34"),
    ] {
        let mut builder = env.tree_builder();
        builder.set_base_direction(direction);
        builder.push_text(before);
        builder.push_inline_box(inline_box(0));
        builder.push_text(after);
        let (mut layout, text) = builder.build();
        layout.break_all_lines(None);
        // Select the box and the text after it.
        let selection = Selection::new(
            Cursor::from_byte_index(&layout, before.len(), Affinity::Downstream),
            Cursor::from_byte_index(&layout, text.len(), Affinity::Downstream),
        );
        let geometry = selection.geometry(&layout);
        env.with_name(case)
            .render_and_check_snapshot(&layout, None, &geometry);
    }
}
