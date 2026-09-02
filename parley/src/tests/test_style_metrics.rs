// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tests for per-style inline box metrics.

use super::test_builders::{FONT_FAMILY_LIST, create_font_context};
use super::utils::ColorBrush;
use crate::layout::style_metrics::StyleMetrics;
use crate::{
    FontFamily, Layout, LayoutContext, LineHeight, StyleProperty, TextStyle, VerticalAlign,
};

fn root_style() -> TextStyle<'static, 'static, ColorBrush> {
    TextStyle {
        font_family: FontFamily::from(FONT_FAMILY_LIST),
        font_size: 20.,
        line_height: LineHeight::Absolute(30.),
        ..TextStyle::default()
    }
}

/// Root style with three spans: `A` (40px) containing `B` (10px, `vertical-align: super`) and a
/// sibling `top` aligned span `C` containing `D`.
fn build() -> Layout<ColorBrush> {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = root_style();
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    builder.push_text("root ");
    builder.push_style_span(TextStyle {
        font_size: 40.,
        line_height: LineHeight::Absolute(50.),
        ..root_style()
    });
    builder.push_style_span(TextStyle {
        font_size: 10.,
        line_height: LineHeight::Absolute(20.),
        vertical_align: VerticalAlign::SUPER,
        ..root_style()
    });
    builder.push_text("B");
    builder.pop_style_span();
    builder.pop_style_span();
    builder.push_style_modification_span(&[StyleProperty::VerticalAlign(VerticalAlign::TOP)]);
    builder
        .push_style_modification_span(&[StyleProperty::VerticalAlign(VerticalAlign::length(3.))]);
    builder.push_text("D");
    builder.pop_style_span();
    builder.pop_style_span();
    let (layout, _) = builder.build();
    layout
}

fn metrics(layout: &Layout<ColorBrush>) -> &[StyleMetrics] {
    &layout.data.style_metrics
}

#[test]
fn one_metrics_entry_per_style() {
    let layout = build();
    assert_eq!(layout.data.styles.len(), 5);
    assert_eq!(metrics(&layout).len(), 5);
}

#[test]
fn line_height_is_distributed_as_half_leading() {
    let layout = build();
    let root = metrics(&layout)[0];
    assert_eq!(root.line_height, 30.);
    assert!((root.over + root.under - 30.).abs() < 1e-4);
    assert!(root.over > root.ascent && root.under > root.descent);
    // Roboto at 20px.
    assert!(root.ascent > 15. && root.ascent < 20.);
    assert!(root.x_height > 9. && root.x_height < 12.);
    assert_eq!(root.baseline_offset, 0.);
    assert_eq!(root.aligned_subtree, 0);
}

#[test]
fn ancestors_without_text_have_metrics() {
    let layout = build();
    // Span A has no direct text but still has a style entry with its own 40px metrics.
    let a = metrics(&layout)[1];
    assert_eq!(a.line_height, 50.);
    assert!(a.ascent > 30.);
    assert_eq!(a.baseline_offset, 0.);
    assert_eq!(a.aligned_subtree, 0);
}

#[test]
fn super_shifts_relative_to_parent() {
    let layout = build();
    let b = metrics(&layout)[2];
    assert_eq!(layout.data.styles[2].parent, 1);
    // WebKit/Blink constant: a third of the *parent's* font size (40px).
    assert!((b.baseline_offset - 40. / 3.).abs() < 1e-4);
    assert_eq!(b.aligned_subtree, 0);
}

#[test]
fn top_starts_an_aligned_subtree() {
    let layout = build();
    let c = metrics(&layout)[3];
    let d = metrics(&layout)[4];
    assert_eq!(c.baseline_offset, 0.);
    assert_eq!(c.aligned_subtree, 3);
    // Children of a `top` box are relative to it, not to the root.
    assert_eq!(d.baseline_offset, 3.);
    assert_eq!(d.aligned_subtree, 3);
}

#[test]
fn text_top_middle_and_text_bottom() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = root_style();
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    for align in [
        VerticalAlign::TEXT_TOP,
        VerticalAlign::MIDDLE,
        VerticalAlign::TEXT_BOTTOM,
    ] {
        builder.push_style_span(TextStyle {
            font_size: 10.,
            line_height: LineHeight::Absolute(12.),
            vertical_align: align,
            ..root_style()
        });
        builder.push_text("x");
        builder.pop_style_span();
    }
    let (layout, _) = builder.build();
    let m = metrics(&layout);
    let root = m[0];
    let (text_top, middle, text_bottom) = (m[1], m[2], m[3]);
    // The box top sits at the parent's content ascent.
    assert!((text_top.baseline_offset + text_top.over - root.ascent).abs() < 1e-4);
    // The box midpoint sits at half the parent's x-height.
    let mid = middle.baseline_offset + (middle.over - middle.under) / 2.;
    assert!((mid - root.x_height / 2.).abs() < 1e-4);
    // The box bottom sits at the parent's content descent.
    assert!((text_bottom.baseline_offset - text_bottom.under + root.descent).abs() < 1e-4);
}

#[test]
fn quantized_metrics_are_whole_pixels() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = TextStyle {
        line_height: LineHeight::MetricsRelative(1.),
        ..root_style()
    };
    let mut builder = lcx.tree_builder(&mut fcx, 1., true, &root);
    builder.push_text("x");
    let (layout, _) = builder.build();
    let root = metrics(&layout)[0];
    assert_eq!(root.ascent.fract(), 0.);
    assert_eq!(root.descent.fract(), 0.);
    assert_eq!(root.over.fract(), 0.);
}

#[test]
fn first_available_font_skips_faces_without_a_space_glyph() {
    // The Noto Color Emoji subset has no U+0020 glyph, so per CSS Fonts' "first available font"
    // rule the inline box metrics come from Roboto (the next family) instead.
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let emoji_first = [
        parlance::FontFamilyName::Named("Noto Color Emoji".into()),
        parlance::FontFamilyName::Named("Roboto".into()),
    ];
    let roboto = [parlance::FontFamilyName::Named("Roboto".into())];
    let root = TextStyle {
        font_family: FontFamily::from(&emoji_first[..]),
        font_size: 20.,
        line_height: LineHeight::MetricsRelative(1.),
        ..TextStyle::default()
    };
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    builder.push_text("x");
    let (layout, _) = builder.build();
    let with_emoji_first = metrics(&layout)[0];

    let mut builder = lcx.tree_builder(
        &mut fcx,
        1.,
        false,
        &TextStyle {
            font_family: FontFamily::from(&roboto[..]),
            ..root
        },
    );
    builder.push_text("x");
    let (layout, _) = builder.build();
    let roboto_only = metrics(&layout)[0];

    // Both fonts share ascent/descent ratios; the x-height tells them apart.
    assert!((roboto_only.x_height - 10.57).abs() < 0.01);
    assert_eq!(with_emoji_first.x_height, roboto_only.x_height);
    assert_eq!(with_emoji_first.ascent, roboto_only.ascent);
    assert_eq!(with_emoji_first.descent, roboto_only.descent);
    assert_eq!(with_emoji_first.line_height, roboto_only.line_height);
}
