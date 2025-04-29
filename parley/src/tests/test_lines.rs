// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{FontWeight, InlineBox, Layout, StyleProperty, testenv};

use super::utils::{ColorBrush, TestEnv};

fn build_layout<'a>(env: &'a mut TestEnv, font_size: f32, line_height: f32) -> Layout<ColorBrush> {
    let text = "Some text here. Let's make it a bit longer so that line wrapping kicks in 😊.\
		And also some اللغة العربية arabic text.\n\
		This is underline and strikethrough text and some extra more text\n\
		and some extra more text\n\
		and some extra more text\n\
		and some extra more text\n\
		and some extra more text\n\
		and some extra more text\n\
		and some extra more text";

    let max_advance = Some(200.0);

    let mut builder = env.ranged_builder(text);
    builder.push_default(StyleProperty::FontSize(font_size));
    builder.push_default(StyleProperty::LineHeight(line_height));

    let bold_style = StyleProperty::FontWeight(FontWeight::new(600.0));
    let underline_style = StyleProperty::Underline(true);
    let strikethrough_style = StyleProperty::Strikethrough(true);

    // Set the first 4 characters to bold
    builder.push(bold_style, 0..4);

    // Set the underline & strikethrough style
    builder.push(underline_style, 141..150);
    builder.push(strikethrough_style, 155..168);

    builder.push_inline_box(InlineBox {
        id: 0,
        index: 40,
        width: 50.0,
        height: 5.0,
    });
    builder.push_inline_box(InlineBox {
        id: 1,
        index: 50,
        width: 50.0,
        height: 3.0,
    });

    let mut layout = builder.build(text);
    layout.break_all_lines(max_advance);
    layout
}

#[test]
fn lots_of_lines() {
    let mut env = testenv!();

    let font_size = 16.0;
    let line_height = 1.3; // 20.8px
    let layout = build_layout(&mut env, font_size, line_height);

    env.with_name("20_8").check_layout_snapshot(&layout);

    let font_size = 16.0;
    let line_height = 1.58125; // 25.3px
    let layout = build_layout(&mut env, font_size, line_height);

    env.with_name("25_3").check_layout_snapshot(&layout);
}
