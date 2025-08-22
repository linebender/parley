//! # Default Style
//!
//! This module provides a benchmark for the default style.

use crate::{ColorBrush, get_contexts, get_samples};
use parley::{Alignment, AlignmentOptions, Layout};
use std::hint::black_box;
use tango_bench::{Benchmark, benchmark_fn};

/// Returns a list of benchmarks for the default style.
pub fn default_style() -> Vec<Benchmark> {
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
                        // The text we are going to style and lay out
                        let text = sample.text;
                        let (mut font_cx_guard, mut layout_cx_guard) = get_contexts();

                        let builder = layout_cx_guard.ranged_builder(
                            &mut font_cx_guard,
                            &text,
                            DISPLAY_SCALE,
                            QUANTIZE,
                        );
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
