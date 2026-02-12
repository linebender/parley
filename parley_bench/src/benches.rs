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
use parley_draw::{AtlasConfig, CpuGlyphCaches, GlyphCache, GlyphRunBuilder, ImageCache};
use std::cell::RefCell;
use std::hint::black_box;
use tango_bench::{Benchmark, benchmark_fn};
use vello_cpu::{Pixmap, RenderContext, kurbo};

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
                            layout.align(
                                Some(MAX_ADVANCE),
                                Alignment::Start,
                                AlignmentOptions::default(),
                            );

                            black_box(layout);
                        });
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
                            layout.align(
                                Some(MAX_ADVANCE),
                                Alignment::Start,
                                AlignmentOptions::default(),
                            );

                            black_box(layout);
                        });
                    })
                },
            )
        })
        .collect()
}

/// Helper function to check if a pixmap is empty (all pixels transparent/zero).
/// Prints a warning message if the pixmap is empty.
fn check_pixmap_is_empty(pixmap: &Pixmap, context: &str) {
    let data = pixmap.data();
    let is_empty = data
        .iter()
        .all(|pixel| pixel.r == 0 && pixel.g == 0 && pixel.b == 0 && pixel.a == 0);

    println!("{} -> {:?}", context, is_empty);
}

thread_local! {
    /// Thread-local storage for the main render context, reused across iterations.
    static RENDERER: RefCell<Option<RenderContext>> = const { RefCell::new(None) };
    /// Thread-local storage for the last rendered pixmap, used to save output after benchmarking.
    static PIXMAP: RefCell<Option<Pixmap>> = const { RefCell::new(None) };
    /// Thread-local storage for glyph caches, reused across iterations.
    static GLYPH_CACHES: RefCell<Option<CpuGlyphCaches>> = const { RefCell::new(None) };
    /// Thread-local storage for the image cache allocator, reused across iterations.
    static IMAGE_CACHE: RefCell<Option<ImageCache>> = const { RefCell::new(None) };
    /// Thread-local storage for glyph renderer, reused across iterations.
    static GLYPH_RENDERER: RefCell<Option<RenderContext>> = const { RefCell::new(None) };
}

