// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Benchmarks
//!
//! This module provides benchmarks for text layout and rendering.

use crate::{ColorBrush, FONT_FAMILY_LIST, get_samples, with_contexts};
use parley::{
    Alignment, AlignmentOptions, FontFamily, FontStyle, FontWeight, Layout, PositionedLayoutItem,
    StyleProperty,
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
        .find(|sample| sample.name == "latin" && sample.modification == "8000 characters")
        .expect("the Latin 8000-character benchmark sample should exist");

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

/// Benchmark breaking a layout into lines.
///
/// This is part of the work that'd be performed as, e.g., a window is being resized.
pub fn line_breaking() -> Vec<Benchmark> {
    const NARROW_MAX_ADVANCE: f32 = 50.;
    const MAX_ADVANCE: f32 = 200.;
    const WORD_SPACING: f32 = 2.;
    const LETTER_SPACING: f32 = 1.;

    // The maximum advances to break at, with the name each is given in the benchmark name.
    const MAX_ADVANCES: [(&str, Option<f32>); 3] = [
        // Narrow lines break often and will have some emergency-breaks within words that don't fit.
        ("narrow", Some(NARROW_MAX_ADVANCE)),
        ("wider", Some(MAX_ADVANCE)),
        // Unwrapped lines only break at mandatory line breaks.
        ("unwrapped", None),
    ];

    fn break_lines(layout: &mut Layout<ColorBrush>, max_advance: Option<f32>) {
        layout.break_all_lines(max_advance);
        black_box((layout.len(), layout.width(), layout.height()));
    }

    // The samples are long enough to break into a meaningful number of lines.
    let samples = || {
        get_samples()
            .iter()
            .filter(|sample| sample.modification == "8000 characters")
    };

    let mut benchmarks = Vec::new();

    for sample in samples() {
        for (max_advance_name, max_advance) in MAX_ADVANCES {
            benchmarks.push(benchmark_fn(
                format!(
                    "Line Breaking - {} {}, {max_advance_name}",
                    sample.name, sample.modification
                ),
                move |b| {
                    let mut layout = build_layout(&sample.text, []);
                    b.iter(move || break_lines(&mut layout, max_advance))
                },
            ));
        }
    }

    // Also bench with word and letter spacing applied, as there are some zero-spacing fast paths.
    for sample in samples() {
        benchmarks.push(benchmark_fn(
            format!(
                "Line Breaking - {} {}, wrapped + spacing",
                sample.name, sample.modification
            ),
            |b| {
                let text_range = 0..sample.text.len();
                let mut layout = build_layout(
                    &sample.text,
                    [
                        (StyleProperty::WordSpacing(WORD_SPACING), text_range.clone()),
                        (StyleProperty::LetterSpacing(LETTER_SPACING), text_range),
                    ],
                );
                b.iter(move || break_lines(&mut layout, Some(MAX_ADVANCE)))
            },
        ));
    }

    benchmarks
}

/// Get the byte ranges of each consecutive chunk of `char_len` characters.
///
/// The last chunk holds the characters remaining.
fn chunks(text: &str, char_len: usize) -> impl Iterator<Item = Range<usize>> {
    let mut starts = text
        .char_indices()
        .map(|(byte_idx, _)| byte_idx)
        .step_by(char_len)
        .peekable();
    std::iter::from_fn(move || {
        let start = starts.next()?;
        let end = starts.peek().copied().unwrap_or(text.len());
        Some(start..end)
    })
}

/// Create style spans changing style every few characters.
fn styled_spans(
    text: &str,
) -> impl Iterator<Item = (StyleProperty<'static, ColorBrush>, Range<usize>)> {
    let style_interval = (text.len() / 5).min(10);
    chunks(text, style_interval)
        .enumerate()
        .filter_map(|(style_idx, range)| {
            let style = match style_idx % 5 {
                0 => StyleProperty::FontStyle(FontStyle::Italic),
                1 => StyleProperty::FontWeight(FontWeight::BOLD),
                2 => StyleProperty::Underline(true),
                3 => StyleProperty::Strikethrough(true),
                4 => return None, // Default style
                _ => unreachable!(),
            };
            Some((style, range))
        })
}

/// Apply `style` to every other chunk of `chunk_char_len` characters of `text`.
fn alternating_spans<'s>(
    text: &str,
    chunk_char_len: usize,
    style: &StyleProperty<'s, ColorBrush>,
) -> impl Iterator<Item = (StyleProperty<'s, ColorBrush>, Range<usize>)> {
    chunks(text, chunk_char_len)
        .step_by(2)
        .map(move |range| (style.clone(), range))
}

