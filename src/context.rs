use super::font::{FontCollection, FontFamily, FontFamilyHandle, GenericFontFamily};
use super::itemize::{AttributeKind, ItemData, RangedAttribute, SpanData};
use super::{Alignment, Attribute, Brush};
use super::{Glyph, Layout, Run};
use core::ops::{Bound, Range, RangeBounds};
use fount::Library;
use swash::shape::ShapeContext;

pub struct LayoutContext<C: FontCollection, B: Brush> {
    state: LayoutState<C, B>,
}

impl<C: FontCollection, B: Brush> LayoutContext<C, B> {
    pub fn new() -> Self {
        Self {
            state: LayoutState::new(),
        }
    }

    pub fn new_layout<'a>(
        &'a mut self,
        fonts: &'a mut C,
        text: &'a str,
    ) -> LayoutBuilder<'a, C, B> {
        fonts.begin_session();
        LayoutBuilder {
            state: &mut self.state,
            fonts,
            text,
        }
    }
}

pub struct LayoutBuilder<'a, C: FontCollection, B: Brush> {
    state: &'a mut LayoutState<C, B>,
    fonts: &'a mut C,
    text: &'a str,
}

impl<'a, C: FontCollection, B: Brush> LayoutBuilder<'a, C, B> {}

pub struct LayoutState<C: FontCollection, B: Brush = ()> {
    pub shape_context: ShapeContext,
    pub defaults: SpanData<C::Family, B>,
    pub attributes: Vec<RangedAttribute<C::Family, B>>,
    pub spans: Vec<SpanData<C::Family, B>>,
    pub items: Vec<ItemData>,
    pub glyphs: Vec<Glyph>,
    pub runs: Vec<Run<C::Font>>,
    pub max_width: f64,
    pub alignment: Alignment,
}

impl<C: FontCollection, B: Brush> LayoutState<C, B> {
    pub fn new() -> Self {
        Self {
            shape_context: ShapeContext::new(),
            defaults: SpanData::default(),
            attributes: vec![],
            spans: vec![],
            items: vec![],
            glyphs: vec![],
            runs: vec![],
            max_width: f64::MAX,
            alignment: Alignment::Start,
        }
    }

    fn reset(&mut self, len: usize) {
        self.defaults = SpanData::default();
        self.defaults.end = len;
        self.attributes.clear();
        self.spans.clear();
        self.items.clear();
        self.runs.clear();
        self.max_width = f64::MAX;
        self.alignment = Alignment::Start;
    }

    pub fn default_attribute(&mut self, attribute: AttributeKind<C::Family, B>) {
        self.defaults.apply(&attribute);
    }

    pub fn range_attribute(
        &mut self,
        text_len: usize,
        range: impl RangeBounds<usize>,
        attribute: AttributeKind<C::Family, B>,
    ) {
        let range = resolve_range(range, text_len);
        if !range.is_empty() {
            self.attributes.push(RangedAttribute {
                attr: attribute,
                start: range.start,
                end: range.end,
            });
        }
    }

    pub fn build(&mut self, fonts: &mut C, text: &str) {
        use super::itemize::{itemize, normalize_spans};
        use super::shape::{shape, FontSelector};
        self.defaults.end = text.len();
        normalize_spans(&self.attributes, &self.defaults, &mut self.spans);
        self.items.clear();
        itemize(text, &mut self.spans, &mut self.items);
        if self.items.is_empty() {
            return;
        }
        let mut font_selector = FontSelector::new(fonts, &self.spans, &self.items[0]);
        shape(
            &mut self.shape_context,
            &mut font_selector,
            &self.spans,
            &self.items,
            text,
            &mut self.glyphs,
            &mut self.runs,
        );
    }
}

fn convert_attr<C: FontCollection, B: Brush>(
    attr: &Attribute<B>,
    fonts: &mut C,
) -> AttributeKind<C::Family, B> {
    match attr {
        Attribute::FontFamily(family) => AttributeKind::Family(match family {
            FontFamily::Named(name) => fonts
                .query_family(name)
                .map(|f| FontFamilyHandle::Named(f))
                .unwrap_or(FontFamilyHandle::Default),
            FontFamily::Generic(family) => FontFamilyHandle::Generic(*family),
        }),
        Attribute::FontSize(size) => AttributeKind::Size(*size as _),
        Attribute::FontWeight(weight) => AttributeKind::Weight(*weight),
        Attribute::FontStyle(style) => AttributeKind::Style(*style),
        Attribute::FontStretch(stretch) => AttributeKind::Stretch(*stretch),
        Attribute::Color(color) => AttributeKind::Color(color.clone()),
        Attribute::Underline(yes) => AttributeKind::Underline(*yes),
        Attribute::Strikethrough(yes) => AttributeKind::Strikethrough(*yes),
    }
}

/// Resolves a `RangeBounds` into a range in the range 0..len.
pub fn resolve_range(range: impl RangeBounds<usize>, len: usize) -> Range<usize> {
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(n) => *n,
        Bound::Excluded(n) => *n + 1,
    };

    let end = match range.end_bound() {
        Bound::Unbounded => len,
        Bound::Included(n) => *n + 1,
        Bound::Excluded(n) => *n,
    };

    start.min(len)..end.min(len)
}
