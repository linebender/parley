// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Glyph renderer implementation using Vello CPU.

use alloc::sync::Arc;

use crate::{kurbo, peniko};
use peniko::color::{AlphaColor, Srgb};
use vello_cpu::{
    Image, ImageSource, PaintType, Pixmap, RenderContext, RenderSettings,
    color::palette::css::BLACK,
};

use crate::{
    colr::{ColrPainter, ColrRenderer},
    glyph::{GlyphRenderer, GlyphType, PreparedGlyph},
};

impl GlyphRenderer for RenderContext {
    fn fill_glyph(&mut self, prepared_glyph: PreparedGlyph<'_>) {
        match prepared_glyph.glyph_type {
            GlyphType::Outline(glyph) => {
                let old_transform = *self.transform();
                self.set_transform(prepared_glyph.transform);
                self.fill_path(glyph.path);
                self.set_transform(old_transform);
            }
            GlyphType::Bitmap(glyph) => {
                // We need to change the state of the render context
                // to render the bitmap, but don't want to pollute the context,
                // so simulate a `save` and `restore` operation.

                use crate::peniko::ImageSampler;
                let old_transform = *self.transform();
                let old_paint = self.paint().clone();

                // If we scale down by a large factor, fall back to cubic scaling.
                let quality = if prepared_glyph.transform.as_coeffs()[0] < 0.5
                    || prepared_glyph.transform.as_coeffs()[3] < 0.5
                {
                    peniko::ImageQuality::High
                } else {
                    peniko::ImageQuality::Medium
                };

                let image = Image {
                    image: ImageSource::Pixmap(Arc::new(glyph.pixmap)),
                    sampler: ImageSampler {
                        x_extend: peniko::Extend::Pad,
                        y_extend: peniko::Extend::Pad,
                        quality,
                        alpha: 1.0,
                    },
                };

                self.set_paint(image);
                self.set_transform(prepared_glyph.transform);
                self.fill_rect(&glyph.area);

                // Restore the state.
                self.set_paint(old_paint);
                self.set_transform(old_transform);
            }
            GlyphType::Colr(glyph) => {
                // Same as for bitmap glyphs, save the state and restore it later on.

                use crate::peniko::ImageSampler;
                let old_transform = *self.transform();
                let old_paint = self.paint().clone();
                let context_color = match old_paint {
                    PaintType::Solid(s) => s,
                    _ => BLACK,
                };

                let area = glyph.area;

                let glyph_pixmap = {
                    let settings = RenderSettings {
                        num_threads: 0,
                        ..*self.render_settings()
                    };

                    let mut ctx = Self::new_with(glyph.pix_width, glyph.pix_height, settings);
                    let mut pix = Pixmap::new(glyph.pix_width, glyph.pix_height);

                    let mut colr_painter = ColrPainter::new(glyph, context_color, &mut ctx);
                    colr_painter.paint();

                    // Technically not necessary since we always render single-threaded, but just
                    // to be safe.
                    ctx.flush();
                    ctx.render_to_pixmap(&mut pix);

                    pix
                };

                let image = Image {
                    image: ImageSource::Pixmap(Arc::new(glyph_pixmap)),
                    sampler: ImageSampler {
                        x_extend: peniko::Extend::Pad,
                        y_extend: peniko::Extend::Pad,
                        // Since the pixmap will already have the correct size, no need to
                        // use a different image quality here.
                        quality: peniko::ImageQuality::Low,
                        alpha: 1.0,
                    },
                };

                self.set_paint(image);
                self.set_transform(prepared_glyph.transform);
                self.fill_rect(&area);

                // Restore the state.
                self.set_paint(old_paint);
                self.set_transform(old_transform);
            }
        }
    }

    fn stroke_glyph(&mut self, prepared_glyph: PreparedGlyph<'_>) {
        match prepared_glyph.glyph_type {
            GlyphType::Outline(glyph) => {
                let old_transform = *self.transform();
                self.set_transform(prepared_glyph.transform);
                self.fill_path(glyph.path);
                self.set_transform(old_transform);
            }
            GlyphType::Bitmap(_) | GlyphType::Colr(_) => {
                // The definitions of COLR and bitmap glyphs can't meaningfully support being stroked.
                // (COLR's imaging model only has fills)
                self.fill_glyph(prepared_glyph);
            }
        }
    }
}

impl ColrRenderer for RenderContext {
    fn push_clip_layer(&mut self, clip: &kurbo::BezPath) {
        Self::push_clip_layer(self, clip);
    }

    fn push_blend_layer(&mut self, blend_mode: peniko::BlendMode) {
        Self::push_blend_layer(self, blend_mode);
    }

    fn fill_solid(&mut self, color: AlphaColor<Srgb>) {
        self.set_paint(color);
        self.fill_rect(&kurbo::Rect::new(
            0.0,
            0.0,
            f64::from(self.width()),
            f64::from(self.height()),
        ));
    }

    fn fill_gradient(&mut self, gradient: peniko::Gradient) {
        self.set_paint(gradient);
        self.fill_rect(&kurbo::Rect::new(
            0.0,
            0.0,
            f64::from(self.width()),
            f64::from(self.height()),
        ));
    }

    fn set_paint_transform(&mut self, affine: kurbo::Affine) {
        Self::set_paint_transform(self, affine);
    }

    fn pop_layer(&mut self) {
        Self::pop_layer(self);
    }
}
