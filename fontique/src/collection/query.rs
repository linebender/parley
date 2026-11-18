// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Query support.

use crate::{Charmap, CharmapIndex, FontId, FontInfo, SourceId, SourceKind, UnicodeRangeSet};

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
    requests: &'a mut HashSet<SourceId>,
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
    pub fn matches_with<'q>(
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
            let mut fonts = bucket.iter();
            let mut default = Some(&mut family.default);
            while let Some(font) = fonts.next().or_else(|| {
                load_font(
                    family_info,
                    self.attributes,
                    default.take()?,
                    self.source_cache,
                )
            }) {
                if font.family_index() == family_info.default_font_index() {
                    default = None;
                }
                if f(font.candidate(family_info, self.requests)) == QueryStatus::Stop {
                    return;
                }
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
        self.matches_with(|candidate| match candidate {
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

/// Candidate generated by a [`Query`].
#[derive(Debug)]
pub enum QueryCandidate<'q, 'a> {
    /// A font that can be used.
    Font(&'q QueryFont),
    /// A font whose data is not available yet.
    ///
    /// This is only yielded for fonts declared with [`Collection::declare_font`]. A consumer
    /// that does not load fonts should [continue](QueryStatus::Continue) with the next font.
    Pending(PendingFont<'a>),
}

/// Candidate font generated by a [`Query`] whose data is not available yet.
///
/// It cannot be used, but its data can be [requested](Self::request).
#[derive(Debug)]
pub struct PendingFont<'a> {
    font: &'a FontInfo,
    requests: &'a mut HashSet<SourceId>,
}

impl PendingFont<'_> {
    /// Returns `true` if the font may be used for the given codepoint once its data is
    /// available, see [`QueryFont::in_unicode_range`].
    #[inline]
    pub fn in_unicode_range(&self, codepoint: impl Into<u32>) -> bool {
        let unicode_range = self.font.unicode_range();
        unicode_range.is_none_or(|set| set.contains(codepoint))
    }

    /// Requests the data of the font. [`Collection::take_font_requests`] returns its source.
    #[inline]
    pub fn request(self) {
        self.requests.insert(self.font.source().id());
    }
}

/// Candidate font generated by a [`Query`].
#[derive(Clone, Debug)]
pub struct QueryFont {
    /// Unique identifier of the font.
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

/// A font of a family as cached by a query.
#[derive(Clone)]
enum CachedFont {
    Loaded(QueryFont),
    /// The index of a pending font in the family font list.
    Pending(usize),
}

impl CachedFont {
    /// Loads the font at `index` in `family`.
    fn load(
        family: &FamilyInfo,
        index: usize,
        attributes: Attributes,
        source_cache: &mut SourceCache,
    ) -> Option<Self> {
        let font_info = family.fonts().get(index)?;
        if matches!(font_info.source().kind(), SourceKind::Pending) {
            return Some(Self::Pending(index));
        }
        Some(Self::Loaded(QueryFont {
            id: font_info.id(),
            family: (family.id(), index),
            blob: font_info.load(Some(source_cache))?,
            index: font_info.index(),
            synthesis: font_info.synthesis(attributes.width, attributes.style, attributes.weight),
            charmap_index: font_info.charmap_index(),
            unicode_range: font_info.unicode_range().cloned(),
        }))
    }

    fn family_index(&self) -> usize {
        match self {
            Self::Loaded(font) => font.family.1,
            Self::Pending(index) => *index,
        }
    }

    #[inline]
    fn candidate<'q, 'a>(
        &'q self,
        family: &'a FamilyInfo,
        requests: &'a mut HashSet<SourceId>,
    ) -> QueryCandidate<'q, 'a> {
        match self {
            Self::Loaded(font) => QueryCandidate::Font(font),
            Self::Pending(index) => QueryCandidate::Pending(PendingFont {
                font: &family.fonts()[*index],
                requests,
            }),
        }
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
            let fonts: SmallVec<[CachedFont; 1]> = indices
                .filter_map(|index| CachedFont::load(family, index, attributes, source_cache))
                .collect();
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
            *status = Entry::Ok(CachedFont::load(
                family,
                family_index,
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
    use alloc::{vec, vec::Vec};
    use linebender_resource_handle::Blob;

    use crate::{Collection, QueryCandidate, QueryStatus, SourceCache};

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
            collection.register_fonts(Blob::new(Arc::new(ROBOTO.to_vec())), Some(info));
        }
        let mut source_cache = SourceCache::default();
        let mut query = collection.query(&mut source_cache);
        query.set_families(["Composite"]);

        // The later font is checked first.
        let mut selected_unrestricted = Vec::new();
        for ch in "a49 ".chars() {
            query.matches_with(|font| {
                let QueryCandidate::Font(font) = font else {
                    unreachable!()
                };
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
    fn pending_fonts() {
        use crate::{CollectionOptions, FontInfoOverride, FontWeight, UnicodeRangeSet};

        const ROBOTO: &[u8] =
            include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");

        // One family, checked in this order: `failed`, `digits`, `any` (all pending), Roboto.
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let roboto = Blob::from(ROBOTO.to_vec());
        let mut info = FontInfoOverride {
            family_name: Some("f"),
            weight: Some(FontWeight::BOLD),
            ..Default::default()
        };
        collection.register_fonts(roboto.clone(), Some(info));
        let (_, any) = collection.declare_font("f", info);
        let range = UnicodeRangeSet::new([0x30..=0x39, 0x660..=0x669]);
        info.unicode_range = Some(&range);
        let (_, digits) = collection.declare_font("f", info);
        let (_, failed) = collection.declare_font("f", info);

        // Returns the index of the font to use for `ch`, and the fonts requested for it.
        let mut source_cache = SourceCache::default();
        let mut select = |collection: &mut Collection, ch| {
            let mut query = collection.query(&mut source_cache);
            query.set_families(["f"]);
            let mut used = None;
            query.matches_with(|font| match font {
                QueryCandidate::Font(font) if font.in_unicode_range(ch) => {
                    used = Some(font.family.1);
                    QueryStatus::Stop
                }
                QueryCandidate::Pending(font) if font.in_unicode_range(ch) => {
                    font.request();
                    QueryStatus::Continue
                }
                _ => QueryStatus::Continue,
            });
            drop(query);
            let mut requests = collection.take_font_requests().collect::<Vec<_>>();
            requests.sort();
            (used, requests)
        };
        // A pending font is yielded in its place, until it is unregistered.
        assert_eq!(
            select(&mut collection, '1'),
            (Some(3), vec![any, digits, failed])
        );
        assert!(collection.unregister_source(failed) && !collection.unregister_source(failed));
        assert_eq!(select(&mut collection, '1'), (Some(2), vec![any, digits]));
        // A supplied font is used in its place, with the attributes and range it was declared with.
        assert!(collection.supply_font(any, roboto.clone()));
        assert!(!collection.supply_font(any, roboto.clone()));
        assert_eq!(select(&mut collection, '1'), (Some(1), vec![digits]));
        assert!(collection.supply_font(digits, roboto));
        assert_eq!(select(&mut collection, '1'), (Some(0), vec![]));
        assert_eq!(select(&mut collection, 'a'), (Some(1), vec![]));
    }

    #[test]
    fn font_ids() {
        use crate::{Attributes, CollectionOptions, FontInfoOverride, FontWeight};
        use alloc::sync::Arc;

        const ROBOTO: &[u8] =
            include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");

        // Two regular fonts that only their identifiers tell apart, and a bold one.
        let mut collection = Collection::new(CollectionOptions {
            shared: true,
            system_fonts: false,
        });
        let ids = [FontWeight::NORMAL, FontWeight::NORMAL, FontWeight::BOLD].map(|weight| {
            let info = FontInfoOverride {
                family_name: Some("Ids"),
                weight: Some(weight),
                ..Default::default()
            };
            let blob = Blob::new(Arc::new(ROBOTO.to_vec()));
            collection.register_fonts(blob, Some(info))[0].1[0].id()
        });

        // A bold query yields the bold font and then the default font of the family. That is
        // the later regular font, until it is unregistered through another handle.
        let mut other = collection.clone();
        for (default, unregistered) in [(ids[1], true), (ids[0], false)] {
            let mut source_cache = SourceCache::default();
            let mut query = collection.query(&mut source_cache);
            query.set_families(["Ids"]);
            query.set_attributes(Attributes {
                weight: FontWeight::BOLD,
                ..Default::default()
            });
            let mut yielded = Vec::new();
            query.matches_with(|font| {
                let QueryCandidate::Font(font) = font else {
                    unreachable!()
                };
                yielded.push(font.id);
                QueryStatus::Continue
            });
            assert_eq!(yielded, [ids[2], default]);
            assert_eq!(other.unregister_font(ids[1]), unregistered);
        }
    }

    #[cfg(feature = "system")]
    #[test]
    fn system_fonts_stay_registered() {
        let mut collection = Collection::default();
        let families: Vec<_> = collection.family_ids().collect();
        let Some(family) = families.into_iter().find_map(|id| collection.family(id)) else {
            return;
        };
        assert!(!collection.unregister_font(family.fonts()[0].id()));
        let fonts_after = collection.family(family.id()).unwrap().fonts().len();
        assert_eq!(fonts_after, family.fonts().len());
    }
}
