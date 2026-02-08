// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A simple example that lays out some text using Parley, extracts outlines using Skrifa and
//! then paints those outlines using Vello Hybrid (CPU/GPU) through Parley Draw.

use std::io::BufWriter;
use std::path::Path;
use std::time::Instant;

use parley::{GlyphRun, Layout, PositionedLayoutItem};
use parley_draw::renderers::vello_renderer::replay_atlas_commands;
use parley_draw::{
    GLYPH_PADDING, GlyphCache, GlyphCacheConfig, GlyphRunBuilder, GpuGlyphCaches, PendingClearRect,
};
use parley_examples_common::{
    ColorBrush, ExampleConfig, FrameKind, FrameStats, frame_sequence, output_dir, prepare_layouts,
};
use peniko::Color;
use vello_common::kurbo::{Affine, Rect, Vec2};
use vello_common::pixmap::Pixmap;
use vello_hybrid::{
    AtlasConfig, AtlasId, RenderSettings, RenderSize, RenderTargetConfig, Renderer, Scene,
};

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let start_time = Instant::now();

    let (
        (simple_layout, simple_width, simple_height, simple_config),
        (rich_layout, rich_width, rich_height, rich_config),
    ) = prepare_layouts();
    let width = simple_width.max(rich_width);
    let height = simple_height.max(rich_height);

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
    let output_path = output_dir(env!("CARGO_MANIFEST_DIR")).join("vello_hybrid_render.png");

    for frame in &frame_sequence() {
        println!("\n=== {} ===", frame.label);
        let (layout, config) = match frame.kind {
            FrameKind::Simple => (&simple_layout, &simple_config),
            FrameKind::Rich => (&rich_layout, &rich_config),
        };
        render_frame(
            layout,
            config,
            &device,
            &queue,
            &texture,
            &texture_view,
            &mut renderer,
            &mut scene,
            &mut glyph_renderer,
            &mut glyph_caches,
            width,
            height,
            &output_path,
            &mut stats,
        );
    }

    println!("\n=== Hybrid Render Performance ===");
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
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    texture_view: &wgpu::TextureView,
    renderer: &mut Renderer,
    scene: &mut Scene,
    glyph_renderer: &mut Scene,
    glyph_caches: &mut GpuGlyphCaches,
    width: u16,
    height: u16,
    output_path: &Path,
    stats: &mut FrameStats,
) {
    stats.start("total_frame");

    stats.start("reset_renderer");
    let current_transform = reset_renderer(
        scene,
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
                    scene.set_paint(glyph_run.style().brush.color);
                    let run = glyph_run.run();

                    let mut run_renderer =
                        GlyphRunBuilder::new(run.font().clone(), current_transform)
                            .font_size(run.font_size())
                            .hint(config.hint)
                            .normalized_coords(run.normalized_coords())
                            .atlas_cache(config.use_atlas_cache)
                            .build(
                                glyph_run
                                    .positioned_glyphs()
                                    .map(|glyph| parley_draw::Glyph {
                                        id: glyph.id,
                                        x: glyph.x,
                                        y: glyph.y,
                                    }),
                                glyph_caches,
                                &mut renderer.image_cache,
                            );
                    stats.start("fill_glyphs");
                    run_renderer.fill_glyphs(scene);
                    stats.end("fill_glyphs");

                    let style = glyph_run.style();
                    if let Some(decoration) = &style.underline {
                        let offset = decoration.offset.unwrap_or(run.metrics().underline_offset);
                        let size = decoration.size.unwrap_or(run.metrics().underline_size);

                        stats.start("render_underline");
                        scene.set_paint(decoration.brush.color);
                        let x = glyph_run.offset();
                        let x1 = x + glyph_run.advance();
                        let baseline = glyph_run.baseline();

                        run_renderer.render_decoration(
                            x..=x1,
                            baseline,
                            offset,
                            size,
                            1.0, // buffer around exclusions
                            scene,
                        );
                        stats.end("render_underline");
                    }
                    if let Some(decoration) = &style.strikethrough {
                        let offset = decoration
                            .offset
                            .unwrap_or(run.metrics().strikethrough_offset);
                        let size = decoration.size.unwrap_or(run.metrics().strikethrough_size);

                        stats.start("render_strikethrough");
                        render_strikethrough(scene, &decoration.brush, &glyph_run, offset, size);
                        stats.end("render_strikethrough");
                    }
                }

                PositionedLayoutItem::InlineBox(inline_box) => {
                    // Restore content transform: glyph drawing sets per-glyph transform and
                    // leaves the scene with the last glyph's transform; inline_box coords
                    // are in layout space, so we need the content transform for the rect.
                    scene.set_transform(current_transform);
                    scene.set_paint(config.foreground_color);
                    let (x0, y0) = (inline_box.x as f64, inline_box.y as f64);
                    let (x1, y1) = (x0 + inline_box.width as f64, y0 + inline_box.height as f64);
                    scene.fill_rect(&Rect::new(x0, y0, x1, y1));
                }
            }
        }
    }

    stats.finish_accumulating("fill_glyphs");
    stats.finish_accumulating("render_underline");
    stats.finish_accumulating("render_strikethrough");

    stats.start("render");
    render(
        device,
        queue,
        texture_view,
        renderer,
        glyph_renderer,
        scene,
        width,
        height,
        glyph_caches,
    );
    stats.end("render");

    save_output(device, queue, texture, width, height, output_path);

    #[cfg(all(debug_assertions, feature = "png"))]
    save_atlas_pages(device, queue, renderer);

    stats.end("total_frame");
}

