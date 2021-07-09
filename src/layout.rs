use piet::kurbo::{Point, Rect, Size};
use piet::{HitTestPoint, HitTestPosition, LineMetric};
use swash::GlyphId;

use super::Layout;

#[derive(Copy, Clone, Default, Debug)]
pub struct Glyph {
    pub id: GlyphId,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

impl piet::TextLayout for Layout {
    fn size(&self) -> Size {
        Size::default()
    }

    fn trailing_whitespace_width(&self) -> f64 {
        0.
    }

    fn image_bounds(&self) -> Rect {
        Rect::default()
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        if line_number == 0 {
            Some(&self.text)
        } else {
            None
        }
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        None
    }

    fn line_count(&self) -> usize {
        0
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        HitTestPoint::default()
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        HitTestPosition::default()
    }
}
