// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Test that the various builders produce the same results.

use fontique::{FontStyle, FontWeight, FontWidth};
use icu_segmenter::options::LineBreakWordOption;
use peniko::color::palette;
use std::borrow::Cow;

use super::utils::{ColorBrush, FONT_STACK, asserts::assert_eq_layout_data, create_font_context};
use crate::{
    FontContext, FontSettings, FontStack, Layout, LayoutContext, LineHeight, OverflowWrap,
    RangedBuilder, StyleProperty, TextStyle, TreeBuilder,
};

/// Set of options for [`build_layout_with_ranged`].
struct RangedOptions<'a> {
    scale: f32,
    quantize: bool,
    max_advance: Option<f32>,
    text: &'a str,
}

/// Set of options for [`build_layout_with_tree`].
struct TreeOptions<'a, 'b> {
    scale: f32,
    quantize: bool,
    max_advance: Option<f32>,
    root_style: &'a TextStyle<'b, ColorBrush>,
}

/// Generates a `Layout` with a ranged builder.
fn build_layout_with_ranged(
    fcx: &mut FontContext,
    lcx: &mut LayoutContext<ColorBrush>,
    opts: &RangedOptions<'_>,
    with_builder: impl Fn(&mut RangedBuilder<'_, ColorBrush>),
) -> Layout<ColorBrush> {
    let mut rb = lcx.ranged_builder(fcx, opts.text, opts.scale, opts.quantize);
    with_builder(&mut rb);
    let mut layout = rb.build(opts.text);
    layout.break_all_lines(opts.max_advance);
    layout
}

/// Generates a `Layout` with a tree builder.
fn build_layout_with_tree(
    fcx: &mut FontContext,
    lcx: &mut LayoutContext<ColorBrush>,
    opts: &TreeOptions<'_, '_>,
    with_builder: impl Fn(&mut TreeBuilder<'_, ColorBrush>),
) -> Layout<ColorBrush> {
    let mut tb = lcx.tree_builder(fcx, opts.scale, opts.quantize, opts.root_style);
    with_builder(&mut tb);
    let (mut layout, _) = tb.build();
    layout.break_all_lines(opts.max_advance);
    layout
}

/// Computes layout in various ways to ensure they all produce the same result.
///
/// ```text
/// LayoutContext A - Ranged
/// LayoutContext A - Ranged for idempotency
///
/// LayoutContext B - Tree
/// LayoutContext B - Tree for idempotency
///
/// LayoutContext C - Ranged for dirt
/// LayoutContext C - Tree from dirty
///
/// LayoutContext D - Tree for dirt
/// LayoutContext D - Ranged from dirty
/// ```
fn assert_builders_produce_same_result<'a, 'b>(
    text: &str,
    scale: f32,
    quantize: bool,
    max_advance: Option<f32>,
    root_style: &'a TextStyle<'b, ColorBrush>,
    with_ranged_builder: impl Fn(&mut RangedBuilder<'_, ColorBrush>),
    with_tree_builder: impl Fn(&mut TreeBuilder<'_, ColorBrush>),
) {
    let mut fcx = create_font_context();

    let mut lcx_a: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut lcx_b: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut lcx_c: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut lcx_d: LayoutContext<ColorBrush> = LayoutContext::new();

    let ropts = RangedOptions {
        scale,
        quantize,
        max_advance,
        text,
    };
    let topts = TreeOptions {
        scale,
        quantize,
        max_advance,
        root_style,
    };

    // Source of truth - ranged builder from a clean layout context
    let layout_truth = build_layout_with_ranged(&mut fcx, &mut lcx_a, &ropts, &with_ranged_builder);
    assert!(
        !layout_truth.data.runs.is_empty(),
        "expected runs to exist for lcx_a_rb_one"
    );

    // Testing idempotence of ranged builder creation
    let layout = build_layout_with_ranged(&mut fcx, &mut lcx_a, &ropts, &with_ranged_builder);
    assert!(
        !layout.data.runs.is_empty(),
        "expected runs to exist for lcx_a_rb_two"
    );
    assert_eq_layout_data(&layout_truth.data, &layout.data, "lcx_a_rb_two");

    // Basic builder compatibility - tree builder from a clean layout context
    let layout = build_layout_with_tree(&mut fcx, &mut lcx_b, &topts, &with_tree_builder);
    assert!(
        !layout.data.runs.is_empty(),
        "expected runs to exist for lcx_b_tb_one"
    );
    assert_eq_layout_data(&layout_truth.data, &layout.data, "lcx_b_tb_one");

    // Testing idempotence of tree builder creation
    let layout = build_layout_with_tree(&mut fcx, &mut lcx_b, &topts, &with_tree_builder);
    assert!(
        !layout.data.runs.is_empty(),
        "expected runs to exist for lcx_b_tb_two"
    );
    assert_eq_layout_data(&layout_truth.data, &layout.data, "lcx_b_tb_two");

    // Priming a fresh layout context with ranged builder creation
    let _ = build_layout_with_ranged(&mut fcx, &mut lcx_c, &ropts, &with_ranged_builder);

    // Testing tree builder creation with a dirty layout context
    let layout = build_layout_with_tree(&mut fcx, &mut lcx_c, &topts, &with_tree_builder);
    assert!(
        !layout.data.runs.is_empty(),
        "expected runs to exist for lcx_c_tb_one"
    );
    assert_eq_layout_data(&layout_truth.data, &layout.data, "lcx_c_tb_one");

    // Priming a fresh layout context with tree builder creation
    let _ = build_layout_with_tree(&mut fcx, &mut lcx_d, &topts, &with_tree_builder);

    // Testing ranged builder creation with a dirty layout context
    let layout = build_layout_with_ranged(&mut fcx, &mut lcx_d, &ropts, &with_ranged_builder);
    assert!(
        !layout.data.runs.is_empty(),
        "expected runs to exist for lcx_d_rb_one"
    );
    assert_eq_layout_data(&layout_truth.data, &layout.data, "lcx_d_rb_one");
}

/// Returns a root style that uses non-default values.
///
/// The [`TreeBuilder`] version of [`set_root_style`].
fn create_root_style() -> TextStyle<'static, ColorBrush> {
    TextStyle {
        font_stack: FontStack::from(FONT_STACK),
        font_size: 20.,
        font_width: FontWidth::CONDENSED,
        font_style: FontStyle::Italic,
        font_weight: FontWeight::BOLD,
        font_variations: FontSettings::List(Cow::Borrowed(&[])), // TODO: Set a non-default value
        font_features: FontSettings::List(Cow::Borrowed(&[])),   // TODO: Set a non-default value
        locale: Some("en-US"),
        brush: ColorBrush::new(palette::css::GREEN),
        has_underline: true,
        underline_offset: Some(2.),
        underline_size: Some(3.5),
        underline_brush: Some(ColorBrush::new(palette::css::CYAN)),
        has_strikethrough: true,
        strikethrough_offset: Some(1.3),
        strikethrough_size: Some(1.7),
        strikethrough_brush: Some(ColorBrush::new(palette::css::BEIGE)),
        line_height: LineHeight::Absolute(30.),
        word_spacing: 2.,
        letter_spacing: 1.5,
        word_break: LineBreakWordOption::BreakAll,
        overflow_wrap: OverflowWrap::Anywhere,
    }
}

/// Sets a root style with non-default values.
///
/// The [`RangedBuilder`] version of [`create_root_style`].
fn set_root_style(rb: &mut RangedBuilder<'_, ColorBrush>) {
    rb.push_default(FontStack::from(FONT_STACK));
    rb.push_default(StyleProperty::FontSize(20.));
    rb.push_default(StyleProperty::FontWidth(FontWidth::CONDENSED));
    rb.push_default(StyleProperty::FontStyle(FontStyle::Italic));
    rb.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
    rb.push_default(StyleProperty::FontVariations(FontSettings::List(
        Cow::Borrowed(&[]),
    )));
    rb.push_default(StyleProperty::FontFeatures(FontSettings::List(
        Cow::Borrowed(&[]),
    )));
    rb.push_default(StyleProperty::Locale(Some("en-US")));
    rb.push_default(StyleProperty::Brush(ColorBrush::new(palette::css::GREEN)));
    rb.push_default(StyleProperty::Underline(true));
    rb.push_default(StyleProperty::UnderlineOffset(Some(2.)));
    rb.push_default(StyleProperty::UnderlineSize(Some(3.5)));
    rb.push_default(StyleProperty::UnderlineBrush(Some(ColorBrush::new(
        palette::css::CYAN,
    ))));
    rb.push_default(StyleProperty::Strikethrough(true));
    rb.push_default(StyleProperty::StrikethroughOffset(Some(1.3)));
    rb.push_default(StyleProperty::StrikethroughSize(Some(1.7)));
    rb.push_default(StyleProperty::StrikethroughBrush(Some(ColorBrush::new(
        palette::css::BEIGE,
    ))));
    rb.push_default(LineHeight::Absolute(30.));
    rb.push_default(StyleProperty::WordSpacing(2.));
    rb.push_default(StyleProperty::LetterSpacing(1.5));
    rb.push_default(StyleProperty::WordBreak(LineBreakWordOption::BreakAll));
    rb.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));
}

/// Test that all the builders have the same default behavior.
#[test]
fn builders_default() {
    let text = "Builders often wear hard hats for safety while working on construction sites.";
    let scale = 2.;
    let quantize = false;
    let max_advance = Some(50.);
    let root_style = TextStyle {
        font_stack: FontStack::from(FONT_STACK),
        ..TextStyle::default()
    };

    let with_ranged_builder = |rb: &mut RangedBuilder<'_, ColorBrush>| {
        rb.push_default(FontStack::from(FONT_STACK));
    };
    let with_tree_builder = |tb: &mut TreeBuilder<'_, ColorBrush>| {
        tb.push_text(text);
    };

    assert_builders_produce_same_result(
        text,
        scale,
        quantize,
        max_advance,
        &root_style,
        with_ranged_builder,
        with_tree_builder,
    );
}

