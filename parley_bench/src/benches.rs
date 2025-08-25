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
                        let text = sample.text;
                        let (mut font_cx_guard, mut layout_cx_guard) = get_contexts();

                        let mut builder = layout_cx_guard.ranged_builder(
                            &mut font_cx_guard,
                            &text,
                            DISPLAY_SCALE,
                            QUANTIZE,
                        );
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
                        let text = sample.text;
                        let (mut font_cx_guard, mut layout_cx_guard) = get_contexts();

                        let mut builder = layout_cx_guard.ranged_builder(
                            &mut font_cx_guard,
                            &text,
                            DISPLAY_SCALE,
                            QUANTIZE,
                        );
                        builder.push_default(StyleProperty::FontStack(FontStack::List(
                            FONT_STACK.into(),
                        )));
                        // Every 10 characters, push a new style
                        let mut style = Style::Default;
                        let mut char_indices = text.char_indices().peekable();
                        let mut i = 0;
                        while let Some((char_idx, _)) = char_indices.next() {
                            let peeked = char_indices.peek();
                            if let Some((peeked_idx, _)) = peeked {
                                if i % 10 == 0 {
                                    style = next_style(&style);
                                    apply_style(&mut builder, &style, char_idx, *peeked_idx);
                                }
                            }
                            i += 1;
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

enum Style {
    Default,
    Italic,
    Bold,
    Underline,
    Strikethrough,
}
fn next_style(style: &Style) -> Style {
    match *style {
        Style::Default => Style::Italic,
        Style::Italic => Style::Bold,
        Style::Bold => Style::Underline,
        Style::Underline => Style::Strikethrough,
        Style::Strikethrough => Style::Default,
    }
}
fn apply_style(
    builder: &mut RangedBuilder<ColorBrush>,
    style: &Style,
    start_idx: usize,
    end_idx: usize,
) {
    let range = start_idx..end_idx;
    match style {
        Style::Default => {}
        Style::Italic => builder.push(StyleProperty::FontStyle(FontStyle::Italic), range),
        Style::Bold => builder.push(StyleProperty::FontWeight(FontWeight::BOLD), range),
        Style::Underline => builder.push(StyleProperty::Underline(true), range),
        Style::Strikethrough => builder.push(StyleProperty::Strikethrough(true), range),
    }
}
