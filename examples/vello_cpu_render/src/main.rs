// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, extracts outlines using Skrifa and
//! then paints those outlines using Vello CPU through Parley Draw.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use parley::{GlyphRun, PositionedLayoutItem};
use parley_draw::renderers::vello_renderer::replay_atlas_commands;
use parley_draw::{AtlasConfig, CpuGlyphCaches, GlyphCache, GlyphRunBuilder, ImageCache};
use parley_examples_common::{ColorBrush, FrameStats, output_dir, prepare_example_layout};
use peniko::Color;
use vello_cpu::{
    Pixmap, RenderContext,
    kurbo::{Affine, Rect, Vec2},
};

fn main() {
    let start_time = Instant::now();

    let (layout, width, height, config) = prepare_example_layout();

    let mut stats = FrameStats::new();

    stats.start("prepare_rendering");
    let (mut renderer, mut glyph_renderer, mut glyph_caches, mut image_cache) =
        prepare_rendering(width, height);
    stats.end("prepare_rendering");

    let num_frames = 5;
    let frame_delay = Duration::from_millis(100);

    for frame in 0..num_frames {
        stats.start("total_frame");
        println!("Rendering frame {}/{}", frame + 1, num_frames);

        stats.start("reset_renderer");
        reset_renderer(
            &mut renderer,
            &mut glyph_renderer,
            width,
            height,
            config.padding,
            config.background_color,
        );
        stats.end("reset_renderer");

        // Enable accumulation mode for metrics that are measured many times per frame
        stats.start_accumulating("fill_glyphs");
        stats.start_accumulating("render_decoration");

        // Render each glyph run
        for line in layout.lines() {
            for item in line.items() {
                match item {
                    PositionedLayoutItem::GlyphRun(glyph_run) => {
                        renderer.set_paint(glyph_run.style().brush.color);
                        let run = glyph_run.run();

                        stats.start("fill_glyphs");
                        GlyphRunBuilder::new(
                            run.font().clone(),
                            *renderer.transform(),
                            &mut renderer,
                        )
                        .font_size(run.font_size())
                        .hint(true)
                        .normalized_coords(run.normalized_coords())
                        .bitmap_cache(true)
                        .fill_glyphs(
                            glyph_run
                                .positioned_glyphs()
                                .map(|glyph| parley_draw::Glyph {
                                    id: glyph.id,
                                    x: glyph.x,
                                    y: glyph.y,
                                }),
                            &mut glyph_caches,
                            &mut image_cache,
                        );
                        stats.end("fill_glyphs");

                        let style = glyph_run.style();
                        if let Some(decoration) = &style.underline {
                            let offset =
                                decoration.offset.unwrap_or(run.metrics().underline_offset);
                            let size = decoration.size.unwrap_or(run.metrics().underline_size);

                            stats.start("render_decoration");
                            render_decoration(
                                &mut renderer,
                                &decoration.brush,
                                &glyph_run,
                                offset,
                                size,
                            );
                            stats.end("render_decoration");
                        }
                        if let Some(decoration) = &style.strikethrough {
                            let offset = decoration
                                .offset
                                .unwrap_or(run.metrics().strikethrough_offset);
                            let size = decoration.size.unwrap_or(run.metrics().strikethrough_size);

                            stats.start("render_decoration");
                            render_decoration(
                                &mut renderer,
                                &decoration.brush,
                                &glyph_run,
                                offset,
                                size,
                            );
                            stats.end("render_decoration");
                        }
                    }

                    PositionedLayoutItem::InlineBox(inline_box) => {
                        renderer.set_paint(config.foreground_color);
                        let (x0, y0) = (inline_box.x as f64, inline_box.y as f64);
                        let (x1, y1) =
                            (x0 + inline_box.width as f64, y0 + inline_box.height as f64);
                        renderer.fill_rect(&Rect::new(x0, y0, x1, y1));
                    }
                }
            }
        }

        // Record accumulated totals for this frame
        stats.finish_accumulating("fill_glyphs");
        stats.finish_accumulating("render_decoration");

        let output_path = output_dir(file!()).join("vello_cpu_render.png");
        stats.start("render");
        let (_render_core_time, _io_time) = render(
            &mut renderer,
            &mut glyph_caches,
            &mut image_cache,
            width,
            height,
            &mut glyph_renderer,
            &output_path,
        );
        stats.end("render");

        stats.end("total_frame");

        if frame < num_frames - 1 {
            thread::sleep(frame_delay);
        }
    }

    let wall_clock_time = start_time.elapsed();

    thread::sleep(frame_delay);

    println!("\n=== CPU Render Performance ===");
    stats.print_summary();

    println!("\nOverall:");
    println!("  Frames rendered:   {}", num_frames);
    println!("  Wall clock time:   {:?}", wall_clock_time);
}

/// Create the renderer and glyph caches (once per app or per thread).
fn prepare_rendering(
    width: u16,
    height: u16,
) -> (RenderContext, RenderContext, CpuGlyphCaches, ImageCache) {
    let renderer = RenderContext::new(width, height);
    let atlas_size = (256, 256);
    let image_cache = ImageCache::new_with_config(AtlasConfig {
        initial_atlas_count: 0,
        max_atlases: 1,
        atlas_size: (atlas_size.0 as u32, atlas_size.1 as u32),
        auto_grow: true,
        ..Default::default()
    });
    let glyph_renderer = RenderContext::new(atlas_size.0, atlas_size.1);
    let glyph_caches = CpuGlyphCaches::with_page_size(256, 256);
    (renderer, glyph_renderer, glyph_caches, image_cache)
}

