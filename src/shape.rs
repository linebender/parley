use super::font::{Font, FontContext};
use super::layout::Layout;
use super::resolve::range::RangedStyle;
use super::resolve::ResolveContext;
use super::style::{Brush, FontFeature, FontVariation};
use swash::shape::*;
use swash::text::cluster::{CharCluster, CharInfo, Token};
use swash::text::{Language, Script};
use swash::{Attributes, FontRef, Synthesis};

pub fn shape_text<B: Brush>(
    rcx: &ResolveContext,
    fcx: &mut FontContext,
    styles: &[RangedStyle<B>],
    infos: &[(CharInfo, u16)],
    levels: &[u8],
    scx: &mut ShapeContext,
    text: &str,
    layout: &mut Layout<B>,
) {
    let mut cur_style = &styles[0].style;
    let mut cur_style_index = 0;
    let mut cur_size = cur_style.font_size;
    let mut cur_level = levels.get(0).copied().unwrap_or(0);
    let mut cur_script = infos
        .iter()
        .map(|x| x.0.script())
        .find(|&script| real_script(script))
        .unwrap_or(Script::Latin);
    let mut cur_locale = cur_style.locale;
    let mut cur_variations = cur_style.font_variations;
    let mut cur_features = cur_style.font_features;
    let mut char_range = 0..0;
    let mut text_range = 0..0;
    macro_rules! shape_run {
        () => {
            let item_text = &text[text_range.clone()];
            let item_infos = &infos[char_range.start..];
            let first_style_index = item_infos[0].1;
            let mut fs =
                FontSelector::new(fcx, rcx, styles, first_style_index, cur_script, cur_locale);
            let options = partition::SimpleShapeOptions {
                size: cur_size,
                script: cur_script,
                language: cur_locale,
                direction: if cur_level & 1 != 0 {
                    Direction::RightToLeft
                } else {
                    Direction::LeftToRight
                },
                variations: rcx.variations(cur_variations).unwrap_or(&[]),
                features: rcx.features(cur_features).unwrap_or(&[]),
                insert_dotted_circles: false,
            };
            partition::shape(
                scx,
                &mut fs,
                &options,
                item_text.char_indices().zip(item_infos).map(
                    |((offset, ch), (info, style_index))| Token {
                        ch,
                        offset: (text_range.start + offset) as u32,
                        len: ch.len_utf8() as u8,
                        info: *info,
                        data: *style_index as _,
                    },
                ),
                |font, shaper| {
                    layout
                        .data
                        .push_run(font.font.clone(), font.synthesis, shaper, cur_level);
                },
            );
        };
    }
    for ((char_index, ch), (info, style_index)) in text.chars().enumerate().zip(infos) {
        let mut break_run = false;
        if cur_style_index != *style_index {
            cur_style_index = *style_index;
            cur_style = &styles[*style_index as usize].style;
            if cur_style.font_size != cur_size
                || cur_style.locale != cur_locale
                || cur_style.font_variations != cur_variations
                || cur_style.font_features != cur_features
            {
                break_run = true;
            }
        }
        let mut script = info.script();
        if !real_script(script) {
            script = cur_script;
        }
        let level = levels.get(char_index).copied().unwrap_or(0);
        if break_run || level != cur_level || script != cur_script {
            shape_run!();
            cur_size = cur_style.font_size;
            cur_level = level;
            cur_script = script;
            cur_locale = cur_style.locale;
            cur_variations = cur_style.font_variations;
            cur_features = cur_style.font_features;
            text_range.start = text_range.end;
            char_range.start = char_range.end;
        }
        text_range.end += ch.len_utf8();
        char_range.end += 1;
    }
    if !text_range.is_empty() {
        shape_run!();
    }
}

fn real_script(script: Script) -> bool {
    script != Script::Common && script != Script::Unknown && script != Script::Inherited
}

struct FontSelector<'a, B: Brush> {
    fcx: &'a mut FontContext,
    rcx: &'a ResolveContext,
    styles: &'a [RangedStyle<B>],
    style_index: u16,
    script: Script,
    locale: Option<Language>,
    attrs: Attributes,
    variations: &'a [FontVariation],
    features: &'a [FontFeature],
}

impl<'a, B: Brush> FontSelector<'a, B> {
    fn new(
        fcx: &'a mut FontContext,
        rcx: &'a ResolveContext,
        styles: &'a [RangedStyle<B>],
        style_index: u16,
        script: Script,
        locale: Option<Language>,
    ) -> Self {
        let style = &styles[style_index as usize].style;
        let fonts_id = style.font_stack.id();
        let fonts = rcx.stack(style.font_stack).unwrap_or(&[]);
        let attrs = Attributes::new(style.font_stretch, style.font_weight, style.font_style);
        let variations = rcx.variations(style.font_variations).unwrap_or(&[]);
        let features = rcx.features(style.font_features).unwrap_or(&[]);
        fcx.cache.select_families(fonts_id, fonts, attrs);
        fcx.cache.select_fallbacks(script, locale, attrs);
        Self {
            fcx,
            rcx,
            styles,
            style_index,
            script,
            locale,
            attrs,
            variations,
            features,
        }
    }
}

impl<'a, B: Brush> partition::Selector for FontSelector<'a, B> {
    type SelectedFont = SelectedFont;

    fn select_font(&mut self, cluster: &mut CharCluster) -> Option<Self::SelectedFont> {
        let style_index = cluster.user_data() as u16;
        if style_index != self.style_index {
            self.style_index = style_index;
            let style = &self.styles[style_index as usize].style;
            let fonts_id = style.font_stack.id();
            let fonts = self.rcx.stack(style.font_stack).unwrap_or(&[]);
            let attrs = Attributes::new(style.font_stretch, style.font_weight, style.font_style);
            let variations = self.rcx.variations(style.font_variations).unwrap_or(&[]);
            let features = self.rcx.features(style.font_features).unwrap_or(&[]);
            self.fcx.cache.select_families(fonts_id, fonts, attrs);
            if self.attrs != attrs {
                self.fcx
                    .cache
                    .select_fallbacks(self.script, self.locale, attrs);
            }
            self.attrs = attrs;
            self.variations = variations;
            self.features = features;
        }
        let (font, synthesis) = self.fcx.cache.map_cluster(cluster)?;
        Some(SelectedFont { font, synthesis })
    }
}

#[derive(PartialEq)]
struct SelectedFont {
    font: Font,
    synthesis: Synthesis,
}

impl partition::SelectedFont for SelectedFont {
    fn font(&self) -> FontRef {
        self.font.as_ref()
    }

    fn synthesis(&self) -> Option<Synthesis> {
        Some(self.synthesis)
    }
}
