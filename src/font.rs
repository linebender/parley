//! Font management.

use fount::{FamilyId, FontData, FontId, GenericFamily, Library, Locale, SourceId};
use std::collections::HashMap;
use swash::proxy::CharmapProxy;
use swash::text::cluster::*;
use swash::text::Script;
use swash::{Attributes, CacheKey, FontRef, Synthesis};

// Make this configurable?
const RETAINED_SOURCE_COUNT: usize = 12;

/// Shared handle to a font.
#[derive(Clone)]
pub struct Font {
    data: FontData,
    offset: u32,
    key: CacheKey,
}

impl Font {
    /// Returns a reference to the font.
    pub fn as_ref(&self) -> FontRef {
        FontRef {
            data: &*self.data,
            offset: self.offset,
            key: self.key,
        }
    }
}

impl PartialEq for Font {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

/// Context for font selection and fallback.
#[derive(Clone)]
pub struct FontContext {
    pub(crate) cache: FontCache,
}

impl FontContext {
    pub fn new() -> Self {
        Self {
            cache: FontCache::new(),
        }
    }

    /// Returns true if a family of the specified name exists in the context.
    pub fn has_family(&self, name: &str) -> bool {
        self.cache.context.family_by_name(name).is_some()
    }

    /// Registers the fonts in the specified font data. Returns the family name
    /// for the first registerd font.
    ///
    /// This API is temporary to support piet until the more of the underlying
    /// font collection code is exposed.
    pub fn register_fonts(&mut self, data: Vec<u8>) -> Option<String> {
        let reg = self.cache.context.register_fonts(data)?;
        let first_family = reg.families.get(0)?;
        let family = self.cache.context.family(*first_family)?;
        Some(family.name().to_owned())
    }
}

#[derive(Clone)]
pub(crate) struct FontCache {
    pub context: fount::FontContext,
    sources: SourceCache,
    selected_params: Option<(usize, Attributes)>,
    selected_fonts: Vec<CachedFont>,
    fallback_params: (Script, Option<Locale>, Attributes),
    fallback_fonts: Vec<CachedFont>,
    emoji_font: Option<CachedFont>,
    attrs: Attributes,
}

impl FontCache {
    pub fn new() -> Self {
        Self {
            context: fount::FontContext::new(&Library::default()),
            sources: SourceCache::default(),
            selected_params: None,
            selected_fonts: vec![],
            fallback_params: (Script::Unknown, None, Attributes::default()),
            fallback_fonts: vec![],
            emoji_font: None,
            attrs: Attributes::default(),
        }
    }

    pub fn reset(&mut self) {
        self.selected_params = None;
        self.selected_fonts.clear();
        self.fallback_params = (Script::Unknown, None, Attributes::default());
        self.fallback_fonts.clear();
        self.sources.serial += 1;
        self.sources.prune();
        self.attrs = Attributes::default();
    }

    pub fn select_families(&mut self, id: usize, families: &[FamilyId], attrs: Attributes) {
        if self.selected_params != Some((id, attrs)) {
            self.selected_params = Some((id, attrs));
            self.selected_fonts.clear();
            let context = &self.context;
            self.selected_fonts.extend(
                families
                    .iter()
                    .filter_map(|id| context.family(*id))
                    .filter_map(|family| family.query(attrs))
                    .map(CachedFont::new),
            );
            self.attrs = attrs;
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
                    .filter_map(|family| family.query(attrs))
                    .map(CachedFont::new),
            );
            self.attrs = attrs;
        }
    }

    pub fn map_cluster(&mut self, cluster: &mut CharCluster) -> Option<(Font, Synthesis)> {
        let mut best = None;
        if map_cluster(
            &self.context,
            &mut self.sources,
            &mut self.selected_fonts,
            cluster,
            &mut best,
        ) {
            return best.map(|(font, attrs)| (font, attrs.synthesize(self.attrs)));
        }
        if cluster.info().is_emoji() {
            if self.emoji_font.is_none() {
                self.emoji_font = self
                    .context
                    .generic_families(GenericFamily::Emoji)
                    .iter()
                    .filter_map(|id| self.context.family(*id))
                    .filter_map(|family| family.query(Attributes::default()))
                    .map(CachedFont::new)
                    .next()
            }
            if let Some(emoji_font) = &mut self.emoji_font {
                if map_cluster(
                    &self.context,
                    &mut self.sources,
                    core::slice::from_mut(emoji_font),
                    cluster,
                    &mut best,
                ) {
                    return best.map(|(font, attrs)| (font, attrs.synthesize(self.attrs)));
                }
            }
        }
        map_cluster(
            &self.context,
            &mut self.sources,
            &mut self.fallback_fonts,
            cluster,
            &mut best,
        );
        best.map(|(font, attrs)| (font, attrs.synthesize(self.attrs)))
    }
}

fn map_cluster(
    context: &fount::FontContext,
    sources: &mut SourceCache,
    fonts: &mut [CachedFont],
    cluster: &mut CharCluster,
    best: &mut Option<(Font, Attributes)>,
) -> bool {
    for font in fonts {
        if font.map_cluster(context, sources, cluster, best) {
            return true;
        }
    }
    false
}

#[derive(Clone, Default)]
struct SourceCache {
    sources: HashMap<SourceId, (u64, FontData)>,
    serial: u64,
}

impl SourceCache {
    fn prune(&mut self) {
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

    fn get(&mut self, context: &fount::FontContext, id: FontId) -> Option<(Font, Attributes)> {
        let entry = context.font(id)?;
        let source_id = entry.source();
        let data = if let Some(cached_source) = self.sources.get_mut(&source_id) {
            cached_source.0 = self.serial;
            cached_source.1.clone()
        } else {
            let data = context.load(source_id)?;
            self.sources.insert(source_id, (self.serial, data.clone()));
            data
        };
        let font_ref = FontRef::from_index(&data, entry.index() as usize)?;
        let offset = font_ref.offset;
        Some((
            Font {
                data,
                offset,
                key: entry.cache_key(),
            },
            entry.attributes(),
        ))
    }
}

#[derive(Clone)]
struct CachedFont {
    id: FontId,
    font: Option<(Font, CharmapProxy)>,
    attrs: Attributes,
    error: bool,
}

impl CachedFont {
    fn new(id: FontId) -> Self {
        Self {
            id,
            font: None,
            attrs: Attributes::default(),
            error: false,
        }
    }

    fn map_cluster(
        &mut self,
        context: &fount::FontContext,
        sources: &mut SourceCache,
        cluster: &mut CharCluster,
        best: &mut Option<(Font, Attributes)>,
    ) -> bool {
        if self.error {
            return false;
        }
        let (font, charmap_proxy) = if let Some(font) = &self.font {
            (&font.0, font.1)
        } else if let Some((font, attrs)) = sources.get(context, self.id) {
            self.font = Some((font.clone(), CharmapProxy::from_font(&font.as_ref())));
            self.attrs = attrs;
            let (font, charmap_proxy) = self.font.as_ref().unwrap();
            (font, *charmap_proxy)
        } else {
            self.error = true;
            return false;
        };
        let charmap = charmap_proxy.materialize(&font.as_ref());
        match cluster.map(|ch| charmap.map(ch)) {
            Status::Complete => {
                *best = Some((font.clone(), self.attrs));
                return true;
            }
            Status::Keep => {
                *best = Some((font.clone(), self.attrs));
            }
            Status::Discard => {}
        }
        false
    }
}
