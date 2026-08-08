// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Benchmarks
//!
//! This module provides benchmarks for text layout and rendering.

use crate::{ColorBrush, FONT_FAMILY_LIST, get_samples, with_contexts};
use parley::{
    Alignment, AlignmentOptions, FontFamily, FontStyle, FontWeight, Layout, RangedBuilder,
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
                        with_contexts(|font_cx, layout_cx| {
                            let mut builder =
                                layout_cx.ranged_builder(font_cx, text, DISPLAY_SCALE, QUANTIZE);
                            builder.push_default(FontFamily::from(FONT_FAMILY_LIST));

                            let mut layout: Layout<ColorBrush> = builder.build(text);
                            layout.break_all_lines(Some(MAX_ADVANCE));
                            layout.align(Alignment::Start, AlignmentOptions::default());

                            black_box(layout);
                        });
                    })
                },
            )
        })
        .collect()
}

/// Benchmark for nonzero word and letter spacing.
pub fn spacing() -> Vec<Benchmark> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;
    const MAX_ADVANCE: f32 = 200.0 * DISPLAY_SCALE;
    const WORD_SPACING: f32 = 2.0;
    const LETTER_SPACING: f32 = 1.0;

    let samples = get_samples();

    samples
        .iter()
        .map(|sample| {
            benchmark_fn(
                format!(
                    "Word + Letter Spacing - {} {}",
                    sample.name, sample.modification
                ),
                |b| {
                    b.iter(|| {
                        let text = &sample.text;
                        with_contexts(|font_cx, layout_cx| {
                            let mut builder =
                                layout_cx.ranged_builder(font_cx, text, DISPLAY_SCALE, QUANTIZE);
                            builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
                            builder.push_default(StyleProperty::WordSpacing(WORD_SPACING));
                            builder.push_default(StyleProperty::LetterSpacing(LETTER_SPACING));

                            let mut layout: Layout<ColorBrush> = builder.build(text);
                            layout.break_all_lines(Some(MAX_ADVANCE));
                            layout.align(Alignment::Start, AlignmentOptions::default());

                            black_box(layout);
                        });
                    })
                },
            )
        })
        .collect()
}

/// Benchmark repeatedly justifying an already line-broken layout.
pub fn repeated_justification() -> [Benchmark; 1] {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;
    const MAX_ADVANCE: f32 = 200.0 * DISPLAY_SCALE;

    let sample = get_samples()
        .iter()
        .find(|sample| sample.name == "latin" && sample.modification == "4 paragraph")
        .expect("the Latin four-paragraph benchmark sample should exist");

    [benchmark_fn(
        format!(
            "Repeated Justification - {} {}",
            sample.name, sample.modification
        ),
        move |b| {
            let text = &sample.text;
            let mut layout = with_contexts(|font_cx, layout_cx| {
                let mut builder = layout_cx.ranged_builder(font_cx, text, DISPLAY_SCALE, QUANTIZE);
                builder.push_default(FontFamily::from(FONT_FAMILY_LIST));

                let mut layout: Layout<ColorBrush> = builder.build(text);
                layout.break_all_lines(Some(MAX_ADVANCE));
                layout
            });

            b.iter(move || {
                layout.align(Alignment::Justify, AlignmentOptions::default());

                // Pass to black box so the optimizer cannot optimize alignment away.
                black_box(
                    layout
                        .lines()
                        .flat_map(|line| line.runs())
                        .map(|run| run.advance())
                        .sum::<f32>(),
                );
                layout.align(Alignment::Start, AlignmentOptions::default());

                // Pass to black box so the optimizer cannot optimize alignment away.
                black_box(
                    layout
                        .lines()
                        .flat_map(|line| line.runs())
                        .map(|run| run.advance())
                        .sum::<f32>(),
                )
            })
        },
    )]
}

/// Benchmark for styled text.
pub fn styled() -> Vec<Benchmark> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;
    const MAX_ADVANCE: f32 = 200.0 * DISPLAY_SCALE;

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

    let samples = get_samples();

    samples
        .iter()
        .map(|sample| {
            benchmark_fn(
                format!("Styled - {} {}", sample.name, sample.modification),
                |b| {
                    b.iter(|| {
                        let text = &sample.text;

                        with_contexts(|font_cx, layout_cx| {
                            let mut builder =
                                layout_cx.ranged_builder(font_cx, text, DISPLAY_SCALE, QUANTIZE);
                            builder.push_default(FontFamily::from(FONT_FAMILY_LIST));

                            // Apply different styles every `style_interval` characters
                            let style_interval = (text.len() / 5).min(10);
                            {
                                let mut chunk_start = 0;
                                let mut style_idx = 0;

                                for (char_count, (byte_idx, _)) in text.char_indices().enumerate() {
                                    if char_count != 0 && char_count % style_interval == 0 {
                                        apply_style(&mut builder, style_idx, chunk_start..byte_idx);
                                        chunk_start = byte_idx;
                                        style_idx += 1;
                                    }
                                }

                                // Apply style to the last chunk if there's remaining text
                                if chunk_start < text.len() {
                                    apply_style(&mut builder, style_idx, chunk_start..text.len());
                                }
                            }

                            let mut layout: Layout<ColorBrush> = builder.build(text);
                            layout.break_all_lines(Some(MAX_ADVANCE));
                            layout.align(Alignment::Start, AlignmentOptions::default());

                            black_box(layout);
                        });
                    })
                },
            )
        })
        .collect()
}
