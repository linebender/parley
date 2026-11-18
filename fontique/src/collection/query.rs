// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Query support.

use crate::{Charmap, CharmapIndex, UnicodeRangeSet};

use super::super::{Collection, SourceCache, matching::match_fonts};

use alloc::vec::Vec;
use smallvec::SmallVec;

use super::{
    super::{Attributes, Blob, FallbackKey, FamilyId, FamilyInfo, GenericFamily, Synthesis},
    Inner,
};

#[derive(Clone, Default)]
pub(super) struct QueryState {
    families: Vec<CachedFamily>,
    fallback_families: Vec<CachedFamily>,
    fallback_chars: SmallVec<[char; 8]>,
    /// Scratch buffer used to compare incoming fallback characters against
    /// `fallback_chars` without allocating.
    scratch_chars: SmallVec<[char; 8]>,
    char_fallback_families: Vec<CachedFamily>,
    char_fallback_synced: bool,
}

impl QueryState {
    fn clear(&mut self) {
        self.families.clear();
        self.fallback_families.clear();
        self.fallback_chars.clear();
        self.scratch_chars.clear();
        self.char_fallback_families.clear();
        self.char_fallback_synced = false;
    }
}

/// State for font selection.
///
/// Instances of this can be obtained from [`Collection::query`].
pub struct Query<'a> {
    collection: &'a mut Inner,
    state: &'a mut QueryState,
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
            for family in &mut self.state.char_fallback_families {
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
            self.state.char_fallback_families.clear();
            self.state.char_fallback_synced = false;
            self.fallbacks = Some(key);
        }
    }

    /// Sets the characters that fallback fonts are expected to cover.
    ///
    /// When set, after the families and fallback families have been
    /// visited, the query is extended with the fonts suggested by the
    /// platform for these specific characters.
    pub fn set_fallback_chars(&mut self, chars: impl IntoIterator<Item = char>) {
        self.state.scratch_chars.clear();
        self.state.scratch_chars.extend(chars);
        if self.state.fallback_chars != self.state.scratch_chars {
            core::mem::swap(
                &mut self.state.fallback_chars,
                &mut self.state.scratch_chars,
            );
            self.state.char_fallback_families.clear();
            self.state.char_fallback_synced = false;
        }
    }

    /// Invokes the given callback with all fonts that match the current
    /// settings.
    ///
    /// Return [`QueryStatus::Stop`] to end iterating over the matching
    /// fonts or [`QueryStatus::Continue`] to continue iterating.
    pub fn matches_with<'q>(&'q mut self, mut f: impl FnMut(&'q QueryFont) -> QueryStatus) {
        // Fonts yielded to `f` borrow the family lists for `'q`, so note which
        // families are already queried before visiting them.
        let mut queried_ids = SmallVec::<[FamilyId; 16]>::new();
        if !self.state.fallback_chars.is_empty() && !self.state.char_fallback_synced {
            queried_ids.extend(
                self.state
                    .families
                    .iter()
                    .chain(self.state.fallback_families.iter())
                    .map(|family| family.id),
            );
        }
        for family in self
            .state
            .families
            .iter_mut()
            .chain(self.state.fallback_families.iter_mut())
        {
            let status = visit_family(
                self.collection,
                self.source_cache,
                self.attributes,
                family,
                &mut f,
            );
            if status == QueryStatus::Stop {
                return;
            }
        }
        if self.state.fallback_chars.is_empty() {
            return;
        }
        // None of the requested or fallback families matched, so extend the
        // search with the families the platform suggests for the specific
        // fallback characters.
        if !self.state.char_fallback_synced {
            let locale = self.fallbacks.and_then(|key| key.locale());
            let families = self
                .collection
                .fallback_families_for_chars(&self.state.fallback_chars, locale);
            for id in families {
                if !queried_ids.contains(&id)
                    && !contains_family(&self.state.char_fallback_families, id)
                {
                    self.state
                        .char_fallback_families
                        .push(CachedFamily::new(id));
                }
            }
            self.state.char_fallback_synced = true;
        }
        for family in self.state.char_fallback_families.iter_mut() {
            let status = visit_family(
                self.collection,
                self.source_cache,
                self.attributes,
                family,
                &mut f,
            );
            if status == QueryStatus::Stop {
                return;
            }
        }
    }

    /// Returns the [first available font] for the current settings: the first
    /// font that [`matches_with`](Self::matches_with) would yield, if any.
    ///
    /// As in CSS, fonts whose [unicode range] excludes U+0020 SPACE are skipped.
    ///
    /// [first available font]: https://drafts.csswg.org/css-fonts-4/#first-available-font
    /// [unicode range]: crate::FontInfoOverride::unicode_range
    pub fn first_available_font(&mut self) -> Option<QueryFont> {
        let mut found = None;
        self.matches_with(|font| {
            if !font.in_unicode_range(' ') {
                return QueryStatus::Continue;
            }
            found = Some(font.clone());
            QueryStatus::Stop
        });
        found
    }
}

