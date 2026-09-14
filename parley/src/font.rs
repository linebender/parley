// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fontique::{
    Attributes, CharmapIndex, Collection, FamilyId, FontStyle, Query, QueryFamily, SourceCache,
};
use hashbrown::HashMap;
use parley_engine::{FontInstance, FontMetrics};
use smallvec::SmallVec;

use crate::FontData;
use crate::style::FontVariation;

/// A font database/cache (wrapper around a Fontique [`Collection`] and [`SourceCache`]).
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
///
/// Besides the collection and source cache, this caches the results of font selection across
/// layouts (see [`Self::clear_cache`]).
#[derive(Default, Clone)]
pub struct FontContext {
    pub collection: Collection,
    pub source_cache: SourceCache,
    pub(crate) cache: FontCache,
}

impl FontContext {
    /// Create a new `FontContext`, discovering system fonts if available.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a `FontContext` from an existing collection and source cache.
    pub fn from_parts(collection: Collection, source_cache: SourceCache) -> Self {
        Self {
            collection,
            source_cache,
            cache: FontCache::default(),
        }
    }

    /// Clear the cached results of font selection.
    ///
    /// The cache is invalidated automatically when the [`Collection`] reports a new
    /// [`generation`](Collection::generation), i.e. after fonts are registered or unregistered or
    /// generic families or fallbacks change, so this only needs calling explicitly if fonts are
    /// mutated behind the collection's back (e.g. the data of a registered blob is swapped).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Number of `(first available font, metrics)` entries currently cached.
    pub fn cache_len(&self) -> (usize, usize) {
        (
            self.cache.first_available_fonts.len(),
            self.cache.metrics.len(),
        )
    }

    /// Drop stale cache entries and return the query and the cache with disjoint borrows.
    pub(crate) fn query_and_cache(&mut self) -> (Query<'_>, &mut FontCache) {
        self.cache.sync(&mut self.collection);
        (
            self.collection.query(&mut self.source_cache),
            &mut self.cache,
        )
    }
}

/// Maximum number of entries in either cache before it is emptied.
const MAX_ENTRIES: usize = 1024;

/// Results of font selection that stay valid while the [`Collection`] does not change.
#[derive(Clone, Default)]
pub(crate) struct FontCache {
    generation: Option<u64>,
    first_available_fonts: HashMap<FirstAvailableFontKey, Option<FirstAvailableFont>>,
    metrics: HashMap<MetricsKey, FontMetrics>,
}

impl FontCache {
    fn clear(&mut self) {
        self.generation = None;
        self.first_available_fonts.clear();
        self.metrics.clear();
    }

    /// Empty the cache if `collection` changed since it was last filled.
    fn sync(&mut self, collection: &mut Collection) {
        let generation = collection.generation();
        if self.generation != Some(generation) {
            self.clear();
            self.generation = Some(generation);
        }
    }

    /// The [first available font] of `families` with `attributes`: the first font that
    /// [`Query::matches_with`] yields, or `None` if none of the families has a loadable font. (CSS
    /// additionally requires the font's `unicode-range` to include U+0020 SPACE; Parley has no
    /// `unicode-range`, so every available font qualifies.) Fonts the query
    /// yields from its fallback families are never considered, so the result is independent of
    /// the query's fallback settings.
    ///
    /// [first available font]: https://drafts.csswg.org/css-fonts-4/#first-available-font
    ///
    /// On a miss `query` is used to find the font, leaving its families and attributes set to
    /// `families` and `attributes`; the returned flag is `true` in that case.
    pub(crate) fn first_available_font(
        &mut self,
        query: &mut Query<'_>,
        families: &[FamilyId],
        attributes: Attributes,
    ) -> (Option<FirstAvailableFont>, bool) {
        let key = FirstAvailableFontKey::new(families, attributes);
        if let Some(font) = self.first_available_fonts.get(&key) {
            return (font.clone(), false);
        }
        query.set_families(families.iter().copied().map(QueryFamily::Id));
        query.set_attributes(attributes);
        let mut found = None;
        query.matches_with(|font| {
            if families.contains(&font.family.0) {
                found = Some(FirstAvailableFont {
                    font: FontInstance {
                        font: FontData {
                            data: font.blob.clone(),
                            index: font.index,
                        },
                        synthesis: font.synthesis,
                    },
                    charmap_index: font.charmap_index,
                });
            }
            fontique::QueryStatus::Stop
        });
        if self.first_available_fonts.len() >= MAX_ENTRIES {
            self.first_available_fonts.clear();
        }
        self.first_available_fonts.insert(key, found.clone());
        (found, true)
    }

    /// The metrics of `font` at `font_size` and `variations`, computing them with `compute` on a
    /// miss.
    pub(crate) fn metrics(
        &mut self,
        font: &FontInstance,
        font_size: f32,
        variations: &[FontVariation],
        compute: impl FnOnce() -> Option<FontMetrics>,
    ) -> Option<FontMetrics> {
        let key = MetricsKey::new(font, font_size, variations);
        if let Some(metrics) = self.metrics.get(&key) {
            return Some(*metrics);
        }
        let metrics = compute()?;
        if self.metrics.len() >= MAX_ENTRIES {
            self.metrics.clear();
        }
        self.metrics.insert(key, metrics);
        Some(metrics)
    }
}

/// A cached font selection result.
#[derive(Clone)]
pub(crate) struct FirstAvailableFont {
    pub(crate) font: FontInstance,
    pub(crate) charmap_index: CharmapIndex,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FirstAvailableFontKey {
    families: SmallVec<[FamilyId; 4]>,
    width: u32,
    weight: u32,
    /// `FontStyle` discriminant and oblique angle bits.
    style: (u8, u32),
}

impl FirstAvailableFontKey {
    fn new(families: &[FamilyId], attributes: Attributes) -> Self {
        let style = match attributes.style {
            FontStyle::Normal => (0, 0),
            FontStyle::Italic => (1, 0),
            FontStyle::Oblique(None) => (2, 0),
            FontStyle::Oblique(Some(angle)) => (3, angle.to_bits()),
        };
        Self {
            families: families.iter().copied().collect(),
            width: attributes.width.ratio().to_bits(),
            weight: attributes.weight.value().to_bits(),
            style,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct MetricsKey {
    blob: u64,
    index: u32,
    size: u32,
    /// Synthesis variation settings followed by the style's variations, as `(tag, value bits)`.
    variations: SmallVec<[(u32, u32); 4]>,
}

impl MetricsKey {
    fn new(font: &FontInstance, font_size: f32, variations: &[FontVariation]) -> Self {
        Self {
            blob: font.font.data.id(),
            index: font.font.index,
            size: font_size.to_bits(),
            variations: font
                .synthesis
                .variation_settings()
                .iter()
                .map(|(tag, value)| (u32::from_be_bytes(tag.to_be_bytes()), value.to_bits()))
                .chain(
                    variations
                        .iter()
                        .map(|v| (u32::from_be_bytes(v.tag.to_bytes()), v.value.to_bits())),
                )
                .collect(),
        }
    }
}
