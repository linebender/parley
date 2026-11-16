// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::tests::utils::fonts::create_font_context;
use crate::{
    Alignment, AlignmentOptions, FontFamily, InlineBox, InlineBoxKind, Layout, LayoutContext,
    PositionedLayoutItem, StyleProperty, VerticalAlign,
};
use std::vec::Vec;

const JUSTIFY_LAST: AlignmentOptions = AlignmentOptions {
    align_when_overflowing: false,
    last_line_alignment: Some(Alignment::Justify),
};

const FONT_FAMILIES: &str = "Roboto, 'Noto Sans CJK SC', 'Noto Kufi Arabic'";

/// Build `text`, and justify it (including the last line) at twice its natural width.
fn layout(text: &str) -> Layout<()> {
    let mut fonts = create_font_context();
    let mut context = LayoutContext::new();
    let mut builder = context.ranged_builder(&mut fonts, text, 1., true);
    builder.push_default(FontFamily::from(FONT_FAMILIES));
    // Exercise glyph-run boundaries within shaped runs, including boundaries at spaces.
    if text.len() > 3 && text.is_char_boundary(3) {
        builder.push(StyleProperty::Underline(true), 0..3);
    }
    let mut layout = builder.build(text);
    justify(&mut layout);
    layout
}

fn justify(layout: &mut Layout<()>) {
    layout.break_all_lines(None);
    let natural_width = layout.data.lines[0].metrics.advance;
    layout.set_alignment(Alignment::Justify, JUSTIFY_LAST);
    layout.break_all_lines(Some((natural_width * 2.).max(100.)));
}

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}

/// How much of an opportunity a shaped cluster owns on one visual side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Share {
    None,
    Half,
    Whole,
}

/// The justification opportunities of line `index` of `layout`.
fn opportunities(layout: &Layout<()>, index: usize) -> Opportunities {
    let data = &layout.data;
    let line = &data.lines[index];
    let mut opportunities = Opportunities::default();
    opportunities.find(&data.shaped_text, line, &data.line_items[line.item_range]);
    opportunities
}

/// The owners of the line's opportunities, as the byte index of the owning shaped cluster's first
/// character, and its shares before and after it.
fn owners(layout: &Layout<()>, index: usize) -> Vec<(usize, Share, Share)> {
    let data = &layout.data;
    // The line's shaped clusters, in the order `Opportunities::shares` indexes them.
    let clusters: Vec<u32> = data.line_items[data.lines[index].item_range]
        .iter()
        .filter(|item| item.kind == LayoutItemKind::TextRun)
        .flat_map(|item| item.shaped_cluster_range)
        .collect();
    let share = |halves| match halves {
        0 => Share::None,
        1 => Share::Half,
        2 => Share::Whole,
        _ => unreachable!(),
    };
    let opportunities = opportunities(layout, index);
    assert_eq!(opportunities.shares.len(), clusters.len());
    let mut owners: Vec<_> = clusters
        .iter()
        .zip(&opportunities.shares)
        .filter(|(_, shares)| shares.0 != 0)
        .map(|(&cluster, shares)| {
            let cluster = data.shaped_text.shaped_clusters()[cluster as usize];
            let character = data.shaped_text.characters()[cluster.chars_range().start as usize];
            (
                character.text_byte_start as usize,
                share(shares.before()),
                share(shares.after()),
            )
        })
        .collect();
    owners.sort_by_key(|owner| owner.0);
    owners
}

/// Justified lines with opportunities must fill the line, and others must not be stretched.
fn assert_fills_width(layout: &Layout<()>) {
    for index in 0..layout.len() {
        let line = layout.get(index).unwrap();
        let metrics = line.metrics();
        let width = metrics.inline_max_coord - metrics.inline_min_coord;
        let advance: f32 = line.runs().map(|run| run.advance()).sum();
        let halves = opportunities(layout, index).halves;
        if halves > 0 && width - metrics.indent - metrics.advance + metrics.hanging_advance > 0. {
            close(advance - metrics.hanging_advance, width);
            let right_edge = line
                .items()
                .map(|item| match item {
                    PositionedLayoutItem::GlyphRun(run) => run.offset() + run.advance(),
                    PositionedLayoutItem::InlineBox(inline_box) => inline_box.x + inline_box.width,
                })
                .reduce(f32::max)
                .unwrap();
            if !layout.data.base_level.is_rtl() {
                close(
                    right_edge - metrics.hanging_advance,
                    metrics.inline_max_coord,
                );
            }
        } else {
            close(advance, metrics.advance);
        }
    }
}

