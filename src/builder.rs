use super::context::LayoutState;
use super::font::Font;
use super::font_cache::FontCache;
use super::itemize::*;
use super::{Glyph, Layout, Run};
use core::ops::RangeBounds;
use fount::{FontContext, GenericFamily, Locale};
use piet::{TextAlignment, TextAttribute, TextStorage};
use std::cell::RefCell;
use std::rc::Rc;
use swash::shape::{self, ShapeContext};
use swash::text::cluster::CharCluster;
use swash::text::Script;
use swash::{Attributes, FontRef, Synthesis};

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
    let first_item = &state.items[0];
    let first_span = &state.spans[0];
    let mut selector = FontSelector {
        font_cache: &mut state.font_cache,
        spans: &state.spans,
        span_index: 0,
        script: first_item.script,
        locale: first_item.locale,
        family: first_span.family,
        attrs: first_span.attributes(),
    };
    shape(
        &mut state.shape_context,
        &mut selector,
        &state.spans,
        &state.items,
        &mut state.glyphs,
        &mut state.runs,
        layout,
    );
    layout.glyphs.extend(state.glyphs.drain(..));
    layout.runs.extend(state.runs.drain(..));
}

fn shape(
    shape_context: &mut ShapeContext,
    font_selector: &mut FontSelector,
    spans: &[SpanData],
    items: &[ItemData],
    glyphs: &mut Vec<Glyph>,
    runs: &mut Vec<Run>,
    layout: &mut Layout,
) {
    use swash::text::cluster::*;
    for item in items {
        font_selector.script = item.script;
        font_selector.locale = item.locale;
        font_selector
            .font_cache
            .select_fallbacks(item.script, item.locale, font_selector.attrs);
        let options = shape::partition::SimpleShapeOptions {
            size: item.size,
            script: item.script,
            language: item.locale,
            ..Default::default()
        };
        let start = item.start;
        let text = &layout.text.as_str()[start..item.end];
        shape::partition::shape(
            shape_context,
            font_selector,
            &options,
            text.char_indices().map(|(i, ch)| Token {
                ch,
                offset: (start + i) as u32,
                len: ch.len_utf8() as u8,
                info: ch.into(),
                // FIXME: Precompute and store this somewhere
                data: match spans.binary_search_by(|probe| probe.start.cmp(&(start + i))) {
                    Ok(index) => index as u32,
                    Err(index) => index.saturating_sub(1) as u32,
                },
            }),
            |font, shaper| {
                let glyph_start = glyphs.len();
                let mut text_start = usize::MAX;
                let mut text_end = 0;
                shaper.shape_with(|cluster| {
                    for g in cluster.glyphs {
                        glyphs.push(Glyph {
                            id: g.id,
                            x: g.x,
                            y: g.y,
                            advance: g.advance,
                        });
                    }
                    let range = cluster.source.to_range();
                    text_start = range.start.min(text_start);
                    text_end = range.end.max(text_end);
                });
                runs.push(Run {
                    font: font.font.clone(),
                    glyph_range: glyph_start..glyphs.len(),
                    text_range: text_start..text_end,
                });
            },
        );
    }
}

struct FontSelector<'a> {
    font_cache: &'a mut FontCache,
    spans: &'a [SpanData],
    span_index: u32,
    script: Script,
    locale: Option<Locale>,
    family: FamilyKind,
    attrs: Attributes,
}

impl<'a> shape::partition::Selector for FontSelector<'a> {
    type SelectedFont = SelectedFont;

    fn select_font(&mut self, cluster: &mut CharCluster) -> Option<Self::SelectedFont> {
        let span_index = cluster.user_data();
        if span_index != self.span_index {
            self.span_index = span_index;
            let span = &self.spans[span_index as usize];
            let family = span.family;
            let diff_family = family != self.family;
            let attrs = span.attributes();
            let diff_attrs = attrs != self.attrs;
            self.family = family;
            self.attrs = attrs;
            if diff_attrs || diff_family {
                if diff_attrs {
                    self.font_cache.select_family(family, attrs);
                    self.font_cache
                        .select_fallbacks(self.script, self.locale, attrs);
                } else {
                    self.font_cache.select_family(family, attrs);
                }
            }
        }
        let font = self.font_cache.map_cluster(cluster)?;
        let synthesis = font.as_ref().attributes().synthesize(self.attrs);
        Some(SelectedFont { font, synthesis })
    }
}

struct SelectedFont {
    pub font: Font,
    pub synthesis: Synthesis,
}

impl PartialEq for SelectedFont {
    fn eq(&self, other: &Self) -> bool {
        self.font.key == other.font.key && self.synthesis == other.synthesis
    }
}

impl shape::partition::SelectedFont for SelectedFont {
    fn font(&self) -> FontRef {
        self.font.as_ref()
    }

    fn synthesis(&self) -> Option<Synthesis> {
        Some(self.synthesis)
    }
}
