// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Benchmarks
//!
//! This module provides benchmarks for text layout and rendering.

use crate::{ColorBrush, FONT_FAMILY_LIST, get_samples, with_contexts};
use parley::{
    Alignment, AlignmentOptions, FontFamily, FontStyle, FontWeight, Layout, PositionedLayoutItem,
    RangedBuilder, StyleProperty,
};
use std::hint::black_box;
use std::ops::Range;
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

/// Build a styled layout for `text`, changing style every few characters.
fn build_styled_layout(text: &str) -> Layout<ColorBrush> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;
    const MAX_ADVANCE: f32 = 200.0 * DISPLAY_SCALE;

    fn apply_style(
        builder: &mut RangedBuilder<'_, ColorBrush>,
        style_idx: usize,
        range: Range<usize>,
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

    with_contexts(|font_cx, layout_cx| {
        let mut builder = layout_cx.ranged_builder(font_cx, text, DISPLAY_SCALE, QUANTIZE);
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
        layout
    })
}

/// Benchmark for styled text.
pub fn styled() -> Vec<Benchmark> {
    let samples = get_samples();

    samples
        .iter()
        .map(|sample| {
            benchmark_fn(
                format!("Styled - {} {}", sample.name, sample.modification),
                |b| {
                    b.iter(|| {
                        black_box(build_styled_layout(&sample.text));
                    })
                },
            )
        })
        .collect()
}

/// Benchmark for iterating the positioned glyph runs and glyphs of a styled layout, as a renderer
/// would.
///
/// The cases fall into three groups:
///
/// - Four paragraphs of Latin with a single style, without line wrapping, giving four shaped runs.
/// - Four paragraphs of Latin with a style alternating every few characters, without line wrapping.
///   Glyph runs break wherever the style changes; some properties, such as bold, also split the
///   shaped run. This benches both cases.
/// - Mixed styling of each script with line wrapping. This covers script, bidi and font fallback
///   handling, and, being wrapped, the line-scoped runs a renderer normally walks.
pub fn iterate_glyph_runs() -> Vec<Benchmark> {
    let latin = get_samples()
        .iter()
        .find(|sample| sample.name == "latin" && sample.modification == "4 paragraph")
        .expect("the Latin four-paragraph benchmark sample should exist");

    let mut benchmarks = vec![benchmark_fn(
        format!(
            "Glyph Runs - {} {}, uniform",
            latin.name, latin.modification
        ),
        |b| {
            let layout = build_unwrapped_layout(&latin.text, []);
            b.iter(move || black_box(walk_items(&layout)))
        },
    )];

    // Underline splits the glyph runs only; bold also splits the shaped runs.
    for (label, chunk_len, style) in [
        ("non-splitting", 1, StyleProperty::Underline(true)),
        ("non-splitting", 16, StyleProperty::Underline(true)),
        ("splitting", 1, StyleProperty::FontWeight(FontWeight::BOLD)),
        ("splitting", 16, StyleProperty::FontWeight(FontWeight::BOLD)),
    ] {
        benchmarks.push(benchmark_fn(
            format!(
                "Glyph Runs - {} {}, {label} every {chunk_len} chars",
                latin.name, latin.modification
            ),
            move |b| {
                let layout = build_alternating_layout(&latin.text, chunk_len, &style);
                b.iter(move || black_box(walk_items(&layout)))
            },
        ));
    }

    for sample in get_samples()
        .iter()
        .filter(|sample| sample.modification == "4 paragraph")
    {
        benchmarks.push(benchmark_fn(
            format!(
                "Glyph Runs - {} {}, mixed",
                sample.name, sample.modification
            ),
            |b| {
                let layout = build_styled_layout(&sample.text);
                b.iter(move || black_box(walk_items(&layout)))
            },
        ));
    }

    benchmarks
}

/// Iterate the positioned glyph runs and glyphs of `layout`, as a renderer would.
fn walk_items(layout: &Layout<ColorBrush>) -> (usize, f32) {
    let mut glyph_count = 0_usize;
    let mut advance = 0.0_f32;
    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    for glyph in glyph_run.positioned_glyphs() {
                        glyph_count += 1;
                        advance += glyph.advance;
                    }
                }
                PositionedLayoutItem::InlineBox(inline_box) => {
                    advance += inline_box.width;
                }
            }
        }
    }
    (glyph_count, advance)
}

/// Build `text` without line wrapping, applying each style of `styles` to its byte range.
///
/// The layout is a single long line. Depending on the styles used, the proportion of glyphs per
/// shaped run or style span varies. E.g., switching between bold and non-bold, the font changes, so
/// the styles split the text into separately shaped items. Switching between underline and
/// non-underline, there's no impact on shaping, and only the style spans change.
fn build_unwrapped_layout<'a>(
    text: &str,
    styles: impl IntoIterator<Item = (StyleProperty<'a, ColorBrush>, Range<usize>)>,
) -> Layout<ColorBrush> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;

    with_contexts(|font_cx, layout_cx| {
        let mut builder = layout_cx.ranged_builder(font_cx, text, DISPLAY_SCALE, QUANTIZE);
        builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
        for (style, range) in styles {
            builder.push(style, range);
        }

        let mut layout: Layout<ColorBrush> = builder.build(text);
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());
        layout
    })
}

/// Build `text` without line wrapping, applying `style` to every other chunk of `chunk_len`
/// characters.
///
/// See [`build_unwrapped_layout`] for an explanation of the impact of varying styles.
fn build_alternating_layout(
    text: &str,
    chunk_len: usize,
    style: &StyleProperty<'_, ColorBrush>,
) -> Layout<ColorBrush> {
    // The byte offset of every `chunk_len`-th character, plus the end of the text.
    let bounds = text
        .char_indices()
        .step_by(chunk_len)
        .map(|(byte_idx, _)| byte_idx)
        .chain([text.len()])
        .collect::<Vec<_>>();
    build_unwrapped_layout(
        text,
        bounds
            .windows(2)
            .step_by(2)
            .map(|chunk| (style.clone(), chunk[0]..chunk[1])),
    )
}

/// Benchmark for a single very long line (no wrapping) with and without justification.
///
/// This exercises per-line work that scales with line length.
pub fn long_line() -> Vec<Benchmark> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;
    const REPEAT: usize = 4;

    fn layout_long_line(text: &str, max_advance: Option<f32>, alignment: Alignment) {
        with_contexts(|font_cx, layout_cx| {
            let mut builder = layout_cx.ranged_builder(font_cx, text, DISPLAY_SCALE, QUANTIZE);
            builder.push_default(FontFamily::from(FONT_FAMILY_LIST));

            let mut layout: Layout<ColorBrush> = builder.build(text);
            layout.break_all_lines(max_advance);
            layout.align(alignment, AlignmentOptions::default());

            black_box(layout);
        });
    }

    let samples = get_samples();

    samples
        .iter()
        .filter(|sample| sample.modification == "4 paragraph")
        .flat_map(|sample| {
            let text: &'static str = Box::leak(
                sample
                    .text
                    .replace('\n', " ")
                    .repeat(REPEAT)
                    .into_boxed_str(),
            );
            [
                benchmark_fn(format!("Long Line - {}", sample.name), move |b| {
                    b.iter(move || layout_long_line(text, None, Alignment::Start))
                }),
                benchmark_fn(format!("Long Line Justify - {}", sample.name), move |b| {
                    // Justification requires a finite `max_advance`; use one wide enough that the
                    // text still lays out as a single line.
                    b.iter(move || layout_long_line(text, Some(1.0e7), Alignment::Justify))
                }),
            ]
        })
        .collect()
}
