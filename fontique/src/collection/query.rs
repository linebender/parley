// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Query support.

use crate::{Charmap, CharmapIndex};

use super::super::{Collection, SourceCache};

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
    char_fallback_families: Vec<CachedFamily>,
    char_fallback_synced: bool,
}

impl QueryState {
    fn clear(&mut self) {
        self.families.clear();
        self.fallback_families.clear();
        self.fallback_chars.clear();
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
    /// When set, if none of the families or fallback families provide glyph
    /// coverage for these characters, the query is extended with fonts
    /// suggested by the platform for the specific characters and, if
    /// [`exhaustive_fallback`] is enabled, with any font in the collection
    /// that covers them.
    ///
    /// [`exhaustive_fallback`]: crate::CollectionOptions::exhaustive_fallback
    pub fn set_fallback_chars(&mut self, chars: impl IntoIterator<Item = char>) {
        let chars: SmallVec<[char; 8]> = chars.into_iter().collect();
        if self.state.fallback_chars != chars {
            self.state.fallback_chars = chars;
            self.state.char_fallback_families.clear();
            self.state.char_fallback_synced = false;
        }
    }

    /// Invokes the given callback with all fonts that match the current
    /// settings.
    ///
    /// Return [`QueryStatus::Stop`] to end iterating over the matching
    /// fonts or [`QueryStatus::Continue`] to continue iterating.
    pub fn matches_with(&mut self, mut f: impl FnMut(&QueryFont) -> QueryStatus) {
        for family in self
            .state
            .families
            .iter_mut()
            .chain(self.state.fallback_families.iter_mut())
        {
            let (status, _) = visit_family(
                self.collection,
                self.source_cache,
                self.attributes,
                family,
                None,
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
        // search with fonts that cover the specific fallback characters,
        // asking the platform first.
        let locale = self.fallbacks.and_then(|key| key.locale());
        if !self.state.char_fallback_synced {
            let (families, _) = self
                .collection
                .fallback_families_for_chars(&self.state.fallback_chars, locale);
            let mut new_families: SmallVec<[FamilyId; 2]> = SmallVec::new();
            for id in families {
                if !contains_family(&self.state.families, id)
                    && !contains_family(&self.state.fallback_families, id)
                    && !contains_family(&self.state.char_fallback_families, id)
                {
                    new_families.push(id);
                }
            }
            self.state
                .char_fallback_families
                .extend(new_families.into_iter().map(CachedFamily::new));
            self.state.char_fallback_synced = true;
        }
        for family in self.state.char_fallback_families.iter_mut() {
            let (status, _) = visit_family(
                self.collection,
                self.source_cache,
                self.attributes,
                family,
                None,
                &mut f,
            );
            if status == QueryStatus::Stop {
                return;
            }
        }
        // As a last resort, scan the entire collection for a font that
        // covers the fallback characters. The results are cached in the
        // collection so this scan only happens once per set of characters.
        if !self.collection.exhaustive_fallback() {
            return;
        }
        let (_, scanned) = self
            .collection
            .fallback_families_for_chars(&self.state.fallback_chars, locale);
        if scanned {
            return;
        }
        for id in self.collection.all_family_ids_by_name() {
            if contains_family(&self.state.families, id)
                || contains_family(&self.state.fallback_families, id)
                || contains_family(&self.state.char_fallback_families, id)
            {
                continue;
            }
            let mut family = CachedFamily::new(id);
            let (status, matched) = visit_family(
                self.collection,
                self.source_cache,
                self.attributes,
                &mut family,
                Some(&self.state.fallback_chars),
                &mut f,
            );
            if matched {
                self.collection.add_fallback_family_for_chars(
                    &self.state.fallback_chars,
                    locale,
                    id,
                );
                self.state.char_fallback_families.push(family);
            }
            if status == QueryStatus::Stop {
                return;
            }
        }
        self.collection
            .mark_chars_scanned(&self.state.fallback_chars, locale);
    }
}

/// Loads the fonts for a family and invokes the callback with each one,
/// optionally requiring the fonts to provide glyph coverage for a set
/// of characters.
///
/// Returns the resulting status and whether any font was passed to the
/// callback.
fn visit_family(
    collection: &mut Inner,
    source_cache: &mut SourceCache,
    attributes: Attributes,
    family: &mut CachedFamily,
    required_chars: Option<&[char]>,
    f: &mut impl FnMut(&QueryFont) -> QueryStatus,
) -> (QueryStatus, bool) {
    let mut matched = false;
    match &mut family.family {
        Entry::Error => return (QueryStatus::Continue, false),
        Entry::Ok(..) => {}
        status @ Entry::Vacant => {
            if let Some(info) = collection.family(family.id) {
                *status = Entry::Ok(info);
            } else {
                *status = Entry::Error;
                return (QueryStatus::Continue, false);
            }
        }
    }
    let Entry::Ok(family_info) = &family.family else {
        return (QueryStatus::Continue, false);
    };
    let mut best_index = None;
    if let Some(font) = load_font(
        family_info,
        attributes,
        &mut family.best,
        false,
        source_cache,
    ) {
        best_index = Some(font.family.1);
        if covers(font, required_chars) {
            matched = true;
            if f(font) == QueryStatus::Stop {
                return (QueryStatus::Stop, matched);
            }
        }
    }
    // Don't invoke for the default font if it's the same as the
    // best match.
    if best_index == Some(family_info.default_font_index()) {
        return (QueryStatus::Continue, matched);
    }
    if let Some(font) = load_font(
        family_info,
        attributes,
        &mut family.default,
        true,
        source_cache,
    ) && covers(font, required_chars)
    {
        matched = true;
        if f(font) == QueryStatus::Stop {
            return (QueryStatus::Stop, matched);
        }
    }
    (QueryStatus::Continue, matched)
}

/// Returns true if the font provides a glyph for each of the required
/// characters (or if no characters are required).
fn covers(font: &QueryFont, required_chars: Option<&[char]>) -> bool {
    let Some(chars) = required_chars else {
        return true;
    };
    let Some(charmap) = font.charmap() else {
        return false;
    };
    chars
        .iter()
        .all(|&ch| charmap.map(ch).is_some_and(|glyph| glyph != 0))
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
}

impl QueryFont {
    /// Attempts to construct a [Charmap] for this font.
    pub fn charmap(&self) -> Option<Charmap<'_>> {
        self.charmap_index.charmap(self.blob.as_ref())
    }
}

fn load_font<'a>(
    family: &FamilyInfo,
    attributes: Attributes,
    font: &'a mut Entry<QueryFont>,
    is_default: bool,
    source_cache: &mut SourceCache,
) -> Option<&'a QueryFont> {
    match font {
        Entry::Error => None,
        Entry::Ok(font) => Some(font),
        status @ Entry::Vacant => {
            // Set to error in case we fail. This simplifies
            // the following code.
            *status = Entry::Error;
            let family_index = if is_default {
                family.default_font_index()
            } else {
                family.match_index(attributes.width, attributes.style, attributes.weight, true)?
            };
            let font_info = family.fonts().get(family_index)?;
            let blob = font_info.load(Some(source_cache))?;
            let blob_index = font_info.index();
            let synthesis =
                font_info.synthesis(attributes.width, attributes.style, attributes.weight);
            *status = Entry::Ok(QueryFont {
                family: (family.id(), family_index),
                blob: blob.clone(),
                index: blob_index,
                synthesis,
                charmap_index: font_info.charmap_index(),
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
    best: Entry<QueryFont>,
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
