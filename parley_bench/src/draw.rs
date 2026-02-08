// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # Draw Benchmarks
//!
//! Benchmarks for text rendering using `parley_draw` with `vello_cpu`.

use crate::{ColorBrush, FONT_FAMILY_LIST, with_contexts};
use parley::{
    Alignment, AlignmentOptions, FontFamily, Layout, PositionedLayoutItem, StyleProperty,
};
use parley_draw::{Glyph, GlyphCaches, GlyphRunBuilder};
use std::hint::black_box;
use tango_bench::{Benchmark, benchmark_fn};
use vello_cpu::{RenderContext, kurbo};

const DISPLAY_SCALE: f32 = 1.0;
const QUANTIZE: bool = true;
const MAX_ADVANCE: f32 = 400.0 * DISPLAY_SCALE;
const PADDING: u16 = 20;

/// Long sample text for benchmarking.
const SAMPLE_TEXT: &str = "Call me Ishmael. Some years ago—never mind how long precisely—having\
little or no money in my purse, and nothing particular to interest me\
on shore, I thought I would sail about a little and see the watery part\
of the world. It is a way I have of driving off the spleen and\
regulating the circulation. Whenever I find myself growing grim about\
the mouth; whenever it is a damp, drizzly November in my soul; whenever\
I find myself involuntarily pausing before coffin warehouses, and\
bringing up the rear of every funeral I meet; and especially whenever\
my hypos get such an upper hand of me, that it requires a strong moral\
principle to prevent me from deliberately stepping into the street, and\
methodically knocking people’s hats off—then, I account it high time to\
get to sea as soon as I can. This is my substitute for pistol and ball.\
With a philosophical flourish Cato throws himself upon his sword; I\
quietly take to the ship. There is nothing surprising in this. If they\
but knew it, almost all men in their degree, some time or other,\
cherish very nearly the same feelings towards the ocean with me.

There now is your insular city of the Manhattoes, belted round by\
wharves as Indian isles by coral reefs—commerce surrounds it with her\
surf. Right and left, the streets take you waterward. Its extreme\
downtown is the battery, where that noble mole is washed by waves, and\
cooled by breezes, which a few hours previous were out of sight of\
land. Look at the crowds of water-gazers there.";

/// Builds a layout with or without underlines.
fn build_layout(text: &str, underline: bool) -> Layout<ColorBrush> {
    with_contexts(|font_cx, layout_cx| {
        let mut builder = layout_cx.ranged_builder(font_cx, text, DISPLAY_SCALE, QUANTIZE);
        builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
        builder.push_default(StyleProperty::FontSize(16.0));

        if underline {
            builder.push(StyleProperty::Underline(true), 0..text.len());
        }

        let mut layout: Layout<ColorBrush> = builder.build(text);
        layout.break_all_lines(Some(MAX_ADVANCE));
        layout.align(
            Some(MAX_ADVANCE),
            Alignment::Start,
            AlignmentOptions::default(),
        );
        layout
    })
}

/// Renders a layout to a renderer, optionally with underlines.
fn render_layout(
    layout: &Layout<ColorBrush>,
    renderer: &mut RenderContext,
    glyph_caches: &mut GlyphCaches,
    with_underline: bool,
) {
    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    let run = glyph_run.run();
                    GlyphRunBuilder::new(run.font().clone(), *renderer.transform(), renderer)
                        .font_size(run.font_size())
                        .hint(true)
                        .normalized_coords(run.normalized_coords())
                        .fill_glyphs(
                            glyph_run.positioned_glyphs().map(|glyph| Glyph {
                                id: glyph.id,
                                x: glyph.x,
                                y: glyph.y,
                            }),
                            glyph_caches,
                        );

                    if with_underline {
                        if let Some(decoration) = &glyph_run.style().underline {
                            let offset =
                                decoration.offset.unwrap_or(run.metrics().underline_offset);
                            let size = decoration.size.unwrap_or(run.metrics().underline_size);

                            let x = glyph_run.offset();
                            let x1 = x + glyph_run.advance();
                            let baseline = glyph_run.baseline();

                            GlyphRunBuilder::new(
                                run.font().clone(),
                                *renderer.transform(),
                                renderer,
                            )
                            .font_size(run.font_size())
                            .hint(true)
                            .normalized_coords(run.normalized_coords())
                            .render_decoration(
                                glyph_run.positioned_glyphs().map(|glyph| Glyph {
                                    id: glyph.id,
                                    x: glyph.x,
                                    y: glyph.y,
                                }),
                                x..=x1,
                                baseline,
                                offset,
                                size,
                                1.0, // buffer around exclusions
                                glyph_caches,
                            );
                        }
                    }
                }
                PositionedLayoutItem::InlineBox(_) => {}
            }
        }
    }
}

/// Creates the render context for drawing.
fn create_renderer(layout: &Layout<ColorBrush>) -> RenderContext {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the layout's not *that* big"
    )]
    let width = layout.width().ceil() as u16 + PADDING * 2;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the layout's not *that* big"
    )]
    let height = layout.height().ceil() as u16 + PADDING * 2;

    let mut renderer = RenderContext::new(width, height);
    renderer.set_transform(kurbo::Affine::translate(kurbo::Vec2::new(
        PADDING as f64,
        PADDING as f64,
    )));
    renderer
}

/// Benchmark for drawing text without underlines, with a fresh cache each iteration.
pub fn draw_no_underline_cold_cache() -> Vec<Benchmark> {
    vec![benchmark_fn("Draw - No underline (cold cache)", |b| {
        let layout = build_layout(SAMPLE_TEXT, false);

        b.iter(move || {
            let layout = layout.clone();
            let mut renderer = create_renderer(&layout);
            let mut glyph_caches = GlyphCaches::new();
            render_layout(&layout, &mut renderer, &mut glyph_caches, false);
            black_box(&renderer);
        })
    })]
}

/// Benchmark for drawing text without underlines, reusing the cache.
pub fn draw_no_underline_warm_cache() -> Vec<Benchmark> {
    vec![benchmark_fn("Draw - No underline (warm cache)", |b| {
        let layout = build_layout(SAMPLE_TEXT, false);
        let mut glyph_caches = GlyphCaches::new();

        b.iter(move || {
            let mut renderer = create_renderer(&layout);
            render_layout(&layout, &mut renderer, &mut glyph_caches, false);
            glyph_caches.maintain();
            black_box(&renderer);
        })
    })]
}

/// Benchmark for drawing text with underlines, with a fresh cache each iteration.
pub fn draw_with_underline_cold_cache() -> Vec<Benchmark> {
    vec![benchmark_fn("Draw - With underline (cold cache)", |b| {
        let layout = build_layout(SAMPLE_TEXT, true);

        b.iter(move || {
            let layout = layout.clone();
            let mut renderer = create_renderer(&layout);
            let mut glyph_caches = GlyphCaches::new();
            render_layout(&layout, &mut renderer, &mut glyph_caches, true);
            black_box(&renderer);
        })
    })]
}

/// Benchmark for drawing text with underlines, reusing the cache.
pub fn draw_with_underline_warm_cache() -> Vec<Benchmark> {
    vec![benchmark_fn("Draw - With underline (warm cache)", |b| {
        let layout = build_layout(SAMPLE_TEXT, true);
        let mut glyph_caches = GlyphCaches::new();

        b.iter(move || {
            let mut renderer = create_renderer(&layout);
            render_layout(&layout, &mut renderer, &mut glyph_caches, true);
            glyph_caches.maintain();
            black_box(&renderer);
        })
    })]
}
