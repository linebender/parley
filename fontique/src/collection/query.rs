// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Query support.

use crate::{Charmap, CharmapIndex, FontId, FontInfo, PendingFontStatus, UnicodeRangeSet};

use super::super::{Collection, SourceCache, matching::match_fonts};

use alloc::vec::Vec;
use hashbrown::HashSet;
use parlance::Script;
use smallvec::SmallVec;

use super::{
    super::{Attributes, Blob, FallbackKey, FamilyId, FamilyInfo, GenericFamily, Synthesis},
    Inner,
};

#[derive(Clone, Default)]
pub(super) struct QueryState {
    families: Vec<CachedFamily>,
    fallback_families: Vec<CachedFamily>,
}

impl QueryState {
    fn clear(&mut self) {
        self.families.clear();
        self.fallback_families.clear();
    }
}

/// State for font selection.
///
/// Instances of this can be obtained from [`Collection::query`].
pub struct Query<'a> {
    collection: &'a mut Inner,
    state: &'a mut QueryState,
    requests: &'a mut HashSet<FontId>,
    source_cache: &'a mut SourceCache,
    attributes: Attributes,
    fallbacks: Option<FallbackKey>,
}

impl<'a> Query<'a> {
    pub(super) fn new(collection: &'a mut Collection, source_cache: &'a mut SourceCache) -> Self {
        collection.query_state.clear();
        Self {
            collection: &mut collection.inner,
            state: &mut collection.query_state,
            requests: &mut collection.font_requests,
            source_cache,
            attributes: Attributes::default(),
            fallbacks: None,
        }
    }

    /// Sets the ordered sequence of families to match against.
    pub fn set_families<'f, I>(&mut self, families: I)
    where
        I: IntoIterator,
        I::Item: Into<QueryFamily<'f>>,
    {
        self.state.families.clear();
        for family in families {
            let family = family.into();
            match family {
                QueryFamily::Named(name) => {
                    if let Some(id) = self.collection.family_id(name) {
                        self.state.families.push(CachedFamily::new(id));
                    }
                }
                QueryFamily::Id(id) => {
                    self.state.families.push(CachedFamily::new(id));
                }
                QueryFamily::Generic(generic) => {
                    for id in self.collection.generic_families(generic) {
                        self.state.families.push(CachedFamily::new(id));
                    }
                }
            }
        }
    }

    /// Sets the primary attributes to match against.
    pub fn set_attributes(&mut self, attributes: Attributes) {
        if self.attributes != attributes {
            for family in &mut self.state.families {
                family.clear_fonts();
            }
            for family in &mut self.state.fallback_families {
                family.clear_fonts();
            }
            self.attributes = attributes;
        }
    }

    /// Sets the script and locale for fallback fonts.
    pub fn set_fallbacks(&mut self, key: impl Into<FallbackKey>) {
        let key = key.into();
        if self.fallbacks != Some(key) {
            self.state.fallback_families.clear();
            self.state.fallback_families.extend(
                self.collection
                    .fallback_families(key)
                    .map(CachedFamily::new),
            );
            // HACK: always add a Han font to the fallback list to capture
            // punctuation in the common script
            // See <https://github.com/linebender/parley/issues/597>
            self.state.fallback_families.extend(
                self.collection
                    .fallback_families(FallbackKey::new(Script::from_bytes(*b"Hani"), None))
                    .map(CachedFamily::new),
            );
            self.fallbacks = Some(key);
        }
    }

    /// Invokes the given callback with all fonts that match the current
    /// settings.
    ///
    /// Return [`QueryStatus::Stop`] to end iterating over the matching
    /// fonts or [`QueryStatus::Continue`] to continue iterating.
    pub fn matches_with<'q>(&'q mut self, mut f: impl FnMut(&'q QueryFont) -> QueryStatus) {
        self.matches_with_pending(|candidate| match candidate {
            QueryCandidate::Font(font) => f(font),
            QueryCandidate::Pending(_) => QueryStatus::Continue,
        });
    }

