use super::font_cache::FontCache;
use super::itemize::{ItemData, RangedAttribute, SpanData};
use super::{Glyph, Layout, LayoutBuilder, Run};
use fount::Library;
use piet::{FontFamily, TextAlignment, TextStorage};
use std::cell::RefCell;
use std::rc::Rc;
use swash::shape::ShapeContext;

#[derive(Clone)]
pub struct LayoutContext {
    state: Rc<RefCell<LayoutState>>,
}

impl LayoutContext {
    pub fn new() -> Self {
        Self::with_library(&Library::default())
    }

    fn with_library(library: &Library) -> Self {
        let state = LayoutState {
            font_cache: FontCache::new(library),
            shape_context: ShapeContext::new(),
            defaults: SpanData::default(),
            attributes: vec![],
            spans: vec![],
            items: vec![],
            glyphs: vec![],
            runs: vec![],
            max_width: f64::MAX,
            alignment: TextAlignment::Start,
        };
        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }

    pub fn new_layout(&mut self, text: impl TextStorage) -> LayoutBuilder {
        self.state.borrow_mut().reset(text.as_str().len());
        LayoutBuilder::new(self.state.clone(), text)
    }
}

impl piet::Text for LayoutContext {
    type TextLayoutBuilder = LayoutBuilder;
    type TextLayout = Layout;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        self.state
            .borrow()
            .font_cache
            .context
            .family_by_name(family_name)?;
        Some(FontFamily::new_unchecked(family_name))
    }

    fn load_font(&mut self, data: &[u8]) -> Result<FontFamily, piet::Error> {
        let s = self.state.borrow();
        let ctx = &s.font_cache.context;
        // TODO: expose the full set of fonts
        if let Some(reg) = ctx.register_fonts(data.into()) {
            if let Some(family) = ctx.family(reg.families[0]) {
                return Ok(FontFamily::new_unchecked(family.name()));
            }
        }
        Err(piet::Error::FontLoadingFailed)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        self.new_layout(text)
    }
}

pub struct LayoutState {
    pub font_cache: FontCache,
    pub shape_context: ShapeContext,
    pub defaults: SpanData,
    pub attributes: Vec<RangedAttribute>,
    pub spans: Vec<SpanData>,
    pub items: Vec<ItemData>,
    pub glyphs: Vec<Glyph>,
    pub runs: Vec<Run>,
    pub max_width: f64,
    pub alignment: TextAlignment,
}

impl LayoutState {
    fn reset(&mut self, len: usize) {
        self.font_cache.reset();
        self.defaults = SpanData::default();
        self.defaults.end = len;
        self.attributes.clear();
        self.spans.clear();
        self.max_width = f64::MAX;
        self.alignment = TextAlignment::Start;
    }
}
