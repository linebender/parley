// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, extracts outlines using Skrifa and
//! then paints those outlines using Vello CPU through Parley Draw.

use std::path::Path;
use std::time::Instant;

use parley::{GlyphRun, Layout, PositionedLayoutItem};
use parley_draw::renderers::vello_renderer::replay_atlas_commands;
use parley_draw::{
    AtlasConfig, CpuGlyphCaches, GlyphCache, GlyphCacheConfig, GlyphRunBuilder, ImageCache,
    PendingClearRect,
};
use parley_examples_common::{
    ColorBrush, ExampleConfig, FrameKind, FrameStats, frame_sequence, output_dir, prepare_layouts,
};
use peniko::Color;
use vello_cpu::{
    Pixmap, RenderContext,
    kurbo::{Affine, Rect, Vec2},
};

fn main() {
    let start_time = Instant::now();

    let (
        (simple_layout, simple_width, simple_height, simple_config),
        (rich_layout, rich_width, rich_height, rich_config),
    ) = prepare_layouts();
    let width = simple_width.max(rich_width);
    let height = simple_height.max(rich_height);

    let mut stats = FrameStats::new();

    stats.start("prepare_rendering");
    let (mut renderer, mut glyph_renderer, mut glyph_caches, mut image_cache) =
        prepare_rendering(width, height);
    stats.end("prepare_rendering");
    let output_path = output_dir(file!()).join("vello_cpu_render.png");

    for frame in &frame_sequence() {
        println!("\n=== {} ===", frame.label);
        let (layout, config) = match frame.kind {
            FrameKind::Simple => (&simple_layout, &simple_config),
            FrameKind::Rich => (&rich_layout, &rich_config),
        };
        render_frame(
            layout,
            config,
            &mut renderer,
            &mut glyph_renderer,
            &mut glyph_caches,
            &mut image_cache,
            width,
            height,
            &output_path,
            &mut stats,
        );
    }

    println!("\n=== CPU Render Performance ===");
    stats.print_summary();
    println!("\nTotal elapsed: {:?}", start_time.elapsed());
}

/// Run one full frame: reset → fill glyphs → render → maintain → print stats.
#[expect(
    clippy::too_many_arguments,
    reason = "render orchestration requires many dependencies"
)]
fn render_frame(
    layout: &Layout<ColorBrush>,
    config: &ExampleConfig,
    renderer: &mut RenderContext,
    glyph_renderer: &mut RenderContext,
    glyph_caches: &mut CpuGlyphCaches,
    image_cache: &mut ImageCache,
    width: u16,
    height: u16,
    output_path: &Path,
    stats: &mut FrameStats,
) {
    stats.start("total_frame");

    stats.start("reset_renderer");
    reset_renderer(
        renderer,
        glyph_renderer,
        width,
        height,
        config.padding,
        config.background_color,
    );
    stats.end("reset_renderer");

    stats.start_accumulating("fill_glyphs");
    stats.start_accumulating("render_underline");
    stats.start_accumulating("render_strikethrough");

    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    renderer.set_paint(glyph_run.style().brush.color);
                    let run = glyph_run.run();

                    stats.start("fill_glyphs");
                    GlyphRunBuilder::new(run.font().clone(), *renderer.transform(), renderer)
                        .font_size(run.font_size())
                        .hint(config.hint)
                        .normalized_coords(run.normalized_coords())
                        .atlas_cache(config.use_atlas_cache)
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
                    stats.end("fill_glyphs");

                    let style = glyph_run.style();
                    if let Some(decoration) = &style.underline {
                        let offset = decoration.offset.unwrap_or(run.metrics().underline_offset);
                        let size = decoration.size.unwrap_or(run.metrics().underline_size);

                        stats.start("render_underline");
                        renderer.set_paint(decoration.brush.color);
                        let x = glyph_run.offset();
                        let x1 = x + glyph_run.advance();
                        let baseline = glyph_run.baseline();

                        GlyphRunBuilder::new(run.font().clone(), *renderer.transform(), renderer)
                            .font_size(run.font_size())
                            .normalized_coords(run.normalized_coords())
                            .render_decoration(
                                glyph_run
                                    .positioned_glyphs()
                                    .map(|glyph| parley_draw::Glyph {
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
                        stats.end("render_underline");
                    }
                    if let Some(decoration) = &style.strikethrough {
                        let offset = decoration
                            .offset
                            .unwrap_or(run.metrics().strikethrough_offset);
                        let size = decoration.size.unwrap_or(run.metrics().strikethrough_size);

                        stats.start("render_strikethrough");
                        render_strikethrough(renderer, &decoration.brush, &glyph_run, offset, size);
                        stats.end("render_strikethrough");
                    }
                }

                PositionedLayoutItem::InlineBox(inline_box) => {
                    renderer.set_paint(config.foreground_color);
                    let (x0, y0) = (inline_box.x as f64, inline_box.y as f64);
                    let (x1, y1) = (x0 + inline_box.width as f64, y0 + inline_box.height as f64);
                    renderer.fill_rect(&Rect::new(x0, y0, x1, y1));
                }
            }
        }
    }

    stats.finish_accumulating("fill_glyphs");
    stats.finish_accumulating("render_underline");
    stats.finish_accumulating("render_strikethrough");

    stats.start("render");
    let pixmap = render(
        renderer,
        glyph_caches,
        image_cache,
        width,
        height,
        glyph_renderer,
    );
    stats.end("render");

    save_output(pixmap, output_path);

    stats.end("total_frame");
}

