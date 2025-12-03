// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Unicode data that Parley's text analysis and shaping pipeline needs at runtime by exposing:
//!
//! - Re-exported ICU4X data providers for grapheme, word, and line breaking, plus Unicode normalization tables used by Parley.
//! - A locale-invariant `CompositePropsV1` provider backed by a compact `CodePointTrie`, allowing the engine to obtain all required character properties with a single lookup.

#![no_std]

use icu_collections::codepointtrie::CodePointTrie;
use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, Script};
use zerofrom::ZeroFrom;

/// Baked data for the `CompositePropsV1` data provider.
#[cfg(feature = "baked")]
pub mod generated;

/// A data provider of `CompositePropsV1`.
#[derive(Clone, Debug, Eq, PartialEq, yoke::Yokeable, ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = composite_props_marker))]
pub struct CompositePropsV1Data<'data> {
    trie: CodePointTrie<'data, u32>,
}

#[cfg(feature = "datagen")]
icu_provider::data_struct!(CompositePropsV1Data<'_>);

impl serde::Serialize for CompositePropsV1Data<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_newtype_struct("CompositePropsV1Data", &self.trie)
    }
}

impl<'de> serde::Deserialize<'de> for CompositePropsV1Data<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let trie = CodePointTrie::deserialize(deserializer)?;
        Ok(CompositePropsV1Data { trie })
    }
}

impl<'data> CompositePropsV1Data<'data> {
    /// Creates a new `CompositePropsV1Data` from a `CodePointTrie`.
    pub fn new(trie: CodePointTrie<'data, u32>) -> Self {
        Self { trie }
    }
}

icu_provider::data_marker!(
    /// Marker for the composite multi-property trie (locale-invariant singleton)
    CompositePropsV1,
    CompositePropsV1Data<'static>,
    is_singleton = true,
);

impl CompositePropsV1Data<'_> {
    /// Returns the properties for a given character.
    #[inline(always)]
    pub fn properties(&self, ch: u32) -> Properties {
        Properties(self.trie.get32(ch))
    }
}

