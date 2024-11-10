use parley::GlyphRun;
use peniko::kurbo::Rect as KurboRect;
use peniko::Color as PenikoColor;
use skrifa::{
    instance::{LocationRef, NormalizedCoord, Size},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef as ReadFontsRef,
    GlyphId, MetadataProvider, OutlineGlyph,
};
use tiny_skia::{
    Color as TinySkiaColor, FillRule, Paint, PathBuilder, PixmapMut, Rect as TinySkiaRect,
    Transform,
};

pub fn to_tiny_skia_color(color: PenikoColor) -> TinySkiaColor {
    TinySkiaColor::from_rgba8(color.r, color.g, color.b, color.a)
}

pub fn to_tiny_skia_rect(rect: KurboRect) -> TinySkiaRect {
    TinySkiaRect::from_ltrb(rect.x0 as _, rect.y0 as _, rect.x1 as _, rect.y1 as _).unwrap()
}

pub fn fill_rect(
    pixmap: &mut PixmapMut,
    rect: KurboRect,
    color: PenikoColor,
    transform: Transform,
) {
    let rect = to_tiny_skia_rect(rect);
    let mut paint = Paint::default();
    paint.set_color(to_tiny_skia_color(color));
    pixmap.fill_rect(rect, &paint, transform, None);
}

pub fn render_glyph_run(glyph_run: &GlyphRun<PenikoColor>, pen: &mut TinySkiaPen<'_, '_>) {
    // Resolve properties of the GlyphRun
    let mut run_x = glyph_run.offset();
    let run_y = glyph_run.baseline();

    // Get the "Run" from the "GlyphRun"
    let run = glyph_run.run();

    // Resolve properties of the Run
    let font = run.font();
    let font_size = run.font_size();

    let normalized_coords = run
        .normalized_coords()
        .iter()
        .map(|coord| NormalizedCoord::from_bits(*coord))
        .collect::<Vec<_>>();

    // Get glyph outlines using Skrifa. This can be cached in production code.
    let font_collection_ref = font.data.as_ref();
    let font_ref = ReadFontsRef::from_index(font_collection_ref, font.index).unwrap();
    let outlines = font_ref.outline_glyphs();

    // Iterates over the glyphs in the GlyphRun
    for glyph in glyph_run.glyphs() {
        let glyph_x = run_x + glyph.x;
        let glyph_y = run_y - glyph.y;
        run_x += glyph.advance;

        let glyph_id = GlyphId::from(glyph.id);
        let glyph_outline = outlines.get(glyph_id).unwrap();

        pen.set_origin(glyph_x, glyph_y);
        pen.draw_glyph(&glyph_outline, font_size, &normalized_coords);
    }
}

pub struct TinySkiaPen<'a, 'b> {
    pixmap: &'b mut PixmapMut<'a>,
    x: f32,
    y: f32,
    paint: Paint<'static>,
    open_path: PathBuilder,
    transform: Transform,
}

impl<'a, 'b> TinySkiaPen<'a, 'b> {
    pub fn new(pixmap: &'b mut PixmapMut<'a>, transform: Transform) -> Self {
        Self {
            pixmap,
            x: 0.0,
            y: 0.0,
            paint: Paint::default(),
            open_path: PathBuilder::new(),
            transform,
        }
    }

    fn set_origin(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    pub fn set_color(&mut self, color: PenikoColor) {
        self.paint.set_color(to_tiny_skia_color(color));
    }

    fn draw_glyph(
        &mut self,
        glyph: &OutlineGlyph<'_>,
        size: f32,
        normalized_coords: &[NormalizedCoord],
    ) {
        let location_ref = LocationRef::new(normalized_coords);
        let settings = DrawSettings::unhinted(Size::new(size), location_ref);
        glyph.draw(settings, self).unwrap();

        let builder = core::mem::replace(&mut self.open_path, PathBuilder::new());
        if let Some(path) = builder.finish() {
            self.pixmap
                .fill_path(&path, &self.paint, FillRule::Winding, self.transform, None);
        }
    }
}

impl OutlinePen for TinySkiaPen<'_, '_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.open_path.move_to(self.x + x, self.y - y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.open_path.line_to(self.x + x, self.y - y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.open_path
            .quad_to(self.x + cx0, self.y - cy0, self.x + x, self.y - y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.open_path.cubic_to(
            self.x + cx0,
            self.y - cy0,
            self.x + cx1,
            self.y - cy1,
            self.x + x,
            self.y - y,
        );
    }

    fn close(&mut self) {
        self.open_path.close();
    }
}