#[test]
fn cj_boundary_ownership() {
    use Share::{Half as H, None as N, Whole as W};
    // Expected byte-indexed owners, derived from visual boundaries.
    type ExpectedOwner = (usize, Share, Share);
    let cases: &[(&str, &[ExpectedOwner])] = &[
        ("one two", &[(3, N, W)]),
        ("漢字", &[(0, N, H), (3, H, N)]),
        ("漢 字", &[(3, N, W)]),
        ("a漢字b", &[(1, W, H), (4, H, W)]),
        ("漢字典", &[(0, N, H), (3, H, H), (6, H, N)]),
        ("漢　字", &[(0, N, H), (3, H, H), (6, H, N)]),
        ("a　b", &[(1, W, W)]),
        ("漢　", &[]),
        ("漢、。字", &[(0, N, H), (3, H, H), (6, H, H), (9, H, N)]),
        ("漢字 ", &[(0, N, H), (3, H, N)]),
        ("漢\u{a0}字", &[(3, N, W)]),
        ("漢字\u{a0}", &[(0, N, H), (3, H, N)]),
        ("漢\u{2060}字", &[(0, N, H), (6, H, N)]),
        ("\u{2060}", &[]),
        ("abc", &[]),
        ("漢", &[]),
        ("fi漢", &[(2, W, N)]),
    ];
    for &(text, expected) in cases {
        let layout = layout(text);
        assert_eq!(layout.len(), 1, "{text:?}");
        assert_eq!(owners(&layout, 0), expected, "{text:?}");
        assert_fills_width(&layout);
    }
}

#[test]
fn cj_wrapped_lines_fill_width() {
    let text = "漢字典漢字典 漢字典、。漢字典 abc 漢字典漢字典";
    let mut fonts = create_font_context();
    let mut context = LayoutContext::new();
    let mut builder = context.ranged_builder(&mut fonts, text, 1., true);
    builder.push_default(FontFamily::from(FONT_FAMILIES));
    let mut layout: Layout<()> = builder.build(text);
    layout.set_alignment(Alignment::Justify, JUSTIFY_LAST);
    layout.break_all_lines(Some(100.));
    assert!(layout.len() > 2);
    assert_fills_width(&layout);
}

#[test]
fn cj_mixed_direction_fills_width() {
    for text in ["漢مرحبا字", "مرحبا漢字", "漢字مرحبا", "مرحبا"] {
        let layout = layout(text);
        assert_fills_width(&layout);
        if opportunities(&layout, 0).halves == 0 {
            // Zero-opportunity RTL lines remain start aligned (visually right aligned).
            let metrics = &layout.data.lines[0].metrics;
            close(metrics.offset, metrics.inline_max_coord - metrics.advance);
        }
    }
}

#[test]
fn cj_inline_box_is_opaque_and_non_owning() {
    let text = "漢\u{fffc}字";
    let mut fonts = create_font_context();
    let mut context = LayoutContext::<()>::new();
    let mut builder = context.ranged_builder(&mut fonts, text, 1., true);
    builder.push_default(FontFamily::named("Noto Sans CJK SC"));
    builder.push_inline_box(InlineBox {
        id: 0,
        kind: InlineBoxKind::InFlow,
        index: 3,
        width: 20.,
        height: 10.,
        baseline: None,
        vertical_align: VerticalAlign::BASELINE,
    });
    let mut layout = builder.build(text);
    justify(&mut layout);
    use Share::{None as N, Whole as W};
    assert_eq!(owners(&layout, 0), [(0, N, W), (6, W, N)]);
    assert_fills_width(&layout);
}

/// Justified lines must fill the line across line breaks, hanging whitespace, and white space
/// handling.
#[test]
fn lines_fill_width_with_white_space_handling() {
    use crate::{TextWrapMode, WhiteSpaceCollapse};
    let texts = [
        "漢字典漢字典 漢字典、。漢字典 abc 漢字典漢字典",
        "one two  three\u{a0}four 漢\u{a0} five\u{3000}six\u{3000}\u{3000}seven",
        "漢\u{2060}字 \u{2060} 典\t漢 字\n\n典 字  \n漢字 ",
        "fi漢 ffi字\u{a0}\u{a0}典 a\u{3000}",
        " \u{3000} 漢 \u{a0}",
        "مرحبا بك  عالم \u{a0}مرحبا ",
    ];
    let collapses = [
        WhiteSpaceCollapse::Collapse,
        WhiteSpaceCollapse::Preserve,
        WhiteSpaceCollapse::PreserveBreaks,
        WhiteSpaceCollapse::BreakSpaces,
    ];
    let mut fonts = create_font_context();
    let mut context = LayoutContext::new();
    for text in texts {
        for collapse in collapses {
            for wrap in [TextWrapMode::Wrap, TextWrapMode::NoWrap] {
                let mut builder = context.ranged_builder(&mut fonts, text, 1., true);
                builder.push_default(FontFamily::from(FONT_FAMILIES));
                builder.push_default(StyleProperty::WhiteSpaceCollapse(collapse));
                builder.push_default(StyleProperty::TextWrapMode(wrap));
                let mut layout: Layout<()> = builder.build(text);
                layout.set_alignment(Alignment::Justify, JUSTIFY_LAST);
                for width in [10., 30., 45., 60., 75., 100., 150., 400.] {
                    layout.break_all_lines(Some(width));
                    assert_fills_width(&layout);
                }
            }
        }
    }
}