    /// Like [`matches_with`](Self::matches_with), but also yields the fonts whose data
    /// is not available yet, in their place in the matching order.
    pub fn matches_with_pending<'q>(
        &'q mut self,
        mut f: impl FnMut(QueryCandidate<'q, '_>) -> QueryStatus,
    ) {
        for family in self
            .state
            .families
            .iter_mut()
            .chain(self.state.fallback_families.iter_mut())
        {
            match &mut family.family {
                Entry::Error => continue,
                Entry::Ok(..) => {}
                status @ Entry::Vacant => {
                    if let Some(info) = self.collection.family(family.id) {
                        *status = Entry::Ok(info);
                    } else {
                        *status = Entry::Error;
                        continue;
                    }
                }
            }
            let Entry::Ok(family_info) = &family.family else {
                continue;
            };
            let bucket = load_bucket(
                family_info,
                self.attributes,
                &mut family.best,
                self.source_cache,
            );
            let mut yielded_default = false;
            for font in bucket {
                if font.family_index() == family_info.default_font_index() {
                    yielded_default = true;
                }
                if f(font.as_candidate(self.requests)) == QueryStatus::Stop {
                    return;
                }
            }
            if yielded_default {
                continue;
            }
            if let Some(font) = load_font(
                family_info,
                self.attributes,
                &mut family.default,
                self.source_cache,
            ) && f(font.as_candidate(self.requests)) == QueryStatus::Stop
            {
                return;
            }
        }
    }

    /// Returns the [first available font] for the current settings: the first
    /// font that [`matches_with`](Self::matches_with) would yield, if any.
    ///
    /// As in CSS, fonts whose [unicode range] excludes U+0020 SPACE are skipped. So are
    /// [pending](QueryCandidate::Pending) fonts, the first of which is
    /// [requested](PendingFont::request).
    ///
    /// [first available font]: https://drafts.csswg.org/css-fonts-4/#first-available-font
    /// [unicode range]: crate::FontInfoOverride::unicode_range
    pub fn first_available_font(&mut self) -> Option<QueryFont> {
        let mut found = None;
        let mut requested = false;
        self.matches_with_pending(|candidate| match candidate {
            QueryCandidate::Font(font) if font.in_unicode_range(' ') => {
                found = Some(font.clone());
                QueryStatus::Stop
            }
            QueryCandidate::Pending(font) if !requested && font.in_unicode_range(' ') => {
                requested = true;
                font.request();
                QueryStatus::Continue
            }
            _ => QueryStatus::Continue,
        });
        found
    }
}

/// Candidate generated by [`Query::matches_with_pending`].
#[derive(Debug)]
pub enum QueryCandidate<'q, 'r> {
    /// A font with loaded data.
    Font(&'q QueryFont),
    /// A font whose data is not available yet and has not failed to load.
    Pending(PendingFont<'q, 'r>),
}

/// Candidate font generated by a [`Query`] whose data is not available yet.
///
/// It cannot be used, but its data can be [requested](Self::request).
#[derive(Debug)]
pub struct PendingFont<'q, 'r> {
    id: FontId,
    status: PendingFontStatus,
    unicode_range: Option<&'q UnicodeRangeSet>,
    requests: &'r mut HashSet<FontId>,
}

impl PendingFont<'_, '_> {
    /// Returns the identifier the font was registered with.
    pub fn id(&self) -> FontId {
        self.id
    }

    /// Returns [`PendingFontStatus::Unloaded`] or [`PendingFontStatus::Loading`].
    pub fn status(&self) -> PendingFontStatus {
        self.status
    }

    /// Returns `true` if the font may be used for the given codepoint once its data is
    /// available, see [`QueryFont::in_unicode_range`].
    #[inline]
    pub fn in_unicode_range(&self, codepoint: impl Into<u32>) -> bool {
        self.unicode_range.is_none_or(|set| set.contains(codepoint))
    }

