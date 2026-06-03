// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lookup keys for the shaping caches.
//!
//! Each cache stores an owned key (`*Id`) and is queried with a borrowed key (`*Key`),
//! so a cache hit avoids allocating an owned key. The borrowed key implements
//! [`Equivalent`] for the comparison and `From`/`Into` for materializing the owned key
//! on a miss; this is the contract [`LruCache::entry`](super::lru_cache::LruCache::entry)
//! requires of its lookup keys.

use alloc::boxed::Box;

use fontique::Synthesis;
use harfrust::{Direction, Feature, Language, Script};
use hashbrown::Equivalent;
use parlance::FontVariation;

use crate::common::NormalizedCoord;

/// Key for the [`ShaperData`](harfrust::ShaperData) cache: the font blob's id and
/// its index within a font collection. Doubles as its own lookup key.
#[derive(PartialEq, Copy, Clone)]
pub(super) struct ShapeDataKey {
    /// The font collection's blob ID.
    font_blob_id: u64,
    /// The font's index in the font collection.
    font_index: u32,
}

impl ShapeDataKey {
    pub(super) const fn new(font_blob_id: u64, font_index: u32) -> Self {
        Self {
            font_blob_id,
            font_index,
        }
    }
}

impl Equivalent<Self> for ShapeDataKey {
    #[inline(always)]
    fn equivalent(&self, key: &Self) -> bool {
        self == key
    }
}

impl From<&Self> for ShapeDataKey {
    #[inline(always)]
    fn from(key: &Self) -> Self {
        *key
    }
}

/// Stored key for the [`ShaperInstance`](harfrust::ShaperInstance) cache.
pub(super) struct ShapeInstanceId {
    /// The font collection's blob ID.
    font_blob_id: u64,
    /// The font's index in the font collection.
    font_index: u32,
    synthesis: Synthesis,
    variations: Box<[FontVariation]>,
}

/// Borrowed lookup key for the [`ShaperInstance`](harfrust::ShaperInstance) cache.
pub(super) struct ShapeInstanceKey<'a> {
    /// The font collection's blob ID.
    font_blob_id: u64,
    /// The font's index in the font collection.
    font_index: u32,
    synthesis: &'a Synthesis,
    variations: &'a [FontVariation],
}

impl<'a> ShapeInstanceKey<'a> {
    pub(super) fn new(
        font_blob_id: u64,
        font_index: u32,
        synthesis: &'a Synthesis,
        variations: &'a [FontVariation],
    ) -> Self {
        Self {
            font_blob_id,
            font_index,
            synthesis,
            variations,
        }
    }
}

impl Equivalent<ShapeInstanceId> for ShapeInstanceKey<'_> {
    fn equivalent(&self, key: &ShapeInstanceId) -> bool {
        self.font_blob_id == key.font_blob_id
            && self.font_index == key.font_index
            && *self.synthesis == key.synthesis
            && self.variations == &*key.variations
    }
}

impl<'a> From<ShapeInstanceKey<'a>> for ShapeInstanceId {
    fn from(key: ShapeInstanceKey<'a>) -> Self {
        Self {
            font_blob_id: key.font_blob_id,
            font_index: key.font_index,
            synthesis: *key.synthesis,
            variations: key.variations.into(),
        }
    }
}

/// Stored key for the [`ShapePlan`](harfrust::ShapePlan) cache.
pub(super) struct ShapePlanId {
    /// The font collection's blob ID.
    font_blob_id: u64,
    /// The font's index in the font collection.
    font_index: u32,
    coords: Box<[NormalizedCoord]>,
    direction: Direction,
    script: Script,
    language: Option<Language>,
    features: Box<[Feature]>,
}

/// Borrowed lookup key for the [`ShapePlan`](harfrust::ShapePlan) cache.
///
/// The plan is keyed on the resolved *normalized* coords rather than the user-space
/// variations (and synthesis) `harfrust` derives them from: the coords are what
/// actually determine the plan's feature-variation selection, and they are all the
/// re-shape path retains, so both the initial-shape and re-shape paths share entries.
pub(super) struct ShapePlanKey<'a> {
    /// The font collection's blob ID.
    font_blob_id: u64,
    /// The font's index in the font collection.
    font_index: u32,
    coords: &'a [NormalizedCoord],
    direction: Direction,
    script: Script,
    language: Option<&'a Language>,
    features: &'a [Feature],
}

impl<'a> ShapePlanKey<'a> {
    pub(super) fn new(
        font_blob_id: u64,
        font_index: u32,
        coords: &'a [NormalizedCoord],
        direction: Direction,
        script: Script,
        language: Option<&'a Language>,
        features: &'a [Feature],
    ) -> Self {
        Self {
            font_blob_id,
            font_index,
            coords,
            direction,
            script,
            language,
            features,
        }
    }
}

impl Equivalent<ShapePlanId> for ShapePlanKey<'_> {
    fn equivalent(&self, key: &ShapePlanId) -> bool {
        self.font_blob_id == key.font_blob_id
            && self.font_index == key.font_index
            && self.direction == key.direction
            && self.script == key.script
            && self.language == key.language.as_ref()
            && self.coords == &*key.coords
            && self.features == &*key.features
    }
}

impl<'a> From<ShapePlanKey<'a>> for ShapePlanId {
    fn from(key: ShapePlanKey<'a>) -> Self {
        Self {
            font_blob_id: key.font_blob_id,
            font_index: key.font_index,
            coords: key.coords.into(),
            direction: key.direction,
            script: key.script,
            language: key.language.cloned(),
            features: key.features.into(),
        }
    }
}
