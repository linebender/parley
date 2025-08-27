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
    pub(crate) fn new(font_blob_id: u64, font_index: u32) -> Self {
        Self {
            font_blob_id,
            font_index,
        }
    }
}

impl Equivalent<ShapeDataKey> for ShapeDataKey {
    fn equivalent(&self, key: &ShapeDataKey) -> bool {
        self == key
    }
}

impl Into<ShapeDataKey> for &ShapeDataKey {
    fn into(self) -> ShapeDataKey {
        *self
    }
}

pub(crate) type ShapeInstanceId = (u64, u32, fontique::Synthesis, Option<Box<[FontVariation]>>);

pub(crate) struct ShapeInstanceKey<'a> {
    font_blob_id: u64,
    font_index: u32,
    synthesis: &'a fontique::Synthesis,
    variations: Option<&'a [FontVariation]>,
}

impl<'a> ShapeInstanceKey<'a> {
    pub(crate) fn new(
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
    fn equivalent(&self, key: &ShapeInstanceId) -> bool {
        self.font_blob_id == key.0
            && self.font_index == key.1
            && *self.synthesis == key.2
            && self.variations == key.3.as_deref()
    }
}

impl<'a> Into<ShapeInstanceId> for ShapeInstanceKey<'a> {
    fn into(self) -> ShapeInstanceId {
        (
            self.font_blob_id,
            self.font_index,
            *self.synthesis,
            self.variations.map(|v| v.to_vec().into()),
        )
    }
}

pub(crate) type ShapePlanId = (
    u64,
    u32,
    harfrust::Direction,
    harfrust::Script,
    Option<harfrust::Language>,
    Box<[harfrust::Feature]>,
);

pub(crate) struct ShapePlanKey<'a> {
    font_blob_id: u64,
    font_index: u32,
    direction: harfrust::Direction,
    script: harfrust::Script,
    language: Option<harfrust::Language>,
    features: &'a [harfrust::Feature],
}

impl<'a> ShapePlanKey<'a> {
    pub(crate) fn new(
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
    fn equivalent(&self, key: &ShapePlanId) -> bool {
        self.font_blob_id == key.0
            && self.font_index == key.1
            && self.direction == key.2
            && self.script == key.3
            && self.language == key.4
            && self.features.len() == key.5.len()
            && self.features.iter().zip(key.5.iter()).all(|(a, b)| a == b)
    }
}

impl<'a> Into<ShapePlanId> for ShapePlanKey<'a> {
    fn into(self) -> ShapePlanId {
        (
            self.font_blob_id,
            self.font_index,
            self.direction,
            self.script,
            self.language,
            self.features.to_vec().into(),
        )
    }
}
