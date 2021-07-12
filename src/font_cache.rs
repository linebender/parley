use super::font::Font;
use super::itemize::FamilyKind;
use fount::{FontContext, FontData, FontId, Library, Locale, SourceId};
use std::collections::HashMap;
use swash::text::cluster::*;
use swash::text::Script;
use swash::{Attributes, FontRef};

// Make this configurable?
const RETAINED_SOURCE_COUNT: usize = 12;

#[derive(Clone)]
pub struct FontCache {
    pub context: FontContext,
    sources: HashMap<SourceId, (u64, FontData)>,
    selected_params: Option<(FamilyKind, Attributes)>,
    selected_fonts: Vec<FontId>,
    fallback_params: (Script, Option<Locale>, Attributes),
    fallback_fonts: Vec<FontId>,
    serial: u64,
}

impl FontCache {
    pub fn new(library: &Library) -> Self {
        Self {
            context: FontContext::new(library),
            sources: HashMap::new(),
            selected_params: None,
            selected_fonts: vec![],
            fallback_params: (Script::Unknown, None, Attributes::default()),
            fallback_fonts: vec![],
            serial: 1,
        }
    }

    pub fn reset(&mut self) {
        self.selected_params = None;
        self.selected_fonts.clear();
        self.fallback_params = (Script::Unknown, None, Attributes::default());
        self.fallback_fonts.clear();
        self.serial += 1;
        let mut target_serial = self.serial.saturating_sub(RETAINED_SOURCE_COUNT as u64);
        let mut count = self.sources.len();
        while count > RETAINED_SOURCE_COUNT {
            self.sources.retain(|_, v| {
                if count > RETAINED_SOURCE_COUNT && v.0 <= target_serial {
                    count -= 1;
                    false
                } else {
                    true
                }
            });
            target_serial += 1;
        }
    }

    pub fn select_family(&mut self, kind: FamilyKind, attrs: Attributes) {
        if self.selected_params != Some((kind, attrs)) {
            self.selected_params = Some((kind, attrs));
            let families = match &kind {
                FamilyKind::Named(id) => core::slice::from_ref(id),
                FamilyKind::Default => self.context.default_families(),
                FamilyKind::Generic(family) => self.context.generic_families(*family),
            };
            self.selected_fonts.clear();
            let context = &self.context;
            self.selected_fonts.extend(
                families
                    .iter()
                    .filter_map(|id| context.family(*id))
                    .filter_map(|family| family.query(attrs)),
            );
        }
    }

    pub fn select_fallbacks(&mut self, script: Script, locale: Option<Locale>, attrs: Attributes) {
        if self.fallback_params != (script, locale, attrs) {
            self.fallback_params = (script, locale, attrs);
            self.fallback_fonts.clear();
            let context = &self.context;
            let fallback_families = context.fallback_families(script, locale);
            self.fallback_fonts.extend(
                fallback_families
                    .iter()
                    .filter_map(|id| context.family(*id))
                    .filter_map(|family| family.query(attrs)),
            );
        }
    }

    pub fn map_cluster(&mut self, cluster: &mut CharCluster) -> Option<Font> {
        let mut best = None;
        for i in 0..self.selected_fonts.len() {
            let font_id = self.selected_fonts[i];
            if let Some(font) = self.load_font(font_id) {
                let charmap = font.as_ref().charmap();
                match cluster.map(|ch| charmap.map(ch)) {
                    Status::Complete => return Some(font),
                    Status::Keep => best = Some(font),
                    _ => {}
                }
            }
        }
        if cluster.info().is_emoji() {
            use fount::GenericFamily;
            let context = &self.context;
            let attrs = self.fallback_params.2;
            if let Some(font_id) = self
                .context
                .generic_families(GenericFamily::Emoji)
                .iter()
                .filter_map(|id| context.family(*id))
                .filter_map(|family| family.query(attrs))
                .next()
            {
                if let Some(font) = self.load_font(font_id) {
                    let charmap = font.as_ref().charmap();
                    match cluster.map(|ch| charmap.map(ch)) {
                        Status::Complete => return Some(font),
                        Status::Keep => best = Some(font),
                        _ => {}
                    }
                }
            }
        }
        for i in 0..self.fallback_fonts.len() {
            let font_id = self.fallback_fonts[i];
            if let Some(font) = self.load_font(font_id) {
                let charmap = font.as_ref().charmap();
                match cluster.map(|ch| charmap.map(ch)) {
                    Status::Complete => return Some(font),
                    Status::Keep => best = Some(font),
                    _ => {}
                }
            }
        }
        best
    }

    pub fn load_font(&mut self, id: FontId) -> Option<Font> {
        let entry = self.context.font(id)?;
        let source_id = entry.source();
        let data = if let Some(cached_source) = self.sources.get_mut(&source_id) {
            cached_source.0 = self.serial;
            cached_source.1.clone()
        } else {
            let data = self.context.load(source_id)?;
            self.sources.insert(source_id, (self.serial, data.clone()));
            data
        };
        let font_ref = FontRef::from_index(&data, entry.index() as usize)?;
        let offset = font_ref.offset;
        Some(Font {
            data,
            offset,
            key: entry.cache_key(),
        })
    }
}