/// Loads the fonts for a family and invokes the callback with each one.
fn visit_family<'q>(
    collection: &mut Inner,
    source_cache: &mut SourceCache,
    attributes: Attributes,
    family: &'q mut CachedFamily,
    f: &mut impl FnMut(&'q QueryFont) -> QueryStatus,
) -> QueryStatus {
    match &mut family.family {
        Entry::Error => return QueryStatus::Continue,
        Entry::Ok(..) => {}
        status @ Entry::Vacant => {
            if let Some(info) = collection.family(family.id) {
                *status = Entry::Ok(info);
            } else {
                *status = Entry::Error;
                return QueryStatus::Continue;
            }
        }
    }
    let Entry::Ok(family_info) = &family.family else {
        return QueryStatus::Continue;
    };
    let bucket = load_bucket(family_info, attributes, &mut family.best, source_cache);
    let mut yielded_default = false;
    for font in bucket {
        if font.family.1 == family_info.default_font_index() {
            yielded_default = true;
        }
        if f(font) == QueryStatus::Stop {
            return QueryStatus::Stop;
        }
    }
    if yielded_default {
        return QueryStatus::Continue;
    }
    if let Some(font) = load_font(family_info, attributes, &mut family.default, source_cache)
        && f(font) == QueryStatus::Stop
    {
        return QueryStatus::Stop;
    }
    QueryStatus::Continue
}

fn contains_family(families: &[CachedFamily], id: FamilyId) -> bool {
    families.iter().any(|family| family.id == id)
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

/// Loads every font in `family` matching `attributes` (all coverage variants of the best match).
fn load_bucket<'a>(
    family: &FamilyInfo,
    attributes: Attributes,
    bucket: &'a mut Entry<SmallVec<[QueryFont; 1]>>,
    source_cache: &mut SourceCache,
) -> &'a [QueryFont] {
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
            let mut fonts: SmallVec<[QueryFont; 1]> = SmallVec::new();
            for index in indices {
                let Some(font_info) = family.fonts().get(index) else {
                    continue;
                };
                let Some(blob) = font_info.load(Some(source_cache)) else {
                    continue;
                };
                fonts.push(QueryFont {
                    family: (family.id(), index),
                    blob: blob.clone(),
                    index: font_info.index(),
                    synthesis: font_info.synthesis(
                        attributes.width,
                        attributes.style,
                        attributes.weight,
                    ),
                    charmap_index: font_info.charmap_index(),
                    unicode_range: font_info.unicode_range().cloned(),
                });
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
    font: &'a mut Entry<QueryFont>,
    source_cache: &mut SourceCache,
) -> Option<&'a QueryFont> {
    match font {
        Entry::Error => None,
        Entry::Ok(font) => Some(font),
        status @ Entry::Vacant => {
            // Set to error in case we fail. This simplifies
            // the following code.
            *status = Entry::Error;
            let family_index = family.default_font_index();
            let font_info = family.fonts().get(family_index)?;
            let blob = font_info.load(Some(source_cache))?;
            let blob_index = font_info.index();
            let synthesis =
                font_info.synthesis(attributes.width, attributes.style, attributes.weight);
            *status = Entry::Ok(QueryFont {
                family: (family.id(), family_index),
                blob,
                index: blob_index,
                synthesis,
                charmap_index: font_info.charmap_index(),
                unicode_range: font_info.unicode_range().cloned(),
            });
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
    best: Entry<SmallVec<[QueryFont; 1]>>,
    default: Entry<QueryFont>,
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
            collection.register_fonts(Blob::new(Arc::new(ROBOTO.to_vec())), Some(info));
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
}
