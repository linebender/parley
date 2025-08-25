//! # Default Style
//!
//! This module provides a benchmark for the default style.

use crate::{ColorBrush, FONT_STACK, get_contexts, get_samples};
use parley::{
    Alignment, AlignmentOptions, FontStack, FontStyle, FontWeight, Layout, RangedBuilder,
    StyleProperty,
};
use std::hint::black_box;
use tango_bench::{Benchmark, benchmark_fn};

/// Benchmark for default style.
pub fn defaults() -> Vec<Benchmark> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;
    const MAX_ADVANCE: f32 = 200.0 * DISPLAY_SCALE;

    let samples = get_samples();

    samples
        .iter()
        .map(|sample| {
            benchmark_fn(
                format!("Default Style - {} {}", sample.name, sample.modification),
                |b| {
                    b.iter(|| {
                        let text = &sample.text;
                        let (mut font_cx, mut layout_cx) = get_contexts();

                        let mut builder =
                            layout_cx.ranged_builder(&mut font_cx, &text, DISPLAY_SCALE, QUANTIZE);
                        builder.push_default(StyleProperty::FontStack(FontStack::List(
                            FONT_STACK.into(),
                        )));

                        let mut layout: Layout<ColorBrush> = builder.build(&text);
                        layout.break_all_lines(Some(MAX_ADVANCE));
                        layout.align(
                            Some(MAX_ADVANCE),
                            Alignment::Start,
                            AlignmentOptions::default(),
                        );

                        black_box(layout);
                    })
                },
            )
        })
        .collect()
}

/// Benchmark for styled text.
pub fn styled() -> Vec<Benchmark> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;
    const MAX_ADVANCE: f32 = 200.0 * DISPLAY_SCALE;

    let samples = get_samples();

    samples
        .iter()
        .map(|sample| {
            benchmark_fn(
                format!("Styled - {} {}", sample.name, sample.modification),
                |b| {
                    b.iter(|| {
                        let text = &sample.text;

                        let (mut font_cx, mut layout_cx) = get_contexts();

                        let mut builder =
                            layout_cx.ranged_builder(&mut font_cx, &text, DISPLAY_SCALE, QUANTIZE);
                        builder.push_default(StyleProperty::FontStack(FontStack::List(
                            FONT_STACK.into(),
                        )));

                        // Apply different styles every `style_interval` characters
                        let style_interval = (text.len() / 5).min(10);
                        {
                            let mut char_count = 0;
                            let mut chunk_start = 0;
                            let mut style_idx = 0;

                            for (byte_idx, _) in text.char_indices() {
                                if char_count != 0 && char_count % style_interval == 0 {
                                    apply_style(&mut builder, style_idx, chunk_start..byte_idx);
                                    chunk_start = byte_idx;
                                    style_idx += 1;
                                }
                                char_count += 1;
                            }

                            // Apply style to the last chunk if there's remaining text
                            if chunk_start < text.len() {
                                apply_style(&mut builder, style_idx, chunk_start..text.len());
                            }
                        }

                        let mut layout: Layout<ColorBrush> = builder.build(&text);
                        layout.break_all_lines(Some(MAX_ADVANCE));
                        layout.align(
                            Some(MAX_ADVANCE),
                            Alignment::Start,
                            AlignmentOptions::default(),
                        );

                        black_box(layout);
                    })
                },
            )
        })
        .collect()
}

fn apply_style(
    builder: &mut RangedBuilder<'_, ColorBrush>,
    style_idx: usize,
    range: std::ops::Range<usize>,
) {
    // Cycle through 5 different styles
    match style_idx % 5 {
        0 => builder.push(StyleProperty::FontStyle(FontStyle::Italic), range),
        1 => builder.push(StyleProperty::FontWeight(FontWeight::BOLD), range),
        2 => builder.push(StyleProperty::Underline(true), range),
        3 => builder.push(StyleProperty::Strikethrough(true), range),
        4 => {} // Default style
        _ => unreachable!(),
    }
}
