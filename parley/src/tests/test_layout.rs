// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::utils::{
    ColorBrush,
    fonts::{FONT_FAMILY_LIST, create_font_context},
};
use crate::{
    Alignment, AlignmentOptions, FontFamily, IndentOptions, Layout, LayoutContext, LineHeight,
    StyleProperty,
};

#[test]
fn clear_resets_to_new() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();

    static TEXT: &str = "Some text that will wrap across several lines.";

    let mut builder = lcx.ranged_builder(&mut fcx, TEXT, 1., false);
    builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
    let mut layout = builder.build(TEXT);
    layout.set_text_indent(10., IndentOptions::default());
    layout.break_all_lines(Some(50.));
    layout.align(Alignment::Center, AlignmentOptions::default());
    assert!(layout.lines().len() > 1);
    assert!(layout.width() > 0.);
    assert!(layout.height() > 0.);

    layout.clear();

    assert_eq!(layout.data, Layout::new().data);
}

/// A run's line height comes from its own style, so a style change that
/// alters it must start a new run: without that, one run covers both styles
/// and every line takes the line height of the style that run was shaped
/// under.
#[test]
fn line_height_is_per_run() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();

    static TEXT: &str = "tall\nshort";

    let mut builder = lcx.ranged_builder(&mut fcx, TEXT, 1., false);
    builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
    builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(40.)));
    builder.push(
        StyleProperty::LineHeight(LineHeight::Absolute(10.)),
        5..TEXT.len(),
    );
    let mut layout = builder.build(TEXT);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());

    let heights: alloc::vec::Vec<f32> = layout
        .lines()
        .map(|line| line.metrics().line_height)
        .collect();
    assert_eq!(heights, [40., 10.]);
    assert_eq!(layout.height(), 50.);
}