    /// Requests the data of the font: [`Collection::take_font_requests`] returns its
    /// identifier. Does nothing for a font that is already
    /// [loading](PendingFontStatus::Loading).
    #[inline]
    pub fn request(self) {
        if self.status == PendingFontStatus::Unloaded {
            self.requests.insert(self.id);
        }
    }
}

/// A query cache entry: a loaded font or a descriptor-only face.
#[derive(Clone)]
enum CachedFont {
    Loaded(QueryFont),
    Pending {
        id: FontId,
        status: PendingFontStatus,
        family_index: usize,
        unicode_range: Option<UnicodeRangeSet>,
    },
}

impl CachedFont {
    fn load(
        family: FamilyId,
        family_index: usize,
        info: &FontInfo,
        attributes: Attributes,
        cache: &mut SourceCache,
    ) -> Option<Self> {
        if let Some(pending) = info.pending() {
            // A font that failed to load is never used.
            let status = pending.status();
            return (status != PendingFontStatus::Failed).then(|| Self::Pending {
                id: info.id(),
                status,
                family_index,
                unicode_range: info.unicode_range().cloned(),
            });
        }
        QueryFont::load(family, family_index, info, attributes, cache).map(Self::Loaded)
    }

    fn family_index(&self) -> usize {
        match self {
            Self::Loaded(font) => font.family.1,
            Self::Pending { family_index, .. } => *family_index,
        }
    }

    #[inline]
    fn as_candidate<'q, 'r>(&'q self, requests: &'r mut HashSet<FontId>) -> QueryCandidate<'q, 'r> {
        match self {
            Self::Loaded(font) => QueryCandidate::Font(font),
            Self::Pending {
                id,
                status,
                unicode_range,
                ..
            } => QueryCandidate::Pending(PendingFont {
                id: *id,
                status: *status,
                unicode_range: unicode_range.as_ref(),
                requests,
            }),
        }
    }
}

impl Drop for Query<'_> {
    fn drop(&mut self) {
        self.state.clear();
    }
}

/// Determines whether a font query operation will continue.
///
/// See [`Query::matches_with`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum QueryStatus {
    /// Query should continue with the next font.
    Continue,
    /// Query should stop.
    Stop,
}

/// Family descriptor for a font query.
///
/// This allows [`Query::set_families`] to
/// take a variety of family types.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum QueryFamily<'a> {
    /// A named font family.
    Named(&'a str),
    /// An identifier for a font family.
    Id(FamilyId),
    /// A generic font family.
    Generic(GenericFamily),
}

impl<'a> From<&'a str> for QueryFamily<'a> {
    fn from(value: &'a str) -> Self {
        Self::Named(value)
    }
}

impl From<FamilyId> for QueryFamily<'static> {
    fn from(value: FamilyId) -> Self {
        Self::Id(value)
    }
}

impl From<GenericFamily> for QueryFamily<'static> {
    fn from(value: GenericFamily) -> Self {
        Self::Generic(value)
    }
}

/// Candidate font generated by a [`Query`].
#[derive(Clone, Debug)]
pub struct QueryFont {
    /// Unique identifier of the font, see [`Collection::font`](crate::Collection::font).
    pub id: FontId,
    /// Family identifier and index of the font in the family font list.
    pub family: (FamilyId, usize),
    /// Blob containing the font data.
    pub blob: Blob<u8>,
    /// Index of a font in a font collection (`ttc`) file.
    pub index: u32,
    /// Synthesis suggestions for this font based on the requested attributes.
    pub synthesis: Synthesis,
    /// Data used for constructing a character map for this font.
    pub charmap_index: CharmapIndex,
    /// The codepoints the font may be used for.
    ///
    /// See [`FontInfoOverride::unicode_range`](crate::FontInfoOverride::unicode_range).
    pub unicode_range: Option<UnicodeRangeSet>,
}