/// Long-lived GPU state created once per application lifetime.
struct HybridRendering {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Off-screen render target for the final composited frame.
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    renderer: Renderer,
    /// Main scene that accumulates draw commands for the visible frame.
    scene: Scene,
    /// Scratch scene reused for rasterizing glyphs into atlas pages.
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

    let atlas_size = (256, 256);
    let renderer = Renderer::new_with(
        &device,
        &RenderTargetConfig {
            format: texture.format(),
            width: width.into(),
            height: height.into(),
        },
        RenderSettings {
            atlas_config: AtlasConfig {
                initial_atlas_count: 1,
                max_atlases: 1,
                atlas_size: (atlas_size.0 as u32, atlas_size.1 as u32),
                auto_grow: false,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let scene = Scene::new(width, height);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "atlas dimensions fit in u16"
    )]
    let glyph_renderer = Scene::new(atlas_size.0 as u16, atlas_size.1 as u16);
    let glyph_caches = GpuGlyphCaches::with_config(GlyphCacheConfig {
        max_entry_age: 2,
        eviction_frequency: 2,
        max_cached_font_size: 128.0,
    });

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

/// Draw a strikethrough as a simple filled rectangle.
fn render_strikethrough(
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

/// Submit scene to GPU, maintain caches, and print cache stats.
///
/// The pipeline is: replay atlas commands → upload bitmaps → render scene →
/// submit → maintain/evict → clear stale atlas slots.
#[expect(
    clippy::too_many_arguments,
    reason = "render orchestration requires many dependencies"
)]
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture_view: &wgpu::TextureView,
    renderer: &mut Renderer,
    glyph_renderer: &mut Scene,
    scene: &Scene,
    width: u16,
    height: u16,
    glyph_caches: &mut GpuGlyphCaches,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Vello Hybrid Render"),
    });

    // Replay outline/COLR draw commands into each atlas page via the GPU.
    glyph_caches
        .glyph_atlas
        .replay_pending_atlas_commands(|recorder| {
            glyph_renderer.reset();
            replay_atlas_commands(&mut recorder.commands, glyph_renderer);
            renderer
                .render_to_atlas(
                    glyph_renderer,
                    device,
                    queue,
                    AtlasId::new(recorder.page_index),
                )
                .expect("Failed to render glyphs to atlas");
        });

    // Upload bitmap glyphs to the GPU atlas. The write offset is
    // allocation_origin + GLYPH_PADDING so the bitmap sits inside its
    // padded slot, matching the CPU backend's placement.
    let padding = u32::from(GLYPH_PADDING);
    for upload in glyph_caches.glyph_atlas.drain_pending_uploads() {
        let resource = renderer
            .image_cache
            .get(upload.image_id)
            .expect("Bitmap image not found in cache");
        let dst_x = resource.offset[0] as u32 + padding;
        let dst_y = resource.offset[1] as u32 + padding;
        renderer.write_to_atlas(
            device,
            queue,
            &mut encoder,
            upload.image_id,
            &upload.pixmap,
            Some([dst_x, dst_y]),
        );
    }

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

    queue.submit([encoder.finish()]);

    glyph_caches.maintain(&mut renderer.image_cache);

    // Clear padded regions for all evicted glyphs so that stale pixel data
    // doesn't bleed through when the slot is reused on a subsequent frame.
    clear_atlas_regions(
        queue,
        renderer,
        glyph_caches.glyph_atlas.drain_pending_clear_rects(),
    );

    println!(
        "  cache: len={} hits={} misses={}",
        glyph_caches.glyph_atlas.len(),
        glyph_caches.glyph_atlas.cache_hits(),
        glyph_caches.glyph_atlas.cache_misses(),
    );
    glyph_caches.glyph_atlas.clear_stats();
}

