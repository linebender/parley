// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::vec::Vec;

use super::utils::{
    ColorBrush,
    fonts::{FONT_FAMILY_LIST, create_font_context},
};
use crate::{
    Alignment, AlignmentOptions, BreakReason, FontFamily, IndentOptions, Layout, LayoutContext,
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

/// Lays out a text with two paragraphs that each wrap, and returns for every line whether it is
/// the last line of its paragraph, its offset, its free space, and the advance of its first space.
fn last_line_alignment_lines(
    alignment: Alignment,
    last_line_alignment: Option<Alignment>,
) -> Vec<(bool, f32, f32, f32)> {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();

    static TEXT: &str = "aaa bbb ccc ddd eee f\nggg hhh iii jjj kkk l";
    const WIDTH: f32 = 100.;

    let mut builder = lcx.ranged_builder(&mut fcx, TEXT, 1., false);
    builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
    let mut layout = builder.build(TEXT);
    layout.break_all_lines(Some(WIDTH));
    layout.align(
        alignment,
        AlignmentOptions {
            last_line_alignment,
            ..AlignmentOptions::default()
        },
    );

    layout
        .lines()
        .map(|line| {
            let is_last = matches!(
                line.break_reason(),
                BreakReason::None | BreakReason::Explicit
            );
            let metrics = line.metrics();
            let free_space = WIDTH - (metrics.advance - metrics.hanging_advance);
            let space_advance = line
                .runs()
                .flat_map(|run| run.clusters())
                .find(|cluster| cluster.is_space_or_nbsp())
                .map(|cluster| cluster.advance())
                .unwrap();
            (is_last, metrics.offset, free_space, space_advance)
        })
        .collect()
}

#[test]
fn last_line_alignment_auto() {
    let unjustified_space = last_line_alignment_lines(Alignment::Start, None)[0].3;

    // The last lines follow the main alignment ...
    let lines = last_line_alignment_lines(Alignment::End, None);
    assert_eq!(lines.iter().filter(|line| line.0).count(), 2);
    assert!(lines.iter().any(|line| !line.0));
    for (_, offset, free_space, _) in lines {
        assert!(free_space > 0.);
        assert_eq!(offset, free_space);
    }

    // ... except when justifying, in which case they are start-aligned.
    for (is_last, offset, _, space_advance) in last_line_alignment_lines(Alignment::Justify, None) {
        assert_eq!(offset, 0.);
        if is_last {
            assert_eq!(space_advance, unjustified_space);
        } else {
            assert!(space_advance > unjustified_space);
        }
    }
}

#[test]
fn last_line_alignment_overrides_alignment() {
    let unjustified_space = last_line_alignment_lines(Alignment::Start, None)[0].3;

    for (is_last, offset, free_space, space_advance) in
        last_line_alignment_lines(Alignment::Justify, Some(Alignment::End))
    {
        if is_last {
            assert_eq!(offset, free_space);
            assert_eq!(space_advance, unjustified_space);
        } else {
            assert_eq!(offset, 0.);
            assert!(space_advance > unjustified_space);
        }
    }

    for (is_last, offset, free_space, space_advance) in
        last_line_alignment_lines(Alignment::Center, Some(Alignment::Justify))
    {
        if is_last {
            assert_eq!(offset, 0.);
            assert!(space_advance > unjustified_space);
        } else {
            assert_eq!(offset, free_space * 0.5);
            assert_eq!(space_advance, unjustified_space);
        }
    }

    for (_, offset, _, space_advance) in
        last_line_alignment_lines(Alignment::Justify, Some(Alignment::Justify))
    {
        assert_eq!(offset, 0.);
        assert!(space_advance > unjustified_space);
    }
}
