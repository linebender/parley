// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renders text styled with `styled_text`, lowered through `styled_text_parley`,
//! and painted with Vello CPU.

#![expect(clippy::cast_possible_truncation, reason = "example image sizes")]

use std::path::Path;
use std::sync::Arc;

use glifo::renderers::vello_renderer::replay_atlas_commands;
use glifo::{
    AtlasConfig, CpuGlyphCaches, GlyphCache, GlyphCacheConfig, GlyphRunBuilder, ImageCache,
    PendingClearRect,
};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontWeight, GenericFamily, GlyphRun, Layout,
    LayoutContext, LineHeight, PositionedLayoutItem,
};
use parley_examples_common::{ColorBrush, output_dir};
use peniko::Color;
use styled_text_parley::{
    ParleyLayoutStyle, ParleyPaintStyle, ParleyStyleChange, ParleyStyleRunWorkspace,
    ParleyStyledTextBuilder, build_layout_from_parley_styled_text,
};
use vello_cpu::{
    Pixmap, RenderContext,
    kurbo::{Affine, Rect, Vec2},
};

const TEXT: &str = concat!(
    "StyledText + Parley\n",
    "This example interns local style payloads, stores compact IDs on spans,\n",
    "resolves overlapping ranges with reusable scratch space, and feeds Parley style runs.\n",
    "\n",
    "BIG, tiny, underline, strike, and paint-only color.\n",
    "Some bidirectional text: English العربية.\n",
);

fn main() {
    let display_scale = 1.0;
    let quantize = true;
    let max_advance = Some(560.0 * display_scale);
    let padding = 24;

    let base_layout = ParleyLayoutStyle {
        font_family: GenericFamily::SystemUi.into(),
        font_size: 18.0,
        line_height: LineHeight::FontSizeRelative(1.35),
        ..ParleyLayoutStyle::default()
    };
    let black = ColorBrush {
        color: Color::BLACK,
    };
    let blue = ColorBrush {
        color: Color::from_rgb8(39, 92, 180),
    };
    let red = ColorBrush {
        color: Color::from_rgb8(190, 53, 48),
    };
    let green = ColorBrush {
        color: Color::from_rgb8(36, 130, 83),
    };
    let purple = ColorBrush {
        color: Color::from_rgb8(112, 69, 166),
    };

    let mut styled = ParleyStyledTextBuilder::new(base_layout, ParleyPaintStyle::new(black));
    styled.reserve(TEXT.len(), 8);
    styled.push_with(
        "StyledText + Parley\n",
        ParleyStyleChange::default()
            .font_size(36.0)
            .font_weight(FontWeight::BOLD)
            .underline(true)
            .brush(blue),
    );
    styled.push("This example interns local style payloads, stores compact IDs on spans,\n");
    styled.push(
        "resolves overlapping ranges with reusable scratch space, and feeds Parley style runs.\n",
    );
    styled.push("\n");
    styled.push_with(
        "BIG",
        ParleyStyleChange::default()
            .font_size(32.0)
            .font_weight(FontWeight::BOLD)
            .letter_spacing(1.0)
            .brush(red),
    );
    styled.push(", ");
    let tiny = styled.push_with("tiny", ParleyStyleChange::default().font_size(12.0));
    styled.apply(tiny, ParleyStyleChange::default().brush(purple));
    styled.push(", ");
    styled.push_with(
        "underline",
        ParleyStyleChange::default().underline(true).brush(green),
    );
    styled.push(", ");
    styled.push_with(
        "strike",
        ParleyStyleChange::default()
            .strikethrough(true)
            .brush(purple),
    );
    styled.push(", and ");
    styled.push_with("paint-only color", ParleyStyleChange::default().brush(red));
    styled.push(".\n");
    styled.push("Some bidirectional text: English العربية.\n");
    let styled = styled.finish();

    let mut font_cx = FontContext::new();
    let mut layout_cx = LayoutContext::new();
    let mut workspace = ParleyStyleRunWorkspace::new();

    let mut layout = build_layout_from_parley_styled_text(
        &mut layout_cx,
        &mut font_cx,
        &styled,
        &mut workspace,
        display_scale,
        quantize,
    )
    .expect("example layout should build");

    layout.break_all_lines(max_advance);
    layout.align(Alignment::Start, AlignmentOptions::default());

    let width = layout.width().ceil() as u16 + padding * 2;
    let height = layout.height().ceil() as u16 + padding * 2;
    let output_path =
        output_dir(env!("CARGO_MANIFEST_DIR")).join("vello_cpu_render_styled_text.png");

    render_layout(&layout, width, height, padding, &output_path);
    println!("wrote {}", output_path.display());
}

fn render_layout(
    layout: &Layout<ColorBrush>,
    width: u16,
    height: u16,
    padding: u16,
    output_path: &Path,
) {
    let (mut renderer, mut glyph_renderer, mut glyph_caches, mut image_cache) =
        prepare_rendering(width, height);

    reset_renderer(
        &mut renderer,
        &mut glyph_renderer,
        width,
        height,
        padding,
        Color::from_rgb8(250, 250, 252),
    );

    for line in layout.lines() {
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    render_glyph_run(
                        &mut renderer,
                        &mut glyph_caches,
                        &mut image_cache,
                        &glyph_run,
                    );
                }
                PositionedLayoutItem::InlineBox(inline_box) => {
                    renderer.set_paint(Color::BLACK);
                    let (x0, y0) = (inline_box.x as f64, inline_box.y as f64);
                    let (x1, y1) = (x0 + inline_box.width as f64, y0 + inline_box.height as f64);
                    renderer.fill_rect(&Rect::new(x0, y0, x1, y1));
                }
            }
        }
    }

    let pixmap = render(
        &mut renderer,
        &mut glyph_caches,
        &mut image_cache,
        width,
        height,
        &mut glyph_renderer,
    );
    save_output(pixmap, output_path);
}

