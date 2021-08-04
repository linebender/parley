use super::context::LayoutState;
use super::font::system::{Font, SystemFontCollection};
use super::font::{FontCollection, FontFamilyHandle, GenericFontFamily};
use super::itemize::*;
use super::Color;
use super::Layout;
use fount::FamilyId;
use piet::kurbo::{Point, Rect, Size};
use piet::{
    FontFamily, HitTestPoint, HitTestPosition, LineMetric, TextAlignment, TextAttribute,
    TextStorage,
};
use std::cell::RefCell;
use std::ops::RangeBounds;
use std::rc::Rc;

#[derive(Clone)]
pub struct PietText {
    fonts: Rc<RefCell<SystemFontCollection>>,
    state: Rc<RefCell<LayoutState<SystemFontCollection>>>,
}

impl PietText {
    pub fn new() -> Self {
        Self {
            fonts: Rc::new(RefCell::new(SystemFontCollection::new())),
            state: Rc::new(RefCell::new(LayoutState::new())),
        }
    }
}

impl piet::Text for PietText {
    type TextLayoutBuilder = PietTextLayoutBuilder;
    type TextLayout = PietTextLayout;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        self.fonts.borrow().context().family_by_name(family_name)?;
        Some(FontFamily::new_unchecked(family_name))
    }

    fn load_font(&mut self, data: &[u8]) -> Result<FontFamily, piet::Error> {
        let fonts = self.fonts.borrow();
        let ctx = &fonts.context();
        // TODO: expose the full set of fonts
        if let Some(reg) = ctx.register_fonts(data.into()) {
            if let Some(family) = ctx.family(reg.families[0]) {
                return Ok(FontFamily::new_unchecked(family.name()));
            }
        }
        Err(piet::Error::FontLoadingFailed)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        self.fonts.borrow_mut().begin_session();
        PietTextLayoutBuilder {
            fonts: self.fonts.clone(),
            text: Rc::new(text),
            state: self.state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct PietTextLayout {
    text: Rc<dyn TextStorage>,
    pub layout: Layout<Font>,
}

impl piet::TextLayout for PietTextLayout {
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

pub struct PietTextLayoutBuilder {
    fonts: Rc<RefCell<SystemFontCollection>>,
    text: Rc<dyn TextStorage>,
    state: Rc<RefCell<LayoutState<SystemFontCollection>>>,
}

impl piet::TextLayoutBuilder for PietTextLayoutBuilder {
    type Out = PietTextLayout;

    fn max_width(self, width: f64) -> Self {
        self.state.borrow_mut().max_width = width;
        self
    }

    fn alignment(self, alignment: TextAlignment) -> Self {
        self.state.borrow_mut().alignment = match alignment {
            TextAlignment::Start => super::Alignment::Start,
            TextAlignment::End => super::Alignment::End,
            TextAlignment::Center => super::Alignment::Center,
            TextAlignment::Justified => super::Alignment::Justified,
        };
        self
    }

    fn default_attribute(self, attribute: impl Into<TextAttribute>) -> Self {
        {
            let mut state = self.state.borrow_mut();
            let mut fonts = self.fonts.borrow_mut();
            let attr = convert_attr(&attribute.into(), &mut fonts);
            state.defaults.apply(&attr);
        }
        self
    }

    fn range_attribute(
        self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute>,
    ) -> Self {
        {
            let mut state = self.state.borrow_mut();
            let mut fonts = self.fonts.borrow_mut();
            let attr = convert_attr(&attribute.into(), &mut fonts);
            state.range_attribute(self.text.len(), range, attr);
        }
        self
    }

    fn build(self) -> Result<Self::Out, piet::Error> {
        let mut state = self.state.borrow_mut();
        let mut fonts = self.fonts.borrow_mut();
        let state = &mut *state;
        state.build(&mut fonts, &self.text);
        fonts.end_session();
        let mut layout = Layout {
            glyphs: vec![],
            runs: vec![],
        };
        layout.glyphs.extend(state.glyphs.drain(..));
        layout.runs.extend(state.runs.drain(..));
        Ok(PietTextLayout {
            text: self.text,
            layout,
        })
    }
}

fn convert_attr(
    attr: &TextAttribute,
    fonts: &mut SystemFontCollection,
) -> AttributeKind<FamilyId, ()> {
    use piet::FontStyle;
    use swash::{Style, Weight};
    match attr {
        TextAttribute::FontFamily(family) => {
            use piet::FontFamilyInner;
            AttributeKind::Family(match family.inner() {
                FontFamilyInner::Named(name) => fonts
                    .query_family(name)
                    .map(|family| FontFamilyHandle::Named(family))
                    .unwrap_or(FontFamilyHandle::Default),
                _ => FontFamilyHandle::Generic(match family.inner() {
                    FontFamilyInner::Monospace => GenericFontFamily::Monospace,
                    FontFamilyInner::SystemUi => GenericFontFamily::SystemUI,
                    FontFamilyInner::Serif => GenericFontFamily::Serif,
                    _ => GenericFontFamily::SansSerif,
                }),
            })
        }
        TextAttribute::FontSize(size) => AttributeKind::Size(*size),
        TextAttribute::Weight(weight) => AttributeKind::Weight(Weight(weight.to_raw())),
        TextAttribute::Style(style) => AttributeKind::Style(match style {
            FontStyle::Italic => Style::Italic,
            _ => Style::Normal,
        }),
        TextAttribute::TextColor(color) => {
            let (r, g, b, a) = color.as_rgba8();
            AttributeKind::Color(Color::Solid([r, g, b, a]))
        }
        TextAttribute::Underline(yes) => AttributeKind::Underline(*yes),
        TextAttribute::Strikethrough(yes) => AttributeKind::Strikethrough(*yes),
    }
}