impl QueryFont {
    fn load(
        family: FamilyId,
        family_index: usize,
        info: &FontInfo,
        attributes: Attributes,
        cache: &mut SourceCache,
    ) -> Option<Self> {
        let blob = info.load(Some(cache))?;
        Some(Self {
            id: info.id(),
            family: (family, family_index),
            blob,
            index: info.index(),
            synthesis: info.synthesis(attributes.width, attributes.style, attributes.weight),
            charmap_index: info.charmap_index(),
            unicode_range: info.unicode_range().cloned(),
        })
    }

    /// Attempts to construct a [Charmap] for this font.
    ///
    /// This maps all codepoints the font has glyphs for, including those outside its
    /// [unicode range](Self::unicode_range).
    #[inline]
    pub fn charmap(&self) -> Option<Charmap<'_>> {
        self.charmap_index.charmap(self.blob.as_ref())
    }

    /// Returns `true` if the font may be used for the given codepoint, see
    /// [`FontInfoOverride::unicode_range`](crate::FontInfoOverride::unicode_range).
    ///
    /// This doesn't mean the font has a glyph for it.
    #[inline]
    pub fn in_unicode_range(&self, codepoint: impl Into<u32>) -> bool {
        let unicode_range = self.unicode_range.as_ref();
        unicode_range.is_none_or(|set| set.contains(codepoint))
    }
}

/// Loads every font in `family` matching `attributes` (all coverage variants of the best match).
fn load_bucket<'a>(
    family: &FamilyInfo,
    attributes: Attributes,
    bucket: &'a mut Entry<SmallVec<[CachedFont; 1]>>,
    source_cache: &mut SourceCache,
) -> &'a [CachedFont] {
    match bucket {
        Entry::Error => &[],
        Entry::Ok(fonts) => fonts,
        status @ Entry::Vacant => {
            *status = Entry::Error;
            let indices = match_fonts(
                family.fonts(),
                attributes.width,
                attributes.style,
                attributes.weight,
                true,
            );
            let mut fonts: SmallVec<[CachedFont; 1]> = SmallVec::new();
            for index in indices {
                let Some(font_info) = family.fonts().get(index) else {
                    continue;
                };
                if let Some(font) =
                    CachedFont::load(family.id(), index, font_info, attributes, source_cache)
                {
                    fonts.push(font);
                }
            }
            if fonts.is_empty() {
                return &[];
            }
            *status = Entry::Ok(fonts);
            let Entry::Ok(fonts) = status else {
                unreachable!()
            };
            fonts
        }
    }
}

fn load_font<'a>(
    family: &FamilyInfo,
    attributes: Attributes,
    font: &'a mut Entry<CachedFont>,
    source_cache: &mut SourceCache,
) -> Option<&'a CachedFont> {
    match font {
        Entry::Error => None,
        Entry::Ok(font) => Some(font),
        status @ Entry::Vacant => {
            // Set to error in case we fail. This simplifies
            // the following code.
            *status = Entry::Error;
            let family_index = family.default_font_index();
            let font_info = family.fonts().get(family_index)?;
            *status = Entry::Ok(CachedFont::load(
                family.id(),
                family_index,
                font_info,
                attributes,
                source_cache,
            )?);
            if let Entry::Ok(font) = status {
                Some(font)
            } else {
                None
            }
        }
    }
}

#[derive(Clone)]
struct CachedFamily {
    id: FamilyId,
    family: Entry<FamilyInfo>,
    best: Entry<SmallVec<[CachedFont; 1]>>,
    default: Entry<CachedFont>,
}

impl CachedFamily {
    fn new(id: FamilyId) -> Self {
        Self {
            id,
            family: Entry::Vacant,
            best: Entry::Vacant,
            default: Entry::Vacant,
        }
    }

    fn clear_fonts(&mut self) {
        self.best = Entry::Vacant;
        self.default = Entry::Vacant;
    }
}

#[derive(Clone)]
enum Entry<T> {
    Ok(T),
    Vacant,
    Error,
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use linebender_resource_handle::Blob;

    use crate::{Collection, QueryStatus, SourceCache};

