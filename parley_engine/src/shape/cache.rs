// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use parlance::FontVariation;

#[derive(PartialEq, Eq, Copy, Clone)]
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

pub(crate) struct ShapeInstanceId {
    /// The font collection's blob ID.
    font_blob_id: u64,
    /// The font's index in the font collection.
    font_index: u32,
    synthesis: parlance::Synthesis,
    variations: Option<Box<[FontVariation]>>,
}

pub(crate) struct ShapeInstanceKey<'a> {
    /// The font collection's blob ID.
    font_blob_id: u64,
    /// The font's index in the font collection.
    font_index: u32,
    synthesis: &'a parlance::Synthesis,
    variations: Option<&'a [FontVariation]>,
}

impl<'a> ShapeInstanceKey<'a> {
    pub(crate) const fn new(
        font_blob_id: u64,
        font_index: u32,
        synthesis: &'a parlance::Synthesis,
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

impl PartialEq<ShapeInstanceId> for ShapeInstanceKey<'_> {
    #[inline(always)]
    fn eq(&self, key: &ShapeInstanceId) -> bool {
        self.font_blob_id == key.font_blob_id
            && self.font_index == key.font_index
            && *self.synthesis == key.synthesis
            && self.variations == key.variations.as_deref()
    }
}

impl<'a> From<ShapeInstanceKey<'a>> for ShapeInstanceId {
    #[inline(always)]
    fn from(key: ShapeInstanceKey<'a>) -> Self {
        Self {
            font_blob_id: key.font_blob_id,
            font_index: key.font_index,
            synthesis: *key.synthesis,
            variations: key.variations.map(|v| v.to_vec().into()),
        }
    }
}