/// Zero out atlas regions on the GPU after eviction.
///
/// Uses `queue.write_texture` to write transparent pixels to each clear rect,
/// preventing stale data from evicted glyphs from bleeding through when the
/// slot is reused on a subsequent frame.
///
// TODO: Add Vello Hybrid's GPU support for clearing atlas regions.
fn clear_atlas_regions(
    queue: &wgpu::Queue,
    renderer: &Renderer,
    rects: impl Iterator<Item = PendingClearRect>,
) {
    let atlas_texture = renderer.atlas_texture();
    let mut zeroed: Vec<u8> = Vec::new();

    for rect in rects {
        let byte_count = rect.width as usize * rect.height as usize * 4;
        zeroed.resize(byte_count, 0);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: rect.x as u32,
                    y: rect.y as u32,
                    z: rect.page_index,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &zeroed[..byte_count],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rect.width as u32 * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: rect.width as u32,
                height: rect.height as u32,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// Read back the rendered texture from the GPU and save it as a PNG file.
fn save_output(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u16,
    height: u16,
    output_path: &Path,
) {
    let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);
    let texture_copy_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: u64::from(bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Save Output"),
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

    let file = std::fs::File::create(output_path).unwrap();
    let w = BufWriter::new(file);
    let mut png_encoder = png::Encoder::new(w, width.into(), height.into());
    png_encoder.set_color(png::ColorType::Rgba);
    let mut writer = png_encoder.write_header().unwrap();
    writer
        .write_image_data(bytemuck::cast_slice(&pixmap.take_unpremultiplied()))
        .unwrap();
}

/// Read back atlas pages from the GPU and save them as PNG files for debugging.
#[cfg(all(debug_assertions, feature = "png"))]
#[expect(
    clippy::cast_possible_truncation,
    reason = "atlas dimensions are well within u16 range"
)]
fn save_atlas_pages(device: &wgpu::Device, queue: &wgpu::Queue, renderer: &Renderer) {
    let atlas_texture = renderer.atlas_texture();
    let size = atlas_texture.size();
    let atlas_width = size.width as u16;
    let atlas_height = size.height as u16;
    let layer_count = size.depth_or_array_layers;

    let output_dir = output_dir(env!("CARGO_MANIFEST_DIR"));
    let _ = std::fs::create_dir_all(&output_dir);

    let bytes_per_row = (u32::from(atlas_width) * 4).next_multiple_of(256);

    for layer in 0..layer_count {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Atlas Readback"),
            size: u64::from(bytes_per_row) * u64::from(atlas_height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Atlas Page Readback"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: atlas_width.into(),
                height: atlas_height.into(),
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);

        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_err() {
                    panic!("Failed to map atlas readback buffer");
                }
            });
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

        let mut img_data = Vec::with_capacity(usize::from(atlas_width) * usize::from(atlas_height));
        for row in buffer
            .slice(..)
            .get_mapped_range()
            .chunks_exact(bytes_per_row as usize)
        {
            img_data.extend_from_slice(bytemuck::cast_slice(&row[..usize::from(atlas_width) * 4]));
        }
        buffer.unmap();

        let pixmap = Pixmap::from_parts(img_data, atlas_width, atlas_height);

        let path = output_dir.join(format!("vello_hybrid_atlas_page_{layer}.png"));
        let file = std::fs::File::create(&path).unwrap();
        let w = BufWriter::new(file);
        let mut png_encoder = png::Encoder::new(w, atlas_width.into(), atlas_height.into());
        png_encoder.set_color(png::ColorType::Rgba);
        let mut writer = png_encoder.write_header().unwrap();
        writer
            .write_image_data(bytemuck::cast_slice(&pixmap.take_unpremultiplied()))
            .unwrap();
    }
}
