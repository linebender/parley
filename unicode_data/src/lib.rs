//! Unicode data as required by the Parley crate for efficient text analysis.

// TODO: Add variation selector to composite props
// TODO: Add bidi class into the trie (and pass it to unicode-bidi)
// TODO: I think region indicator is just a range... I can just encode that.

use databake::Bake;
use icu::properties::props::{BidiClass, GeneralCategory, GraphemeClusterBreak, LineBreak, Script};
use icu_collections::codepointtrie::CodePointTrie;
use zerofrom::ZeroFrom;

#[cfg(feature = "build")]
pub mod build;

#[derive(Clone, Debug, Eq, PartialEq, yoke::Yokeable, ZeroFrom, serde::Serialize, Bake)]
#[databake(path = composite_props_marker)]
#[derive(serde::Deserialize)]
pub struct CompositePropsV1Data<'data> {
    #[serde(borrow)]
    trie: CodePointTrie<'data, u32>,
}

icu_provider::data_struct!(CompositePropsV1Data<'_>);

icu_provider::data_marker!(
    /// Marker for the composite multi-property trie (locale-invariant singleton)
    CompositePropsV1,
    CompositePropsV1Data<'static>,
    is_singleton = true,
);

impl CompositePropsV1Data<'_> {
    #[inline]
    pub fn properties(&self, ch: u32) -> Properties {
        Properties(self.trie.get32(ch))
    }
}

impl unicode_bidi::BidiDataSource for CompositePropsV1Data<'_> {
    // TODO: Store unicode_bidi::BidiClass in the trie instead...
    fn bidi_class(&self, cp: char) -> unicode_bidi::BidiClass {
        self.properties(cp as u32).bidi_class()
    }
}

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

    #[inline(always)]
    fn get(&self, shift: u32, bits: u32) -> u32 {
        (self.0 >> shift) & ((1 << bits) - 1)
    }

    #[inline]
    pub fn script(&self) -> Script {
        Script::from_icu4c_value(self.get(Self::SCRIPT_SHIFT, Self::SCRIPT_BITS) as u16)
    }

    #[inline]
    pub fn general_category(&self) -> GeneralCategory {
        GeneralCategory::try_from(self.get(Self::GC_SHIFT, Self::GC_BITS) as u8).unwrap()
    }

    #[inline]
    pub fn grapheme_cluster_break(&self) -> GraphemeClusterBreak {
        GraphemeClusterBreak::from_icu4c_value(self.get(Self::GCB_SHIFT, Self::GCB_BITS) as u8)
    }

    #[inline]
    pub fn bidi_class(&self) -> unicode_bidi::BidiClass {
        #[allow(unsafe_code)]
        unsafe {
            std::mem::transmute(self.get(Self::BIDI_SHIFT, Self::BIDI_BITS) as u8)
        }
    }

    #[inline]
    pub fn needs_bidi_resolution(&self) -> bool {
        use unicode_bidi::BidiClass::*;
        let bidi_class = self.bidi_class();
        let bidi_mask = 1u32 << (bidi_class as u32);

        const OVERRIDE_MASK: u32 =
            (1 << RLE as u32) | (1 << LRE as u32) | (1 << RLO as u32) | (1 << LRO as u32);
        const ISOLATE_MASK: u32 = (1 << RLI as u32) | (1 << LRI as u32) | (1 << FSI as u32);
        const EXPLICIT_MASK: u32 = OVERRIDE_MASK | ISOLATE_MASK;
        const BIDI_MASK: u32 =
            EXPLICIT_MASK | (1 << R as u32) | (1 << AL as u32) | (1 << AN as u32);

        (bidi_mask & BIDI_MASK) != 0
    }

    #[inline]
    pub fn is_emoji_or_pictograph(&self) -> bool {
        self.get(
            Self::IS_EMOJI_OR_PICTOGRAPH_SHIFT,
            Self::IS_EMOJI_OR_PICTOGRAPH_BITS,
        ) != 0
    }

    #[inline]
    pub fn is_variation_selector(&self) -> bool {
        self.get(
            Self::IS_VARIATION_SELECTOR_SHIFT,
            Self::IS_VARIATION_SELECTOR_BITS,
        ) != 0
    }

    #[inline]
    pub fn is_region_indicator(&self) -> bool {
        self.get(
            Self::IS_REGION_INDICATOR_SHIFT,
            Self::IS_REGION_INDICATOR_BITS,
        ) != 0
    }

    #[inline]
    pub fn is_mandatory_linebreak(&self) -> bool {
        self.get(
            Self::IS_MANDATORY_LINE_BREAK_SHIFT,
            Self::IS_MANDATORY_LINE_BREAK_BITS,
        ) != 0
    }
}
