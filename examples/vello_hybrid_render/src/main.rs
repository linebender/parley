// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, extracts outlines using Skrifa and
//! then paints those outlines using Vello Hybrid (CPU/GPU) through Parley Draw.

use std::io::BufWriter;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use parley::{GlyphRun, PositionedLayoutItem};
use parley_draw::renderers::vello_renderer::replay_atlas_commands;
use parley_draw::{GlyphCache, GlyphRunBuilder, GpuGlyphCaches};
use parley_examples_common::{ColorBrush, FrameStats, output_dir, prepare_example_layout};
use peniko::Color;
use vello_common::kurbo::{Affine, Rect, Vec2};
use vello_common::pixmap::Pixmap;
use vello_hybrid::{AtlasId, RenderSize, RenderTargetConfig, Renderer, Scene};

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let start_time = Instant::now();

    let (layout, width, height, config) = prepare_example_layout();

    let mut stats = FrameStats::new();

    stats.start("prepare_rendering");
    let HybridRendering {
        device,
        queue,
        texture,
        texture_view,
        mut renderer,
        mut scene,
        mut glyph_renderer,
        mut glyph_caches,
    } = prepare_rendering(width, height).await;
    stats.end("prepare_rendering");

    let num_frames = 5;
    let frame_delay = Duration::from_millis(100);

    for frame in 0..num_frames {
        stats.start("total_frame");
        println!("Rendering frame {}/{}", frame + 1, num_frames);

        stats.start("reset_renderer");
        let current_transform = reset_renderer(
            &mut scene,
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

        for line in layout.lines() {
            for item in line.items() {
                match item {
                    PositionedLayoutItem::GlyphRun(glyph_run) => {
                        scene.set_paint(glyph_run.style().brush.color);
                        let run = glyph_run.run();

                        stats.start("fill_glyphs");
                        GlyphRunBuilder::new(run.font().clone(), current_transform, &mut scene)
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
                                &mut renderer.image_cache,
                            );
                        stats.end("fill_glyphs");

                        let style = glyph_run.style();
                        if let Some(decoration) = &style.underline {
                            let offset =
                                decoration.offset.unwrap_or(run.metrics().underline_offset);
                            let size = decoration.size.unwrap_or(run.metrics().underline_size);

                            stats.start("render_decoration");
                            render_decoration(
                                &mut scene,
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
                                &mut scene,
                                &decoration.brush,
                                &glyph_run,
                                offset,
                                size,
                            );
                            stats.end("render_decoration");
                        }
                    }

                    PositionedLayoutItem::InlineBox(inline_box) => {
                        // Restore content transform: glyph drawing sets per-glyph transform and
                        // leaves the scene with the last glyph's transform; inline_box coords
                        // are in layout space, so we need the content transform for the rect.
                        scene.set_transform(current_transform);
                        scene.set_paint(config.foreground_color);
                        let (x0, y0) = (inline_box.x as f64, inline_box.y as f64);
                        let (x1, y1) =
                            (x0 + inline_box.width as f64, y0 + inline_box.height as f64);
                        scene.fill_rect(&Rect::new(x0, y0, x1, y1));
                    }
                }
            }
        }

        // Record accumulated totals for this frame
        stats.finish_accumulating("fill_glyphs");
        stats.finish_accumulating("render_decoration");

        let output_path = output_dir(file!()).join("vello_hybrid_render.png");
        stats.start("render");
        let (_render_core_time, _io_time) = render(
            &device,
            &queue,
            &texture,
            &texture_view,
            &mut renderer,
            &mut glyph_renderer,
            &scene,
            width,
            height,
            &mut glyph_caches,
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

    println!("\n=== Hybrid Render Performance ===");
    stats.print_summary();

    println!("\nOverall:");
    println!("  Frames rendered:   {}", num_frames);
    println!("  Wall clock time:   {:?}", wall_clock_time);
}

/// GPU device, render target, and scene/caches (once per app).
struct HybridRendering {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    renderer: Renderer,
    scene: Scene,
    glyph_renderer: Scene,
    glyph_caches: GpuGlyphCaches,
}

/// Create the hybrid renderer, scene, and glyph caches (once per app).
async fn prepare_rendering(width: u16, height: u16) -> HybridRendering {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .expect("Failed to find an appropriate adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .expect("Failed to create device");

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Render Target"),
        size: wgpu::Extent3d {
            width: width.into(),
            height: height.into(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let renderer = Renderer::new(
        &device,
        &RenderTargetConfig {
            format: texture.format(),
            width: width.into(),
            height: height.into(),
        },
    );

    let scene = Scene::new(width, height);

    // Size the glyph renderer to match the GPU atlas page dimensions.
    // rasterize_glyph_to_atlas positions glyphs at atlas coordinates,
    // so the scene must cover the full atlas page.
    let (atlas_w, atlas_h) = renderer.image_cache.atlas_manager().config().atlas_size;
    let glyph_renderer = Scene::new(atlas_w as u16, atlas_h as u16);
    let glyph_caches = GpuGlyphCaches::new();

    HybridRendering {
        device,
        queue,
        texture,
        texture_view,
        renderer,
        scene,
        glyph_renderer,
        glyph_caches,
    }
}

/// Reset scene, clear background, set transform for the frame; returns the current transform.
fn reset_renderer(
    scene: &mut Scene,
    glyph_renderer: &mut Scene,
    width: u16,
    height: u16,
    padding: u32,
    background_color: Color,
) -> Affine {
    scene.reset();
    glyph_renderer.reset();
    scene.set_paint(background_color);
    scene.fill_rect(&Rect::new(0.0, 0.0, width as f64, height as f64));
    let current_transform = Affine::translate(Vec2::new(padding as f64, padding as f64));
    scene.set_transform(current_transform);
    current_transform
}

fn render_decoration(
    scene: &mut Scene,
    brush: &ColorBrush,
    glyph_run: &GlyphRun<'_, ColorBrush>,
    offset: f32,
    size: f32,
) {
    scene.set_paint(brush.color);
    let y = glyph_run.baseline() - offset;
    let x = glyph_run.offset();
    let x1 = x + glyph_run.advance();
    let y1 = y + size;
    scene.fill_rect(&Rect::new(x as f64, y as f64, x1 as f64, y1 as f64));
}

/// Submit scene to GPU, copy texture to buffer, build pixmap, maintain caches, write PNG.
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    texture_view: &wgpu::TextureView,
    renderer: &mut Renderer,
    glyph_renderer: &mut Scene,
    scene: &Scene,
    width: u16,
    height: u16,
    glyph_caches: &mut GpuGlyphCaches,
    output_path: &Path,
) -> (Duration, Duration) {
    let render_start = Instant::now();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Vello Hybrid Render"),
    });

    // Replay deferred outline/COLR atlas commands into the glyph renderer,
    // one recorder per atlas page. A single glyph_renderer is reused for all pages.
    for recorder in glyph_caches.bitmap_cache.take_pending_atlas_commands() {
        glyph_renderer.reset();
        replay_atlas_commands(&recorder.commands, glyph_renderer);
        renderer
            .render_to_atlas(glyph_renderer, device, queue, AtlasId::new(recorder.page_index))
            .expect("Failed to render glyphs to atlas");
    }

    // Upload any bitmap glyphs that were newly cached this frame.
    // Bitmap pixmaps can't go through render_to_atlas (which rasterises Scene
    // commands), so we write them directly to the atlas texture.
    for upload in glyph_caches.bitmap_cache.take_pending_uploads() {
        renderer.write_to_atlas(
            device,
            queue,
            &mut encoder,
            upload.image_id,
            &*upload.pixmap,
        );
    }

    // Render main scene (references cached glyphs via ImageId)
    renderer
        .render(
            scene,
            device,
            queue,
            &mut encoder,
            &RenderSize {
                width: width.into(),
                height: height.into(),
            },
            texture_view,
        )
        .expect("Failed to render scene");
    glyph_caches.maintain(&mut renderer.image_cache);

    let render_core_time = render_start.elapsed();

    let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);
    let texture_copy_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: u64::from(bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &texture_copy_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: width.into(),
            height: height.into(),
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    texture_copy_buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            if result.is_err() {
                panic!("Failed to map texture for reading");
            }
        });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    let mut img_data = Vec::with_capacity(usize::from(width) * usize::from(height));
    for row in texture_copy_buffer
        .slice(..)
        .get_mapped_range()
        .chunks_exact(bytes_per_row as usize)
    {
        img_data.extend_from_slice(bytemuck::cast_slice(&row[0..usize::from(width) * 4]));
    }
    texture_copy_buffer.unmap();

    let pixmap = Pixmap::from_parts(img_data, width, height);

    let io_start = Instant::now();
    let file = std::fs::File::create(output_path).unwrap();
    let w = BufWriter::new(file);
    let mut png_encoder = png::Encoder::new(w, width.into(), height.into());
    png_encoder.set_color(png::ColorType::Rgba);
    let mut writer = png_encoder.write_header().unwrap();
    writer
        .write_image_data(bytemuck::cast_slice(&pixmap.take_unpremultiplied()))
        .unwrap();
    let io_time = io_start.elapsed();

    (render_core_time, io_time)
}
