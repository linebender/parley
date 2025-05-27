// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Test that the various builders produce the same results.

use std::borrow::Cow;

use fontique::{FontStyle, FontWeight, FontWidth};
use peniko::color::palette;
use swash::text::WordBreakStrength;

use super::utils::{ColorBrush, FONT_STACK, asserts::assert_eq_layout_data, create_font_context};
use crate::{
    FontSettings, FontStack, LayoutContext, LineHeight, OverflowWrap, RangedBuilder, StyleProperty,
    TextStyle, TreeBuilder,
};

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
fn assert_builders_produce_same_result(
    text: &str,
    scale: f32,
    quantize: bool,
    max_advance: Option<f32>,
    root_style: &TextStyle<'_, ColorBrush>,
    with_ranged_builder: impl Fn(&mut RangedBuilder<'_, ColorBrush>),
    with_tree_builder: impl Fn(&mut TreeBuilder<'_, ColorBrush>),
) {
    let mut fcx = create_font_context();

    let mut lcx_a: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut lcx_b: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut lcx_c: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut lcx_d: LayoutContext<ColorBrush> = LayoutContext::new();

    // Source of truth - ranged builder from a clean layout context
    let mut lcx_a_rb_one = lcx_a.ranged_builder(&mut fcx, text, scale, quantize);
    with_ranged_builder(&mut lcx_a_rb_one);
    let mut lcx_a_rb_one_layout = lcx_a_rb_one.build(text);
    lcx_a_rb_one_layout.break_all_lines(max_advance);
    assert!(
        !lcx_a_rb_one_layout.data.runs.is_empty(),
        "expected runs to exist for lcx_a_rb_one_layout"
    );

    // Testing idempotence of ranged builder creation
    let mut lcx_a_rb_two = lcx_a.ranged_builder(&mut fcx, text, scale, quantize);
    with_ranged_builder(&mut lcx_a_rb_two);
    let mut lcx_a_rb_two_layout = lcx_a_rb_two.build(text);
    lcx_a_rb_two_layout.break_all_lines(max_advance);
    assert!(
        !lcx_a_rb_two_layout.data.runs.is_empty(),
        "expected runs to exist for lcx_a_rb_two_layout"
    );

    assert_eq_layout_data(
        &lcx_a_rb_one_layout.data,
        &lcx_a_rb_two_layout.data,
        "lcx_a_rb_two",
    );

    // Basic builder compatibility - tree builder from a clean layout context
    let mut lcx_b_tb_one = lcx_b.tree_builder(&mut fcx, scale, quantize, root_style);
    with_tree_builder(&mut lcx_b_tb_one);
    let (mut lcx_b_tb_one_layout, _) = lcx_b_tb_one.build();
    lcx_b_tb_one_layout.break_all_lines(max_advance);
    assert!(
        !lcx_b_tb_one_layout.data.runs.is_empty(),
        "expected runs to exist for lcx_b_tb_one_layout"
    );

    assert_eq_layout_data(
        &lcx_a_rb_one_layout.data,
        &lcx_b_tb_one_layout.data,
        "lcx_b_tb_one",
    );

    // Testing idempotence of tree builder creation
    let mut lcx_b_tb_two = lcx_b.tree_builder(&mut fcx, scale, quantize, root_style);
    with_tree_builder(&mut lcx_b_tb_two);
    let (mut lcx_b_tb_two_layout, _) = lcx_b_tb_two.build();
    lcx_b_tb_two_layout.break_all_lines(max_advance);
    assert!(
        !lcx_b_tb_two_layout.data.runs.is_empty(),
        "expected runs to exist for lcx_b_tb_two_layout"
    );

    assert_eq_layout_data(
        &lcx_a_rb_one_layout.data,
        &lcx_b_tb_two_layout.data,
        "lcx_b_tb_two",
    );

    // Priming a fresh layout context with ranged builder creation
    let mut lcx_c_rb_one = lcx_c.ranged_builder(&mut fcx, text, scale, quantize);
    with_ranged_builder(&mut lcx_c_rb_one);
    let _ = lcx_c_rb_one.build(text);

    // Testing tree builder creation with a dirty layout context
    let mut lcx_c_tb_one = lcx_c.tree_builder(&mut fcx, scale, quantize, root_style);
    with_tree_builder(&mut lcx_c_tb_one);
    let (mut lcx_c_tb_one_layout, _) = lcx_c_tb_one.build();
    lcx_c_tb_one_layout.break_all_lines(max_advance);
    assert!(
        !lcx_c_tb_one_layout.data.runs.is_empty(),
        "expected runs to exist for lcx_c_tb_one_layout"
    );

    assert_eq_layout_data(
        &lcx_a_rb_one_layout.data,
        &lcx_c_tb_one_layout.data,
        "lcx_c_tb_one",
    );

    // Priming a fresh layout context with tree builder creation
    let mut lcx_d_tb_one = lcx_d.tree_builder(&mut fcx, scale, quantize, root_style);
    with_tree_builder(&mut lcx_d_tb_one);
    let _ = lcx_d_tb_one.build();

    // Testing ranged builder creation with a dirty layout context
    let mut lcx_d_rb_one = lcx_d.ranged_builder(&mut fcx, text, scale, quantize);
    with_ranged_builder(&mut lcx_d_rb_one);
    let mut lcx_d_rb_one_layout = lcx_d_rb_one.build(text);
    lcx_d_rb_one_layout.break_all_lines(max_advance);
    assert!(
        !lcx_d_rb_one_layout.data.runs.is_empty(),
        "expected runs to exist for lcx_d_rb_one_layout"
    );

    assert_eq_layout_data(
        &lcx_a_rb_one_layout.data,
        &lcx_d_rb_one_layout.data,
        "lcx_d_rb_one",
    );
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
        word_break: WordBreakStrength::BreakAll,
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
    rb.push_default(StyleProperty::WordBreak(WordBreakStrength::BreakAll));
    rb.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));
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