/// Benchmark for glyph cache rendering (both cached and uncached).
pub fn glyph_cache() -> Vec<Benchmark> {
    const DISPLAY_SCALE: f32 = 1.0;
    const QUANTIZE: bool = true;
    const MAX_ADVANCE: f32 = 200.0 * DISPLAY_SCALE;

    let samples = get_samples();
    let mut benchmarks = Vec::new();

    // Generate benchmarks alternating cached/uncached for each sample
    for sample in samples.iter() {
        let sample_name = sample.name;
        let sample_modification = sample.modification;

        for use_cache in [false, true] {
            let cache_label = if use_cache { "✅" } else { "❌" };
            let text = sample.text.clone();

            benchmarks.push(benchmark_fn(
                format!(
                    "Glyph Render (cache {}) - {} {}",
                    cache_label, sample_name, sample_modification
                ),
                move |b| {
                    let text = text.clone();

                    // Setup: Create layout once outside the benchmark loop
                    let layout: Layout<ColorBrush> = with_contexts(|font_cx, layout_cx| {
                        let mut builder =
                            layout_cx.ranged_builder(font_cx, &text, DISPLAY_SCALE, QUANTIZE);
                        builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
                        let mut layout: Layout<ColorBrush> = builder.build(&text);
                        layout.break_all_lines(Some(MAX_ADVANCE));
                        layout.align(
                            Some(MAX_ADVANCE),
                            Alignment::Start,
                            AlignmentOptions::default(),
                        );
                        layout
                    });

                    let width = (layout.width().ceil() as u16).max(1);
                    let height = (layout.height().ceil() as u16).max(1);

                    // Setup: Create main renderer with correct dimensions
                    RENDERER.with(|r| {
                        *r.borrow_mut() = Some(RenderContext::new(width, height));
                    });

                    let glyph_renderer_size = (256, 256);

                    // Setup: Create glyph cache and renderer (once, not cleared between variants)
                    GLYPH_CACHES.with(|gc| {
                        if gc.borrow().is_none() {
                            *gc.borrow_mut() = Some(CpuGlyphCaches::with_page_size(
                                glyph_renderer_size.0,
                                glyph_renderer_size.1,
                            ));
                        }
                    });

                    IMAGE_CACHE.with(|ic| {
                        if ic.borrow().is_none() {
                            *ic.borrow_mut() = Some(ImageCache::new_with_config(AtlasConfig {
                                initial_atlas_count: 0,
                                max_atlases: 1,
                                atlas_size: (
                                    glyph_renderer_size.0 as u32,
                                    glyph_renderer_size.1 as u32,
                                ),
                                auto_grow: true,
                                ..Default::default()
                            }));
                        }
                    });

                    GLYPH_RENDERER.with(|gr| {
                        if gr.borrow().is_none() {
                            *gr.borrow_mut() = Some(RenderContext::new(
                                glyph_renderer_size.0,
                                glyph_renderer_size.1,
                            ));
                        }
                    });

                    // Save a reference render BEFORE b.iter(). In tango-bench,
                    // b.iter() does NOT execute the closure — it wraps it in a
                    // Sampler and returns a Box<dyn ErasedSampler>. The actual
                    // iterations only run later when tango calls measure(). Any
                    // code placed after b.iter() still runs during the factory
                    // phase, before any benchmark iteration has executed, so
                    // thread-locals like PIXMAP would always be empty.
                    let cache_suffix = if use_cache { "enabled" } else { "disabled" };
                    let path_prefix = format!(
                        "../examples/_output/bench_{}_{}_{}",
                        sample_name.to_lowercase().replace(' ', "_"),
                        sample_modification.replace(' ', "_"),
                        cache_suffix
                    );

                    // Clear cache statistics before warm-up
                    GLYPH_CACHES.with(|gc| {
                        if let Some(glyph_caches) = gc.borrow_mut().as_mut() {
                            glyph_caches.bitmap_cache.clear_stats();
                        }
                    });

                    RENDERER.with(|r| {
                        GLYPH_CACHES.with(|gc| {
                            IMAGE_CACHE.with(|ic| {
                                GLYPH_RENDERER.with(|gr| {
                                    let mut r = r.borrow_mut();
                                    let mut gc = gc.borrow_mut();
                                    let mut ic = ic.borrow_mut();
                                    let mut gr = gr.borrow_mut();

                                    let renderer = r.as_mut().unwrap();
                                    let glyph_caches = gc.as_mut().unwrap();
                                    let image_cache = ic.as_mut().unwrap();
                                    let glyph_renderer = gr.as_mut().unwrap();

                                    renderer.reset();
                                    glyph_renderer.reset();

                                    render_layout_glyphs(
                                        &layout,
                                        renderer,
                                        glyph_caches,
                                        image_cache,
                                        glyph_renderer,
                                        use_cache,
                                    );

                                    if let Some(atlas_pixmap) =
                                        glyph_caches.bitmap_cache.page_pixmap_mut(0)
                                    {
                                        glyph_renderer.flush();
                                        glyph_renderer.render_to_pixmap_region(atlas_pixmap, 0, 0);
                                        // check_pixmap_is_empty(atlas_pixmap, &"atlas_pixmap ");
                                    }
                                });
                            });
                        });
                    });

                    let result = b.iter(move || {
                        RENDERER.with(|r| {
                            GLYPH_CACHES.with(|gc| {
                                IMAGE_CACHE.with(|ic| {
                                    GLYPH_RENDERER.with(|gr| {
                                        let mut r = r.borrow_mut();
                                        let mut gc = gc.borrow_mut();
                                        let mut ic = ic.borrow_mut();
                                        let mut gr = gr.borrow_mut();

                                        let renderer = r.as_mut().unwrap();
                                        let glyph_caches = gc.as_mut().unwrap();
                                        let image_cache = ic.as_mut().unwrap();
                                        let glyph_renderer = gr.as_mut().unwrap();

                                        // Clear cache statistics at the start of each benchmark iteration
                                        // glyph_caches.bitmap_cache.clear_stats();

                                        renderer.reset();
                                        glyph_renderer.reset();

                                        render_layout_glyphs(
                                            &layout,
                                            renderer,
                                            glyph_caches,
                                            image_cache,
                                            glyph_renderer,
                                            use_cache,
                                        );

                                        // glyph_caches.bitmap_cache.print_cache_stats();

                                        // if let Some(atlas_pixmap) =
                                        //     glyph_caches.bitmap_cache.page_pixmap_mut(0)
                                        // {
                                        //     glyph_renderer.flush();
                                        //     glyph_renderer.render_to_pixmap_region(
                                        //         atlas_pixmap,
                                        //         0,
                                        //         0,
                                        //     );
                                        // }

                                        // // Register atlas pages with the render context
                                        // let page_count = glyph_caches.bitmap_cache.page_count();
                                        // for page_index in 0..page_count {
                                        //     if let Some(atlas_pixmap) =
                                        //         glyph_caches.bitmap_cache.page_pixmap(page_index)
                                        //     {
                                        //         renderer.register_image(atlas_pixmap.clone());
                                        //     }
                                        // }

                                        // // Render to pixmap
                                        // let mut pixmap = Pixmap::new(width, height);
                                        // renderer.render_to_pixmap(&mut pixmap);

                                        // // Store pixmap in thread-local for saving after benchmark
                                        // PIXMAP.with(|p| *p.borrow_mut() = Some(pixmap));

                                        black_box(&*renderer);
                                    });
                                });
                            });
                        });
                    });

                    // Save output once after benchmark completes (outside the timed loop)
                    PIXMAP.with(|p| {
                        if let Some(pixmap) = p.borrow_mut().take() {
                            let filename = format!("{path_prefix}.png");
                            let png_data = pixmap.into_png().expect("Failed to encode PNG");
                            std::fs::write(&filename, png_data).expect("Failed to write PNG");
                        }
                    });

                    // Save atlas pages after benchmark completes
                    GLYPH_CACHES.with(|gc| {
                        if let Some(glyph_caches) = gc.borrow().as_ref() {
                            glyph_caches.save_atlas_pages_to(&path_prefix);
                        }
                    });

                    result
                },
            ));
        }
    }

    benchmarks
}

/// Renders all glyphs from the layout using the provided caches and renderer.
///
/// This function is extracted to allow pre-warming the cache before benchmarking.
fn render_layout_glyphs(
    layout: &Layout<ColorBrush>,
    renderer: &mut RenderContext,
    glyph_caches: &mut CpuGlyphCaches,
    image_cache: &mut ImageCache,
    glyph_renderer: &mut RenderContext,
    use_cache: bool,
) {
    for line in layout.lines() {
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let run = glyph_run.run();
                GlyphRunBuilder::new(run.font().clone(), kurbo::Affine::IDENTITY, renderer)
                    .font_size(run.font_size())
                    .hint(true)
                    .normalized_coords(run.normalized_coords())
                    .bitmap_cache(use_cache)
                    .fill_glyphs(
                        glyph_run
                            .positioned_glyphs()
                            .map(|glyph| parley_draw::Glyph {
                                id: glyph.id,
                                x: glyph.x,
                                y: glyph.y,
                            }),
                        glyph_caches,
                        image_cache,
                    );
            }
        }
    }
}
