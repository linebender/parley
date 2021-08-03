use swash::GlyphId;

use super::Layout;

#[derive(Copy, Clone, Default, Debug)]
pub struct Glyph {
    pub id: GlyphId,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}
