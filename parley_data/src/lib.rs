// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `parley_data` packages the Unicode data that Parley's text analysis and shaping pipeline needs at runtime.
//! It exposes a locale-invariant `CompositeProps` data backed by a compact `CodePointTrie`, allowing the engine to obtain all required character properties with a single lookup.

#![no_std]

use icu_properties::props::{BidiClass, GeneralCategory, GraphemeClusterBreak, Script};

/// Baked data.
#[cfg(feature = "baked")]
pub mod generated;

/// Unicode character properties relevant for text analysis.
#[derive(Copy, Clone, Debug)]
pub struct Properties(u32);

impl Properties {
    const SCRIPT_BITS: u32 = 8;
    const GC_BITS: u32 = 5;
    const GCB_BITS: u32 = 5;
    const BIDI_BITS: u32 = 5;
    const IS_EMOJI_OR_PICTOGRAPH_BITS: u32 = 1;
    const IS_VARIATION_SELECTOR_BITS: u32 = 1;
    const IS_REGION_INDICATOR_BITS: u32 = 1;
    const IS_MANDATORY_LINE_BREAK_BITS: u32 = 1;

    const SCRIPT_SHIFT: u32 = 0;
    const GC_SHIFT: u32 = Self::SCRIPT_SHIFT + Self::SCRIPT_BITS;
    const GCB_SHIFT: u32 = Self::GC_SHIFT + Self::GC_BITS;
    const BIDI_SHIFT: u32 = Self::GCB_SHIFT + Self::GCB_BITS;
    const IS_EMOJI_OR_PICTOGRAPH_SHIFT: u32 = Self::BIDI_SHIFT + Self::BIDI_BITS;
    const IS_VARIATION_SELECTOR_SHIFT: u32 =
        Self::IS_EMOJI_OR_PICTOGRAPH_SHIFT + Self::IS_EMOJI_OR_PICTOGRAPH_BITS;
    const IS_REGION_INDICATOR_SHIFT: u32 =
        Self::IS_VARIATION_SELECTOR_SHIFT + Self::IS_VARIATION_SELECTOR_BITS;
    const IS_MANDATORY_LINE_BREAK_SHIFT: u32 =
        Self::IS_REGION_INDICATOR_SHIFT + Self::IS_REGION_INDICATOR_BITS;

    #[cfg(feature = "baked")]
    #[inline(always)]
    /// Returns the properties for a given character.
    pub fn get(ch: char) -> Self {
        Self(generated::COMPOSITE.get(ch))
    }

    /// Creates a new [`Properties`] from the given properties
    pub fn new(
        script: Script,
        gc: GeneralCategory,
        gcb: GraphemeClusterBreak,
        bidi: BidiClass,
        is_emoji_or_pictographic: bool,
        is_variation_selector: bool,
        is_region_indicator: bool,
        is_mandatory_linebreak: bool,
    ) -> Self {
        let s = script.to_icu4c_value() as u32;
        let gc = gc as u32;
        let gcb = gcb.to_icu4c_value() as u32;
        let bidi = bidi.to_icu4c_value() as u32;

        Self(
            (s << Self::SCRIPT_SHIFT)
                | (gc << Self::GC_SHIFT)
                | (gcb << Self::GCB_SHIFT)
                | (bidi << Self::BIDI_SHIFT)
                | ((is_emoji_or_pictographic as u32) << Self::IS_EMOJI_OR_PICTOGRAPH_SHIFT)
                | ((is_variation_selector as u32) << Self::IS_VARIATION_SELECTOR_SHIFT)
                | ((is_region_indicator as u32) << Self::IS_REGION_INDICATOR_SHIFT)
                | ((is_mandatory_linebreak as u32) << Self::IS_MANDATORY_LINE_BREAK_SHIFT),
        )
    }

    #[inline(always)]
    fn bits(&self, shift: u32, bits: u32) -> u32 {
        (self.0 >> shift) & ((1 << bits) - 1)
    }

    /// Returns the script for the character.
    #[inline(always)]
    pub fn script(&self) -> Script {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "script data only occupies SCRIPT_BITS bits; we cast to `u16` to fulfil the `from_icu4c_value` contract."
        )]
        Script::from_icu4c_value(self.bits(Self::SCRIPT_SHIFT, Self::SCRIPT_BITS) as u16)
    }

    /// Returns the general category for the character.
    #[inline(always)]
    pub fn general_category(&self) -> GeneralCategory {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "general category data only occupies GC_BITS bits."
        )]
        GeneralCategory::try_from(self.bits(Self::GC_SHIFT, Self::GC_BITS) as u8).unwrap()
    }

    /// Returns the grapheme cluster break for the character.
    #[inline(always)]
    pub fn grapheme_cluster_break(&self) -> GraphemeClusterBreak {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "cluster break data only occupies GCB_BITS bits"
        )]
        GraphemeClusterBreak::from_icu4c_value(self.bits(Self::GCB_SHIFT, Self::GCB_BITS) as u8)
    }

    /// Returns the bidirectional class for the character.
    #[inline(always)]
    pub fn bidi_class(&self) -> BidiClass {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "bidi class data only occupies BIDI_BITS bits"
        )]
        BidiClass::from_icu4c_value(self.bits(Self::BIDI_SHIFT, Self::BIDI_BITS) as u8)
    }

    /// Returns whether the character is an emoji or pictograph.
    #[inline(always)]
    pub fn is_emoji_or_pictograph(&self) -> bool {
        self.bits(
            Self::IS_EMOJI_OR_PICTOGRAPH_SHIFT,
            Self::IS_EMOJI_OR_PICTOGRAPH_BITS,
        ) != 0
    }

    /// Returns whether the character is a variation selector.
    #[inline(always)]
    pub fn is_variation_selector(&self) -> bool {
        self.bits(
            Self::IS_VARIATION_SELECTOR_SHIFT,
            Self::IS_VARIATION_SELECTOR_BITS,
        ) != 0
    }

    /// Returns whether the character is a region indicator.
    #[inline(always)]
    pub fn is_region_indicator(&self) -> bool {
        self.bits(
            Self::IS_REGION_INDICATOR_SHIFT,
            Self::IS_REGION_INDICATOR_BITS,
        ) != 0
    }

    /// Returns whether the character is a mandatory linebreak.
    #[inline(always)]
    pub fn is_mandatory_linebreak(&self) -> bool {
        self.bits(
            Self::IS_MANDATORY_LINE_BREAK_SHIFT,
            Self::IS_MANDATORY_LINE_BREAK_BITS,
        ) != 0
    }
}

impl From<Properties> for u32 {
    fn from(value: Properties) -> Self {
        value.0
    }
}