fn prepare_rendering(
    width: u16,
    height: u16,
) -> (RenderContext, RenderContext, CpuGlyphCaches, ImageCache) {
    let atlas_size = (256, 256);
    let renderer = RenderContext::new(width, height);
    let image_cache = ImageCache::new_with_config(AtlasConfig {
        initial_atlas_count: 1,
        max_atlases: 1,
        atlas_size: (u32::from(atlas_size.0), u32::from(atlas_size.1)),
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
            max_cached_font_size: 128.0,
        },
    );
    (renderer, glyph_renderer, glyph_caches, image_cache)
}

fn reset_renderer(
    renderer: &mut RenderContext,
    glyph_renderer: &mut RenderContext,
    width: u16,
    height: u16,
    padding: u16,
    background_color: Color,
) {
    renderer.reset();
    glyph_renderer.reset();
    renderer.set_paint(background_color);
    renderer.fill_rect(&Rect::new(0.0, 0.0, width as f64, height as f64));
    renderer.set_transform(Affine::translate(Vec2::new(
        f64::from(padding),
        f64::from(padding),
    )));
}

fn render_glyph_run(
    renderer: &mut RenderContext,
    glyph_caches: &mut CpuGlyphCaches,
    image_cache: &mut ImageCache,
    glyph_run: &GlyphRun<'_, ColorBrush>,
) {
    let run = glyph_run.run();
    let mut run_renderer = GlyphRunBuilder::new(run.font().clone(), *renderer.transform())
        .font_size(run.font_size())
        .hint(true)
        .normalized_coords(run.normalized_coords())
        .atlas_cache(true)
        .build(
            glyph_run.positioned_glyphs().map(|glyph| glifo::Glyph {
                id: glyph.id,
                x: glyph.x,
                y: glyph.y,
            }),
            glyph_caches,
            image_cache,
        );

    renderer.set_paint(glyph_run.style().brush.color);
    run_renderer.fill_glyphs(renderer);

    let style = glyph_run.style();
    if let Some(decoration) = &style.underline {
        let offset = decoration.offset.unwrap_or(run.metrics().underline_offset);
        let size = decoration.size.unwrap_or(run.metrics().underline_size);
        renderer.set_paint(decoration.brush.color);
        let x = glyph_run.offset();
        let x1 = x + glyph_run.advance();
        run_renderer.render_decoration(x..=x1, glyph_run.baseline(), offset, size, 1.0, renderer);
    }
    if let Some(decoration) = &style.strikethrough {
        let offset = decoration
            .offset
            .unwrap_or(run.metrics().strikethrough_offset);
        let size = decoration.size.unwrap_or(run.metrics().strikethrough_size);
        render_strikethrough(renderer, &decoration.brush, glyph_run, offset, size);
    }
}

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

fn render(
    renderer: &mut RenderContext,
    glyph_caches: &mut CpuGlyphCaches,
    image_cache: &mut ImageCache,
    width: u16,
    height: u16,
    glyph_renderer: &mut RenderContext,
) -> Pixmap {
    glyph_caches
        .glyph_atlas
        .replay_pending_atlas_commands_with_pixmaps(|recorder, pixmaps| {
            glyph_renderer.reset();
            replay_atlas_commands(&mut recorder.commands, glyph_renderer);
            glyph_renderer.flush();
            if let Some(atlas_pixmap) = pixmaps
                .get_mut(recorder.page_index as usize)
                .and_then(Arc::get_mut)
            {
                glyph_renderer.composite_to_pixmap_at_offset(atlas_pixmap, 0, 0);
            }
        });

    let uploads: Vec<_> = glyph_caches.glyph_atlas.drain_pending_uploads().collect();
    for upload in uploads {
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

    let clear_rects: Vec<_> = glyph_caches
        .glyph_atlas
        .drain_pending_clear_rects()
        .collect();
    for rect in clear_rects {
        if let Some(atlas_pixmap) = glyph_caches
            .glyph_atlas
            .page_pixmap_mut(rect.page_index as usize)
        {
            clear_pixmap_region(atlas_pixmap, &rect);
        }
    }

    pixmap
}

fn save_output(pixmap: Pixmap, output_path: &Path) {
    let png = pixmap.into_png().unwrap();
    std::fs::write(output_path, &png).unwrap();
}

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

fn copy_pixmap_to_atlas(
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

    for row in 0..copy_height {
        let src_start = row * src_stride * 4;
        let src_end = src_start + copy_width * 4;
        let dst_start = ((usize::from(dst_y) + row) * dst_stride + usize::from(dst_x)) * 4;
        let dst_end = dst_start + copy_width * 4;
        dst_data[dst_start..dst_end].copy_from_slice(&src_data[src_start..src_end]);
    }
}