/// Build a styled layout for `text`, changing style every few characters.
fn build_styled_layout(text: &str) -> Layout<ColorBrush> {
    const DISPLAY_SCALE: f32 = 1.0;
    const MAX_ADVANCE: f32 = 200.0 * DISPLAY_SCALE;

    let mut layout = build_layout(text, styled_spans(text));
    layout.break_all_lines(Some(MAX_ADVANCE));
    layout.align(Alignment::Start, AlignmentOptions::default());
    layout
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
/// - 8000 characters of Latin with a single style, without line wrapping, giving one shaped run per
///   paragraph.
/// - 8000 characters of Latin with a style alternating every few characters, without line wrapping.
///   Glyph runs break wherever the style changes; some properties, such as bold, also split the
///   shaped run. This benches both cases.
/// - Mixed styling of each script with line wrapping. This covers script, bidi and font fallback
///   handling, and, being wrapped, the line-scoped runs a renderer normally walks.
pub fn iterate_glyph_runs() -> Vec<Benchmark> {
    let latin = get_samples()
        .iter()
        .find(|sample| sample.name == "latin" && sample.modification == "8000 characters")
        .expect("the Latin 8000-character benchmark sample should exist");

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
                let layout = build_unwrapped_layout(
                    &latin.text,
                    alternating_spans(&latin.text, chunk_len, &style),
                );
                b.iter(move || black_box(walk_items(&layout)))
            },
        ));
    }

    for sample in get_samples()
        .iter()
        .filter(|sample| sample.modification == "8000 characters")
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

/// Benchmark for computing content widths.
///
/// Note this benches the calculation on layouts before line breaking is performed; that's the
/// sequencing that'll likely be used by clients (i.e., measure possible sizes, and later on line
/// break once the target size is known).
pub fn content_widths() -> Vec<Benchmark> {
    let samples = get_samples();

    let mut benchmarks = Vec::new();

    for sample in samples.iter() {
        benchmarks.push(benchmark_fn(
            format!(
                "Content Widths - {} {}, uniform",
                sample.name, sample.modification
            ),
            |b| {
                let layout = build_layout(&sample.text, []);
                b.iter(move || black_box(layout.calculate_content_widths()))
            },
        ));
    }

    for sample in samples.iter() {
        benchmarks.push(benchmark_fn(
            format!(
                "Content Widths - {} {}, mixed",
                sample.name, sample.modification
            ),
            |b| {
                let layout = build_layout(&sample.text, styled_spans(&sample.text));
                b.iter(move || black_box(layout.calculate_content_widths()))
            },
        ));
    }

    benchmarks
}

/// Build `text` into a layout. Each style of `styles` is applied to its given byte range.
///
/// This doesn't break the layout into lines.
fn build_layout<'a>(
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

        builder.build(text)
    })
}

/// Build `text` and line break without a maximum advance, i.e., lines are not wrapped. Each style
/// of `styles` is applied to its given byte range.
///
/// The layout is a single long line. Depending on the styles used, the proportion of glyphs per
/// shaped run or style span varies. E.g., switching between bold and non-bold, the font changes, so
/// the styles split the text into separately shaped items. Switching between underline and
/// non-underline, there's no impact on shaping, and only the style spans change.
fn build_unwrapped_layout<'a>(
    text: &str,
    styles: impl IntoIterator<Item = (StyleProperty<'a, ColorBrush>, Range<usize>)>,
) -> Layout<ColorBrush> {
    let mut layout = build_layout(text, styles);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    layout
}

/// Benchmark for a single very long line (no wrapping) with and without justification.
///
/// This exercises per-line work that scales with line length.
pub fn long_line() -> Vec<Benchmark> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;

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
        .filter(|sample| sample.modification == "8000 characters")
        .flat_map(|sample| {
            let text: &'static str = Box::leak(sample.text.replace('\n', " ").into_boxed_str());
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