impl unicode_bidi::BidiDataSource for CompositePropsV1Data<'_> {
    fn bidi_class(&self, cp: char) -> unicode_bidi::BidiClass {
        self.properties(cp as u32).bidi_class()
    }
}

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

    /// Packs the given arguments into a single u32.
    #[cfg(feature = "datagen")]
    pub fn new(
        script: Script,
        gc: GeneralCategory,
        gcb: GraphemeClusterBreak,
        bidi: icu_properties::props::BidiClass,
        is_emoji_or_pictographic: bool,
        is_variation_selector: bool,
        is_region_indicator: bool,
        is_mandatory_linebreak: bool,
    ) -> Self {
        let s = script.to_icu4c_value() as u32;
        let gc = gc as u32;
        let gcb = gcb.to_icu4c_value() as u32;
        use icu_properties::props::BidiClass;
        let bidi = match bidi {
            BidiClass::LeftToRight => 9,
            BidiClass::RightToLeft => 17,
            BidiClass::EuropeanNumber => 5,
            BidiClass::EuropeanSeparator => 6,
            BidiClass::EuropeanTerminator => 7,
            BidiClass::ArabicNumber => 1,
            BidiClass::CommonSeparator => 4,
            BidiClass::ParagraphSeparator => 2,
            BidiClass::SegmentSeparator => 21,
            BidiClass::WhiteSpace => 22,
            BidiClass::OtherNeutral => 14,
            BidiClass::LeftToRightEmbedding => 10,
            BidiClass::LeftToRightOverride => 12,
            BidiClass::ArabicLetter => 0,
            BidiClass::RightToLeftEmbedding => 18,
            BidiClass::RightToLeftOverride => 20,
            BidiClass::PopDirectionalFormat => 15,
            BidiClass::NonspacingMark => 13,
            BidiClass::BoundaryNeutral => 3,
            BidiClass::FirstStrongIsolate => 8,
            BidiClass::LeftToRightIsolate => 11,
            BidiClass::RightToLeftIsolate => 19,
            BidiClass::PopDirectionalIsolate => 16,
            _ => unreachable!("Invalid BidiClass: {:?}", bidi),
        };

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
    fn get(&self, shift: u32, bits: u32) -> u32 {
        (self.0 >> shift) & ((1 << bits) - 1)
    }

    /// Returns the script for the character.
    #[inline(always)]
    pub fn script(&self) -> Script {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "script data only occupies SCRIPT_BITS bits; we cast to `u16` to fulfil the `from_icu4c_value` contract."
        )]
        Script::from_icu4c_value(self.get(Self::SCRIPT_SHIFT, Self::SCRIPT_BITS) as u16)
    }

    /// Returns the general category for the character.
    #[inline(always)]
    pub fn general_category(&self) -> GeneralCategory {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "general category data only occupies GC_BITS bits."
        )]
        GeneralCategory::try_from(self.get(Self::GC_SHIFT, Self::GC_BITS) as u8).unwrap()
    }

    /// Returns the grapheme cluster break for the character.
    #[inline(always)]
    pub fn grapheme_cluster_break(&self) -> GraphemeClusterBreak {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "cluster break data only occupies GCB_BITS bits"
        )]
        GraphemeClusterBreak::from_icu4c_value(self.get(Self::GCB_SHIFT, Self::GCB_BITS) as u8)
    }

    /// Returns the bidirectional class for the character.
    #[inline(always)]
    pub fn bidi_class(&self) -> unicode_bidi::BidiClass {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "bidi class data only occupies BIDI_BITS bits"
        )]
        // Note: the values match the discriminants as of unicode_bidi 0.3.18,
        // so this is not a runtime match.
        match self.get(Self::BIDI_SHIFT, Self::BIDI_BITS) as u8 {
            0 => unicode_bidi::BidiClass::AL,
            1 => unicode_bidi::BidiClass::AN,
            2 => unicode_bidi::BidiClass::B,
            3 => unicode_bidi::BidiClass::BN,
            4 => unicode_bidi::BidiClass::CS,
            5 => unicode_bidi::BidiClass::EN,
            6 => unicode_bidi::BidiClass::ES,
            7 => unicode_bidi::BidiClass::ET,
            8 => unicode_bidi::BidiClass::FSI,
            9 => unicode_bidi::BidiClass::L,
            10 => unicode_bidi::BidiClass::LRE,
            11 => unicode_bidi::BidiClass::LRI,
            12 => unicode_bidi::BidiClass::LRO,
            13 => unicode_bidi::BidiClass::NSM,
            14 => unicode_bidi::BidiClass::ON,
            15 => unicode_bidi::BidiClass::PDF,
            16 => unicode_bidi::BidiClass::PDI,
            17 => unicode_bidi::BidiClass::R,
            18 => unicode_bidi::BidiClass::RLE,
            19 => unicode_bidi::BidiClass::RLI,
            20 => unicode_bidi::BidiClass::RLO,
            21 => unicode_bidi::BidiClass::S,
            22 => unicode_bidi::BidiClass::WS,
            val => {
                debug_assert!(false, "Invalid BidiClass: {val}");
                unicode_bidi::BidiClass::ON // Other Neutral
            }
        }
    }

    /// Returns whether the character needs bidirectional resolution.
    #[inline(always)]
    pub fn needs_bidi_resolution(&self) -> bool {
        const _: () = assert!(
            Properties::BIDI_BITS < 32,
            "too many bidi classes for bit shift hack"
        );
        let bidi_mask = 1 << self.get(Self::BIDI_SHIFT, Self::BIDI_BITS);

        const OVERRIDE_MASK: u32 = (1 << 18) | (1 << 10) | (1 << 20) | (1 << 12);
        const ISOLATE_MASK: u32 = (1 << 19) | (1 << 11) | (1 << 8);
        const EXPLICIT_MASK: u32 = OVERRIDE_MASK | ISOLATE_MASK;
        const BIDI_MASK: u32 = EXPLICIT_MASK | (1 << 17) | (1 << 0) | (1 << 1);

        (bidi_mask & BIDI_MASK) != 0
    }

    /// Returns whether the character is an emoji or pictograph.
    #[inline(always)]
    pub fn is_emoji_or_pictograph(&self) -> bool {
        self.get(
            Self::IS_EMOJI_OR_PICTOGRAPH_SHIFT,
            Self::IS_EMOJI_OR_PICTOGRAPH_BITS,
        ) != 0
    }

    /// Returns whether the character is a variation selector.
    #[inline(always)]
    pub fn is_variation_selector(&self) -> bool {
        self.get(
            Self::IS_VARIATION_SELECTOR_SHIFT,
            Self::IS_VARIATION_SELECTOR_BITS,
        ) != 0
    }

    /// Returns whether the character is a region indicator.
    #[inline(always)]
    pub fn is_region_indicator(&self) -> bool {
        self.get(
            Self::IS_REGION_INDICATOR_SHIFT,
            Self::IS_REGION_INDICATOR_BITS,
        ) != 0
    }

    /// Returns whether the character is a mandatory linebreak.
    #[inline(always)]
    pub fn is_mandatory_linebreak(&self) -> bool {
        self.get(
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