/// Create the renderer and glyph caches (once per app or per thread).
fn prepare_rendering(
    width: u16,
    height: u16,
) -> (RenderContext, RenderContext, CpuGlyphCaches, ImageCache) {
    let atlas_size = (256, 256);
    let renderer = RenderContext::new(width, height);
    let image_cache = ImageCache::new_with_config(AtlasConfig {
        initial_atlas_count: 1,
        max_atlases: 1,
        atlas_size: (atlas_size.0 as u32, atlas_size.1 as u32),
        auto_grow: false,
        ..Default::default()
    });
    let glyph_renderer = RenderContext::new(atlas_size.0, atlas_size.1);
    let glyph_caches = CpuGlyphCaches::with_config(
        256,
        256,
        GlyphCacheConfig {
            max_entry_age: 2,
            eviction_frequency: 2,
        },
    );
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

/// Draw a strikethrough as a simple filled rectangle.
fn render_strikethrough(
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

/// Rasterize to pixmap, maintain caches, and print cache stats.
///
/// The pipeline is: replay atlas commands → upload bitmaps → register atlas
/// pages as images → composite to pixmap → maintain/evict → clear stale slots.
fn render(
    renderer: &mut RenderContext,
    glyph_caches: &mut CpuGlyphCaches,
    image_cache: &mut ImageCache,
    width: u16,
    height: u16,
    glyph_renderer: &mut RenderContext,
) -> Pixmap {
    // Replay outline/COLR draw commands into each atlas page's pixmap.
    for mut recorder in glyph_caches.glyph_atlas.take_pending_atlas_commands() {
        glyph_renderer.reset();
        replay_atlas_commands(&mut recorder.commands, glyph_renderer);
        glyph_renderer.flush();
        if let Some(atlas_pixmap) = glyph_caches
            .glyph_atlas
            .page_pixmap_mut(recorder.page_index as usize)
        {
            glyph_renderer.composite_to_pixmap_at_offset(atlas_pixmap, 0, 0);
        }
    }

    // Bitmap uploads must happen before register_image(): after registration the
    // Arc is shared and page_pixmap_mut() returns None.
    for upload in glyph_caches.glyph_atlas.take_pending_uploads() {
        let page_index = upload.atlas_slot.page_index as usize;

        let Some(atlas_pixmap) = glyph_caches.glyph_atlas.page_pixmap_mut(page_index) else {
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

    // Share atlas page pixmaps with the renderer so glyphs can be composited.
    let page_count = glyph_caches.glyph_atlas.page_count();
    for page_index in 0..page_count {
        if let Some(page_pixmap) = glyph_caches.glyph_atlas.page_pixmap(page_index) {
            renderer.register_image(page_pixmap.clone());
        }
    }

    let mut pixmap = Pixmap::new(width, height);
    renderer.render_to_pixmap(&mut pixmap);
    renderer.clear_images();
    glyph_caches.maintain(image_cache);

    // Clear padded regions for all evicted glyphs so that stale pixel data
    // doesn't bleed through when the slot is reused on a subsequent frame.
    for rect in glyph_caches.glyph_atlas.take_pending_clear_rects() {
        if let Some(atlas_pixmap) = glyph_caches
            .glyph_atlas
            .page_pixmap_mut(rect.page_index as usize)
        {
            clear_pixmap_region(atlas_pixmap, &rect);
        }
    }

    println!(
        "  cache: len={} hits={} misses={}",
        glyph_caches.glyph_atlas.len(),
        glyph_caches.glyph_atlas.cache_hits(),
        glyph_caches.glyph_atlas.cache_misses(),
    );
    glyph_caches.glyph_atlas.clear_stats();

    #[cfg(all(debug_assertions, feature = "png"))]
    glyph_caches.save_atlas_pages();

    pixmap
}

/// Save the rendered pixmap as a PNG file.
fn save_output(pixmap: Pixmap, output_path: &Path) {
    let png = pixmap.into_png().unwrap();
    std::fs::write(output_path, &png).unwrap();
}

/// Zero out a rectangular region in the atlas pixmap.
///
/// Necessary because `composite_to_pixmap_at_offset` uses `SrcOver` blending,
/// so stale pixels from evicted glyphs would bleed through if not cleared.
fn clear_pixmap_region(dst: &mut Pixmap, rect: &PendingClearRect) {
    let dst_stride = dst.width() as usize;
    let dst_data = dst.data_as_u8_slice_mut();
    let clear_width = rect.width as usize;
    let clear_height = rect.height as usize;

    for y in 0..clear_height {
        let row_start = ((rect.y as usize + y) * dst_stride + rect.x as usize) * 4;
        let row_end = row_start + clear_width * 4;
        dst_data[row_start..row_end].fill(0);
    }
}

/// Copy bitmap glyph pixels into a rectangular region of an atlas page.
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