/// Reset render context, clear background, and set transform for the frame.
fn reset_renderer(
    renderer: &mut RenderContext,
    glyph_renderer: &mut RenderContext,
    width: u16,
    height: u16,
    padding: u32,
    background_color: Color,
) {
    renderer.reset();
    glyph_renderer.reset();
    renderer.set_paint(background_color);
    renderer.fill_rect(&Rect::new(0.0, 0.0, width as f64, height as f64));
    renderer.set_transform(Affine::translate(Vec2::new(padding as f64, padding as f64)));
}

fn render_decoration(
    renderer: &mut RenderContext,
    brush: &ColorBrush,
    glyph_run: &GlyphRun<'_, ColorBrush>,
    offset: f32,
    size: f32,
) {
    renderer.set_paint(brush.color);
    let y = glyph_run.baseline() - offset;
    let x = glyph_run.offset();
    let x1 = x + glyph_run.advance();
    let y1 = y + size;
    renderer.fill_rect(&Rect::new(x as f64, y as f64, x1 as f64, y1 as f64));
}

/// Rasterize to pixmap, maintain caches, optionally save debug artifacts, write PNG.
fn render(
    renderer: &mut RenderContext,
    glyph_caches: &mut CpuGlyphCaches,
    image_cache: &mut ImageCache,
    width: u16,
    height: u16,
    glyph_renderer: &mut RenderContext,
    output_path: &Path,
) -> (Duration, Duration) {
    let render_start = Instant::now();

    // Replay deferred outline/COLR atlas commands into the glyph renderer,
    // one recorder per atlas page. A single glyph_renderer is reused for all pages.
    for recorder in glyph_caches.bitmap_cache.take_pending_atlas_commands() {
        glyph_renderer.reset();
        replay_atlas_commands(&recorder.commands, glyph_renderer);
        glyph_renderer.flush();
        if let Some(atlas_pixmap) =
            glyph_caches.bitmap_cache.page_pixmap_mut(recorder.page_index as usize)
        {
            glyph_renderer.render_to_pixmap_region(atlas_pixmap, 0, 0);
        }
    }

    // Process pending bitmap uploads BEFORE registering atlas pages.
    // We need mutable access to the atlas pixmaps, which won't be available
    // after we clone them into the renderer via register_image().
    for upload in glyph_caches.bitmap_cache.take_pending_uploads() {
        let atlas_idx = upload.atlas_slot.page_index as usize;

        let Some(atlas_pixmap) = glyph_caches.bitmap_cache.page_pixmap_mut(atlas_idx) else {
            continue;
        };

        copy_pixmap_to_atlas(
            &upload.pixmap,
            atlas_pixmap,
            upload.atlas_slot.x,
            upload.atlas_slot.y,
            upload.atlas_slot.width,
            upload.atlas_slot.height,
        );
    }

    // Register atlas pages with the render context
    let page_count = glyph_caches.bitmap_cache.page_count();
    for page_index in 0..page_count {
        if let Some(page_pixmap) = glyph_caches.bitmap_cache.page_pixmap(page_index) {
            renderer.register_image(page_pixmap.clone());
        }
    }

    let mut pixmap = Pixmap::new(width, height);
    renderer.render_to_pixmap(&mut pixmap);
    glyph_caches.maintain(image_cache);

    let render_core_time = render_start.elapsed();

    #[cfg(all(debug_assertions, feature = "png"))]
    glyph_caches.save_atlas_pages();

    #[cfg(feature = "debug_glyph_bounds")]
    {
        glyph_caches.bitmap_cache.print_stats();
        println!();
        glyph_caches.bitmap_cache.print_keys_grouped();
    }

    let io_start = Instant::now();
    let png = pixmap.into_png().unwrap();
    std::fs::write(output_path, &png).unwrap();
    let io_time = io_start.elapsed();

    (render_core_time, io_time)
}

/// Copy a pixmap to a region in the atlas.
///
/// This is a utility function for copying bitmap glyph pixels to the atlas
/// after draining pending uploads. Used by the CPU backend example.
pub fn copy_pixmap_to_atlas(
    src: &Pixmap,
    dst: &mut Pixmap,
    dst_x: u16,
    dst_y: u16,
    width: u16,
    height: u16,
) {
    let copy_width = width as usize;
    let copy_height = height as usize;
    let src_stride = src.width() as usize;
    let dst_stride = dst.width() as usize;

    // Use data_as_u8_slice for byte-level access (data() returns &[PremulRgba8])
    let src_data = src.data_as_u8_slice();
    let dst_data = dst.data_as_u8_slice_mut();

    for y in 0..copy_height {
        let src_row_start = y * src_stride * 4;
        let src_row_end = src_row_start + copy_width * 4;
        let dst_row_start = ((dst_y as usize + y) * dst_stride + dst_x as usize) * 4;
        let dst_row_end = dst_row_start + copy_width * 4;

        dst_data[dst_row_start..dst_row_end].copy_from_slice(&src_data[src_row_start..src_row_end]);
    }
}
