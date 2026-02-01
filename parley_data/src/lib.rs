// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Unicode data that Parley's text analysis and shaping pipeline needs at runtime by exposing:
//!
//! - Re-exported ICU4X data providers for grapheme, word, and line breaking, plus Unicode normalization tables used by Parley.
//! - A locale-invariant `CompositeProps` provider backed by a compact `CodePointTrie`, allowing the engine to obtain all required character properties with a single lookup.
//! - Runtime loading of segmenter models for language-specific word/line breaking (via [`SegmenterModelData`]).
//!
//! # Pluggable segmenter models
//!
//! By default, Parley uses ICU4X's rule-based segmentation. This works well for most languages, but produces suboptimal results for languages like Thai, Lao, Khmer, Burmese, Chinese, and Japanese that don't use spaces between words.
//!
//! For better segmentation in these languages, you can load LSTM or dictionary models at runtime.
//! These models are included here if you enable the `bundled-segmenter-models` feature, but you may want to export them from this crate at build time and load them dynamically.

#![no_std]

use icu_properties::props::{BidiClass, GeneralCategory, GraphemeClusterBreak, Script};

/// Baked data.
#[cfg(feature = "baked")]
pub mod generated;

/// Lookup for [`Properties`]
#[derive(Clone, Debug, Copy)]
pub struct CompositeProps;

#[cfg(feature = "baked")]
impl CompositeProps {
    /// Returns the properties for a given character.
    #[inline(always)]
    pub fn properties(&self, ch: u32) -> Properties {
        Properties(generated::COMPOSITE.get32(ch))
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
    pub fn bidi_class(&self) -> BidiClass {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "bidi class data only occupies BIDI_BITS bits"
        )]
        BidiClass::from_icu4c_value(self.get(Self::BIDI_SHIFT, Self::BIDI_BITS) as u8)
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

/// Bundled segmenter models for languages requiring complex word breaking.
///
/// These are pre-generated postcard blobs that can be loaded at runtime.
///
/// You can also depend on `parley_data` in your build, and export these models to files that you can ship alongside
/// your Parley-using application and load dynamically.
#[cfg(feature = "bundled-segmenter-models")]
pub mod bundled_models {
    /// Thai LSTM model for word/line segmentation.
    pub const THAI_LSTM: &[u8] = include_bytes!(
        "generated/icu4x_data/segmenter_models/Thai_codepoints_exclusive_model4_heavy.postcard"
    );

    /// Lao LSTM model for word/line segmentation.
    pub const LAO_LSTM: &[u8] = include_bytes!(
        "generated/icu4x_data/segmenter_models/Lao_codepoints_exclusive_model4_heavy.postcard"
    );

    /// Khmer LSTM model for word/line segmentation.
    pub const KHMER_LSTM: &[u8] = include_bytes!(
        "generated/icu4x_data/segmenter_models/Khmer_codepoints_exclusive_model4_heavy.postcard"
    );

    /// Burmese LSTM model for word/line segmentation.
    pub const BURMESE_LSTM: &[u8] = include_bytes!(
        "generated/icu4x_data/segmenter_models/Burmese_codepoints_exclusive_model4_heavy.postcard"
    );

    /// Chinese/Japanese dictionary for word segmentation.
    pub const CJ_DICT: &[u8] =
        include_bytes!("generated/icu4x_data/segmenter_models/cjdict.postcard");

    /// Thai dictionary for word segmentation.
    pub const THAI_DICT: &[u8] =
        include_bytes!("generated/icu4x_data/segmenter_models/thaidict.postcard");

    /// Lao dictionary for word segmentation.
    pub const LAO_DICT: &[u8] =
        include_bytes!("generated/icu4x_data/segmenter_models/laodict.postcard");

    /// Khmer dictionary for word segmentation.
    pub const KHMER_DICT: &[u8] =
        include_bytes!("generated/icu4x_data/segmenter_models/khmerdict.postcard");

    /// Burmese dictionary for word segmentation.
    pub const BURMESE_DICT: &[u8] =
        include_bytes!("generated/icu4x_data/segmenter_models/burmesedict.postcard");

    /// All bundled LSTM models as (model ID, blob) pairs.
    pub const ALL_LSTM: &[(&str, &[u8])] = &[
        ("Thai_codepoints_exclusive_model4_heavy", THAI_LSTM),
        ("Lao_codepoints_exclusive_model4_heavy", LAO_LSTM),
        ("Khmer_codepoints_exclusive_model4_heavy", KHMER_LSTM),
        ("Burmese_codepoints_exclusive_model4_heavy", BURMESE_LSTM),
    ];

    /// All bundled "preferred" models (LSTM when possible, dictionary if not) as (model ID, blob) pairs.
    pub const ALL_AUTO: &[(&str, &[u8])] = &[
        ("cjdict", CJ_DICT),
        ("Thai_codepoints_exclusive_model4_heavy", THAI_LSTM),
        ("Lao_codepoints_exclusive_model4_heavy", LAO_LSTM),
        ("Khmer_codepoints_exclusive_model4_heavy", KHMER_LSTM),
        ("Burmese_codepoints_exclusive_model4_heavy", BURMESE_LSTM),
    ];

    /// All bundled dictionary models as (model ID, blob) pairs.
    pub const ALL_DICT: &[(&str, &[u8])] = &[
        ("cjdict", CJ_DICT),
        ("thaidict", THAI_DICT),
        ("laodict", LAO_DICT),
        ("khmerdict", KHMER_DICT),
        ("burmesedict", BURMESE_DICT),
    ];

    /// All bundled models, LSTM and dictionary, as (model ID, blob) pairs. Note that you cannot load both LSTM and
    /// dictionary models for the same locale at runtime.
    pub const ALL: &[(&str, &[u8])] = &[
        ("Thai_codepoints_exclusive_model4_heavy", THAI_LSTM),
        ("Lao_codepoints_exclusive_model4_heavy", LAO_LSTM),
        ("Khmer_codepoints_exclusive_model4_heavy", KHMER_LSTM),
        ("Burmese_codepoints_exclusive_model4_heavy", BURMESE_LSTM),
        ("cjdict", CJ_DICT),
        ("thaidict", THAI_DICT),
        ("laodict", LAO_DICT),
        ("khmerdict", KHMER_DICT),
        ("burmesedict", BURMESE_DICT),
    ];
}