/// Test that all the builders behave the same when given the same root style.
#[test]
fn builders_root_only() {
    let text = "Builders often wear hard hats for safety while working on construction sites.";
    let scale = 2.;
    let quantize = false;
    let max_advance = Some(50.);
    let root_style = create_root_style();

    let with_ranged_builder = |rb: &mut RangedBuilder<'_, ColorBrush>| {
        set_root_style(rb);
    };
    let with_tree_builder = |tb: &mut TreeBuilder<'_, ColorBrush>| {
        tb.push_text(text);
    };

    assert_builders_produce_same_result(
        text,
        scale,
        quantize,
        max_advance,
        &root_style,
        with_ranged_builder,
        with_tree_builder,
    );
}

/// Test that all the builders behave the same with mixed styles.
#[test]
fn builders_mixed_styles() {
    let text = "Builders often wear hard hats for safety while working on construction sites.";
    let scale = 2.;
    let quantize = false;
    let max_advance = Some(50.);
    let root_style = create_root_style();

    let with_ranged_builder = |rb: &mut RangedBuilder<'_, ColorBrush>| {
        set_root_style(rb);

        // Make the first word bigger
        rb.push(StyleProperty::FontSize(68.), 0..8);
        // Push two modified styles for the same range
        rb.push(StyleProperty::LetterSpacing(4.), 12..17);
        rb.push(StyleProperty::WordSpacing(3.), 12..17);
        // Plus, change the line height for the last letter
        rb.push(StyleProperty::LineHeight(LineHeight::Absolute(40.)), 16..17);
    };
    let with_tree_builder = |tb: &mut TreeBuilder<'_, ColorBrush>| {
        // Make the first word bigger
        tb.push_style_modification_span(&[StyleProperty::FontSize(68.)]);
        tb.push_text(&text[..8]);
        tb.pop_style_span();

        tb.push_text(&text[8..12]);

        // Push two modified styles in batch
        tb.push_style_modification_span(&[
            StyleProperty::LetterSpacing(4.),
            StyleProperty::WordSpacing(3.),
        ]);
        tb.push_text(&text[12..16]);
        // Plus, change the line height for the last letter
        tb.push_style_modification_span(&[StyleProperty::LineHeight(LineHeight::Absolute(40.))]);
        tb.push_text(&text[16..17]);
        tb.pop_style_span();
        tb.pop_style_span();

        tb.push_text(&text[17..]);
    };

    assert_builders_produce_same_result(
        text,
        scale,
        quantize,
        max_advance,
        &root_style,
        with_ranged_builder,
        with_tree_builder,
    );
}
