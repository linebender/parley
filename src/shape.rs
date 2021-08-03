use super::font::*;
use super::itemize::*;
use super::{Glyph, Layout, Run};
use fount::Locale;
use swash::shape::{self, ShapeContext};
use swash::text::cluster::CharCluster;
use swash::text::Script;
use swash::{Attributes, FontRef, Synthesis};

pub fn shape<C: FontCollection>(
    shape_context: &mut ShapeContext,
    font_selector: &mut FontSelector<C>,
    spans: &[SpanData<C::Family>],
    items: &[ItemData],
    text: &str,
    glyphs: &mut Vec<Glyph>,
    runs: &mut Vec<Run<C::Font>>,
) {
    use swash::text::cluster::*;
    for item in items {
        font_selector.script = item.script;
        font_selector.locale = item.locale;
        // font_selector
        //     .font_cache
        //     .select_fallbacks(item.script, item.locale, font_selector.attrs);
        let options = shape::partition::SimpleShapeOptions {
            size: item.size,
            script: item.script,
            language: item.locale,
            ..Default::default()
        };
        let start = item.start;
        let item_text = &text[start..item.end];
        shape::partition::shape(
            shape_context,
            font_selector,
            &options,
            item_text.char_indices().map(|(i, ch)| Token {
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

pub struct FontSelector<'a, C: FontCollection> {
    fonts: &'a mut C,
    spans: &'a [SpanData<C::Family>],
    span_index: u32,
    script: Script,
    locale: Option<Locale>,
    family: FontFamilyHandle<C::Family>,
    attrs: Attributes,
}

impl<'a, C: FontCollection> FontSelector<'a, C> {
    pub fn new(fonts: &'a mut C, spans: &'a [SpanData<C::Family>], first_item: &ItemData) -> Self {
        let first_span = &spans[0];
        let attrs = first_span.attributes();
        Self {
            fonts,
            spans,
            span_index: 0,
            script: first_item.script,
            locale: first_item.locale,
            family: first_span.family.clone(),
            attrs,
        }
    }
}

impl<'a, C: FontCollection> shape::partition::Selector for FontSelector<'a, C> {
    type SelectedFont = SelectedFont<C::Font>;

    fn select_font(&mut self, cluster: &mut CharCluster) -> Option<Self::SelectedFont> {
        let span_index = cluster.user_data();
        if span_index != self.span_index {
            self.span_index = span_index;
            let span = &self.spans[span_index as usize];
            let family = span.family.clone();
            let diff_family = family != self.family;
            let attrs = span.attributes();
            let diff_attrs = attrs != self.attrs;
            self.family = family;
            self.attrs = attrs;
            // if diff_attrs || diff_family {
            //     if diff_attrs {
            //         self.font_cache.select_family(family, attrs);
            //         self.font_cache
            //             .select_fallbacks(self.script, self.locale, attrs);
            //     } else {
            //         self.font_cache.select_family(family, attrs);
            //     }
            // }
        }
        let font = self.fonts.map(
            &self.family,
            self.attrs,
            &FontFallbacks {
                script: self.script,
                locale: self.locale,
            },
            cluster,
        )?;
        Some(SelectedFont { font })
    }
}

pub struct SelectedFont<F: FontInstance> {
    pub font: F,
}

impl<F: FontInstance> PartialEq for SelectedFont<F> {
    fn eq(&self, other: &Self) -> bool {
        self.font == other.font
    }
}

impl<F: FontInstance> shape::partition::SelectedFont for SelectedFont<F> {
    fn font(&self) -> FontRef {
        self.font.as_font_ref()
    }

    fn synthesis(&self) -> Option<Synthesis> {
        self.font.synthesis()
    }
}