    #[test]
    fn unicode_range() {
        use crate::{CollectionOptions, FontInfoOverride, UnicodeRangeSet};
        use alloc::sync::Arc;

        const ROBOTO: &[u8] =
            include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");

        // One family of two fonts: an unrestricted one, and a later one for digits.
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let digits = UnicodeRangeSet::new([0x35..=0x39, 0x30..=0x35, 0x32..=0x33, 0x660..=0x669]);
        for unicode_range in [None, Some(&digits)] {
            let info = FontInfoOverride {
                family_name: Some("Composite"),
                unicode_range,
                ..Default::default()
            };
            collection.register_font(Blob::new(Arc::new(ROBOTO.to_vec())), 0, Some(info));
        }
        let mut source_cache = SourceCache::default();
        let mut query = collection.query(&mut source_cache);
        query.set_families(["Composite"]);

        // The later font is checked first.
        let mut selected_unrestricted = Vec::new();
        for ch in "a49 ".chars() {
            query.matches_with(|font| {
                if !font.in_unicode_range(ch) || font.charmap().unwrap().map(ch).is_none() {
                    return QueryStatus::Continue;
                }
                selected_unrestricted.push(font.in_unicode_range('a'));
                QueryStatus::Stop
            });
        }
        assert_eq!(selected_unrestricted, [true, false, false, true]);
        assert!(query.first_available_font().unwrap().in_unicode_range('a'));
    }

    #[test]
    fn font_ids() {
        use crate::{CollectionOptions, FontInfoOverride};
        use alloc::sync::Arc;

        const ROBOTO: &[u8] =
            include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");

        for shared in [false, true] {
            let mut collection = Collection::new(CollectionOptions {
                shared,
                system_fonts: false,
            });
            let mut ids = Vec::new();
            for _ in 0..2 {
                let info = FontInfoOverride {
                    family_name: Some("Ids"),
                    ..Default::default()
                };
                let id = crate::FontId::new();
                let blob = Blob::new(Arc::new(ROBOTO.to_vec()));
                let (_, font) = collection
                    .register_font_with_id(id, blob.clone(), 0, Some(info))
                    .unwrap();
                assert_eq!(font.id(), id);
                // An identifier names one font, and a file holds only the fonts it holds.
                assert!(
                    collection
                        .register_font_with_id(id, blob.clone(), 0, Some(info))
                        .is_none()
                );
                assert!(collection.register_font(blob, 1, Some(info)).is_none());
                ids.push(id);
            }
            let family = collection.family_id("Ids").unwrap();
            assert_ne!(ids[0], ids[1]);
            for id in &ids {
                assert_eq!(collection.font(*id).unwrap().id(), *id);
                assert_eq!(collection.font_family_id(*id), Some(family));
            }
            // A clone of the collection knows the same fonts by the same identifiers.
            assert!(collection.clone().font(ids[0]).is_some());

            // Queries report the identifier; the later font is checked first.
            let mut source_cache = SourceCache::default();
            let mut query = collection.query(&mut source_cache);
            query.set_families(["Ids"]);
            let mut matched = None;
            query.matches_with(|font| {
                matched = Some(font.id);
                QueryStatus::Stop
            });
            assert_eq!(matched, Some(ids[1]));
            drop(query);

            // The two fonts have the same attributes, so only the identifier tells
            // them apart.
            assert!(collection.unregister_font(ids[1]));
            assert!(!collection.unregister_font(ids[1]));
            assert!(collection.font(ids[1]).is_none());
            assert_eq!(collection.font_family_id(ids[1]), None);
            let fonts = collection.family(family).unwrap();
            assert_eq!(fonts.fonts().len(), 1);
            assert_eq!(fonts.fonts()[0].id(), ids[0]);

            collection.clear();
            assert!(collection.font(ids[0]).is_none());

            // Every font in a file, each with a new identifier.
            let all = collection.register_all_fonts(Blob::new(Arc::new(ROBOTO.to_vec())), None);
            assert_eq!(all.len(), 1);
            assert!(!ids.contains(&all[0].1.id()));
            assert_eq!(collection.font_family_id(all[0].1.id()), Some(all[0].0));
        }
    }
}
