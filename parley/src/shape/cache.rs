// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::FontVariation;
use alloc::boxed::Box;
use hashbrown::Equivalent;

#[derive(PartialEq, Copy, Clone)]
pub(crate) struct ShapeDataKey {
    /// The font collection's blob ID.
    font_blob_id: u64,
    /// The font's index in the font collection.
    font_index: u32,
}

impl ShapeDataKey {
    pub(crate) const fn new(font_blob_id: u64, font_index: u32) -> Self {
        Self {
            font_blob_id,
            font_index,
        }
    }
}

impl Equivalent<ShapeDataKey> for ShapeDataKey {
    #[inline(always)]
    fn equivalent(&self, key: &ShapeDataKey) -> bool {
        self == key
    }
}

impl Into<ShapeDataKey> for &ShapeDataKey {
    #[inline(always)]
    fn into(self) -> ShapeDataKey {
        *self
    }
}

pub(crate) struct ShapeInstanceId {
    font_blob_id: u64,
    font_index: u32,
    synthesis: fontique::Synthesis,
    variations: Option<Box<[FontVariation]>>,
}

pub(crate) struct ShapeInstanceKey<'a> {
    font_blob_id: u64,
    font_index: u32,
    synthesis: &'a fontique::Synthesis,
    variations: Option<&'a [FontVariation]>,
}

impl<'a> ShapeInstanceKey<'a> {
    pub(crate) const fn new(
        font_blob_id: u64,
        font_index: u32,
        synthesis: &'a fontique::Synthesis,
        variations: Option<&'a [FontVariation]>,
    ) -> Self {
        Self {
            font_blob_id,
            font_index,
            synthesis,
            variations,
        }
    }
}

impl<'a> Equivalent<ShapeInstanceId> for ShapeInstanceKey<'a> {
    #[inline(always)]
    fn equivalent(&self, key: &ShapeInstanceId) -> bool {
        self.font_blob_id == key.font_blob_id
            && self.font_index == key.font_index
            && *self.synthesis == key.synthesis
            && self.variations == key.variations.as_deref()
    }
}

impl<'a> Into<ShapeInstanceId> for ShapeInstanceKey<'a> {
    #[inline(always)]
    fn into(self) -> ShapeInstanceId {
        ShapeInstanceId {
            font_blob_id: self.font_blob_id,
            font_index: self.font_index,
            synthesis: *self.synthesis,
            variations: self.variations.map(|v| v.to_vec().into()),
        }
    }
}

pub(crate) struct ShapePlanId {
    font_blob_id: u64,
    font_index: u32,
    direction: harfrust::Direction,
    script: harfrust::Script,
    language: Option<harfrust::Language>,
    features: Box<[harfrust::Feature]>,
}

pub(crate) struct ShapePlanKey<'a> {
    font_blob_id: u64,
    font_index: u32,
    direction: harfrust::Direction,
    script: harfrust::Script,
    language: Option<harfrust::Language>,
    features: &'a [harfrust::Feature],
}

impl<'a> ShapePlanKey<'a> {
    pub(crate) const fn new(
        font_blob_id: u64,
        font_index: u32,
        direction: harfrust::Direction,
        script: harfrust::Script,
        language: Option<harfrust::Language>,
        features: &'a [harfrust::Feature],
    ) -> Self {
        Self {
            font_blob_id,
            font_index,
            direction,
            script,
            language,
            features,
        }
    }
}

impl<'a> Equivalent<ShapePlanId> for ShapePlanKey<'a> {
    #[inline(always)]
    fn equivalent(&self, key: &ShapePlanId) -> bool {
        self.font_blob_id == key.font_blob_id
            && self.font_index == key.font_index
            && self.direction == key.direction
            && self.script == key.script
            && self.language == key.language
            && self.features.len() == key.features.len()
            && self
                .features
                .iter()
                .zip(key.features.iter())
                .all(|(a, b)| a == b)
    }
}

impl<'a> Into<ShapePlanId> for ShapePlanKey<'a> {
    #[inline(always)]
    fn into(self) -> ShapePlanId {
        ShapePlanId {
            font_blob_id: self.font_blob_id,
            font_index: self.font_index,
            direction: self.direction,
            script: self.script,
            language: self.language,
            features: self.features.to_vec().into(),
        }
    }
}
