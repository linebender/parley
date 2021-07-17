use super::context::LayoutState;
use super::itemize::*;
use super::shape::{shape, FontSelector};
use super::Layout;
use core::ops::RangeBounds;
use fount::{FontContext, GenericFamily};
use piet::{TextAlignment, TextAttribute, TextStorage};
use std::cell::RefCell;
use std::rc::Rc;

pub struct LayoutBuilder {
    state: Rc<RefCell<LayoutState>>,
    text: Rc<dyn TextStorage>,
}

impl LayoutBuilder {
    pub(crate) fn new(state: Rc<RefCell<LayoutState>>, text: impl TextStorage) -> Self {
        Self {
            state,
            text: Rc::new(text),
        }
    }
}

impl piet::TextLayoutBuilder for LayoutBuilder {
    type Out = Layout;

    fn max_width(self, width: f64) -> Self {
        self.state.borrow_mut().max_width = width;
        self
    }

    fn alignment(self, alignment: TextAlignment) -> Self {
        self.state.borrow_mut().alignment = alignment;
        self
    }

    fn default_attribute(self, attribute: impl Into<TextAttribute>) -> Self {
        {
            let mut state = self.state.borrow_mut();
            let attr = convert_attr(&attribute.into(), &state.font_cache.context);
            state.defaults.apply(&attr);
        }
        self
    }

    fn range_attribute(
        self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute>,
    ) -> Self {
        let range = piet::util::resolve_range(range, self.text.len());
        if !range.is_empty() {
            let mut state = self.state.borrow_mut();
            let attr = convert_attr(&attribute.into(), &state.font_cache.context);
            state.attributes.push(RangedAttribute {
                attr,
                start: range.start,
                end: range.end,
            });
        }
        self
    }

    fn build(self) -> Result<Self::Out, piet::Error> {
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        normalize_spans(&state.attributes, state.defaults, &mut state.spans);
        let mut layout = Layout {
            text: self.text,
            glyphs: vec![],
            runs: vec![],
        };
        build(state, &mut layout);
        Ok(layout)
    }
}

fn convert_attr(attr: &TextAttribute, fcx: &FontContext) -> AttributeKind {
    use piet::FontStyle;
    use swash::{Style, Weight};
    match attr {
        TextAttribute::FontFamily(family) => {
            use piet::FontFamilyInner;
            AttributeKind::Family(match family.inner() {
                FontFamilyInner::Named(name) => fcx
                    .family_by_name(name)
                    .map(|f| FamilyKind::Named(f.id()))
                    .unwrap_or(FamilyKind::Default),
                _ => FamilyKind::Generic(match family.inner() {
                    FontFamilyInner::Monospace => GenericFamily::Monospace,
                    FontFamilyInner::SystemUi => GenericFamily::SystemUI,
                    FontFamilyInner::Serif => GenericFamily::Serif,
                    _ => GenericFamily::SansSerif,
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
            AttributeKind::Color([r, g, b, a])
        }
        TextAttribute::Underline(yes) => AttributeKind::Underline(*yes),
        TextAttribute::Strikethrough(yes) => AttributeKind::Strikethrough(*yes),
    }
}

fn build(state: &mut LayoutState, layout: &mut Layout) {
    state.items.clear();
    itemize(&layout.text, &mut state.spans, &mut state.items);
    if state.items.is_empty() {
        return;
    }
    let mut font_selector = FontSelector::new(&mut state.font_cache, &state.spans, &state.items[0]);
    shape(
        &mut state.shape_context,
        &mut font_selector,
        &state.spans,
        &state.items,
        &mut state.glyphs,
        &mut state.runs,
        layout,
    );
    layout.glyphs.extend(state.glyphs.drain(..));
    layout.runs.extend(state.runs.drain(..));
}
