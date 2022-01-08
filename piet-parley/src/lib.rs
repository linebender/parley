use parley::context::RangedBuilder;
use parley::layout::Cursor;
use parley::style::Brush;
use parley::*;
use piet::kurbo::{Point, Rect, Size};
use piet::*;

use std::cell::RefCell;
use std::ops::RangeBounds;
use std::rc::Rc;

pub use parley;
pub use parley::swash;

impl Brush for ParleyBrush {}

#[derive(Clone, PartialEq, Debug)]
pub struct ParleyBrush(pub Color);

impl Default for ParleyBrush {
    fn default() -> Self {
        Self(Color::grey(0.0))
    }
}

#[derive(Clone)]
pub struct ParleyTextLayout {
    pub text: ParleyTextStorage,
    pub layout: Layout<ParleyBrush>,
}

impl TextLayout for ParleyTextLayout {
    fn size(&self) -> Size {
        self.image_bounds().size()
    }

    fn image_bounds(&self) -> Rect {
        Rect::new(0., 0., self.layout.width() as _, self.layout.height() as _)
    }

    fn text(&self) -> &str {
        self.text.0.as_str()
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        let range = self.layout.get(line_number)?.text_range();
        self.text().get(range)
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        let line = self.layout.get(line_number)?;
        let range = line.text_range();
        let trailing_whitespace = self
            .line_text(line_number)?
            .chars()
            .rev()
            .take_while(|ch| ch.is_whitespace())
            .map(|ch| ch.len_utf8())
            .sum();
        let metrics = line.metrics();
        let y_offset =
            metrics.baseline as f64 - metrics.ascent as f64 - metrics.leading as f64 * 0.5;
        let baseline = metrics.baseline as f64 - y_offset;
        Some(LineMetric {
            start_offset: range.start,
            end_offset: range.end,
            trailing_whitespace,
            baseline,
            height: metrics.size() as f64,
            y_offset,
        })
    }

    fn line_count(&self) -> usize {
        self.layout.len()
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        let cursor = Cursor::from_point(&self.layout, point.x as f32, point.y as f32);
        let mut result = HitTestPoint::default();
        let range = cursor.text_range();
        // FIXME: this is horribly broken for BiDi text
        if cursor.is_trailing() {
            result.idx = range.end;
        } else {
            result.idx = range.start;
        }
        result.is_inside = cursor.is_inside();
        result
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        let cursor = Cursor::from_position(&self.layout, idx, true);
        let mut result = HitTestPosition::default();
        result.point = Point::new(cursor.offset() as f64, cursor.baseline() as f64);
        result.line = cursor.path().line_index;
        result
    }

    fn trailing_whitespace_width(&self) -> f64 {
        // TODO:
        0.0
    }
}

pub struct ParleyTextLayoutBuilder {
    text: ParleyTextStorage,
    builder: RangedBuilder<'static, ParleyBrush, ParleyTextStorage>,
    max_width: f64,
    alignment: layout::Alignment,
}

impl TextLayoutBuilder for ParleyTextLayoutBuilder {
    type Out = ParleyTextLayout;

    fn max_width(mut self, width: f64) -> Self {
        self.max_width = width;
        self
    }

    fn alignment(mut self, alignment: TextAlignment) -> Self {
        use layout::Alignment;
        self.alignment = match alignment {
            TextAlignment::Start => Alignment::Start,
            TextAlignment::Center => Alignment::Middle,
            TextAlignment::End => Alignment::End,
            TextAlignment::Justified => Alignment::Justified,
        };
        self
    }

    fn range_attribute(
        mut self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute>,
    ) -> Self {
        self.builder.push(&convert_attr(&attribute.into()), range);
        self
    }

    fn default_attribute(mut self, attribute: impl Into<TextAttribute>) -> Self {
        self.builder.push_default(&convert_attr(&attribute.into()));
        self
    }

    fn build(mut self) -> Result<Self::Out, Error> {
        let mut layout = self.builder.build();
        layout.break_all_lines(Some(self.max_width as f32), self.alignment);
        Ok(ParleyTextLayout {
            text: self.text,
            layout,
        })
    }
}

#[derive(Clone)]
pub struct ParleyText {
    fcx: Rc<RefCell<FontContext>>,
    lcx: context::RcLayoutContext<ParleyBrush>,
    scale: f32,
}

impl ParleyText {
    pub fn new() -> Self {
        Self::with_font_context(FontContext::new())
    }

    pub fn with_font_context(fcx: FontContext) -> Self {
        Self {
            fcx: Rc::new(RefCell::new(fcx)),
            lcx: context::RcLayoutContext::new(),
            scale: 1.,
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }
}

impl Default for ParleyText {
    fn default() -> Self {
        Self::new()
    }
}

impl Text for ParleyText {
    type TextLayoutBuilder = ParleyTextLayoutBuilder;
    type TextLayout = ParleyTextLayout;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        if self.fcx.borrow().has_family(family_name) {
            Some(FontFamily::new_unchecked(family_name))
        } else {
            None
        }
    }

    fn load_font(&mut self, data: &[u8]) -> Result<FontFamily, Error> {
        if let Some(family_name) = self.fcx.borrow_mut().register_fonts(data.into()) {
            Ok(FontFamily::new_unchecked(family_name))
        } else {
            Err(Error::FontLoadingFailed)
        }
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        let text = ParleyTextStorage(Rc::new(text));
        let builder = self
            .lcx
            .ranged_builder(self.fcx.clone(), text.clone(), self.scale);
        let builder = ParleyTextLayoutBuilder {
            builder,
            text,
            max_width: f64::INFINITY,
            alignment: layout::Alignment::Start,
        };
        let defaults = piet::util::LayoutDefaults::default();
        builder
            .default_attribute(TextAttribute::FontFamily(defaults.font))
            .default_attribute(TextAttribute::FontSize(defaults.font_size))
            .default_attribute(TextAttribute::TextColor(defaults.fg_color))
    }
}

#[derive(Clone)]
pub struct ParleyTextStorage(pub Rc<dyn TextStorage>);

impl context::TextSource for ParleyTextStorage {
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn convert_attr<'a>(attr: &'a TextAttribute) -> style::StyleProperty<'a, ParleyBrush> {
    use style::FontStyle as Style;
    use style::FontWeight as Weight;
    use style::GenericFamily;
    use style::StyleProperty::*;
    match attr {
        TextAttribute::FontFamily(family) => {
            use style::FontFamily::*;
            FontStack(style::FontStack::Single(match family.inner() {
                FontFamilyInner::Named(name) => Named(&*name),
                FontFamilyInner::SansSerif => Generic(GenericFamily::SansSerif),
                FontFamilyInner::Serif => Generic(GenericFamily::Serif),
                FontFamilyInner::SystemUi => Generic(GenericFamily::SystemUi),
                FontFamilyInner::Monospace => Generic(GenericFamily::Monospace),
                _ => Named(""),
            }))
        }
        TextAttribute::FontSize(size) => FontSize(*size as f32),
        TextAttribute::Weight(weight) => FontWeight(Weight(weight.to_raw())),
        TextAttribute::Style(style) => FontStyle(match style {
            piet::FontStyle::Regular => Style::Normal,
            piet::FontStyle::Italic => Style::Italic,
        }),
        TextAttribute::TextColor(color) => Brush(ParleyBrush(color.clone())),
        TextAttribute::Underline(enable) => Underline(*enable),
        TextAttribute::Strikethrough(enable) => Strikethrough(*enable),
    }
}
