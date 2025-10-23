use icu::properties::provider::PropertyCodePointMap;
use databake::Bake;
use icu_collections::codepointtrie::CodePointTrie;
use zerofrom::ZeroFrom;

#[derive(Clone, Debug, Eq, PartialEq, yoke::Yokeable, ZeroFrom)]
#[derive(serde::Serialize, Bake)]
#[databake(path = composite_props_marker)]
#[derive(serde::Deserialize)]
pub struct CompositePropsV1Data<'data> {
    #[serde(borrow)]
    pub trie: CodePointTrie<'data, u32>,

}

icu_provider::data_struct!(CompositePropsV1Data<'_>);

icu_provider::data_marker!(
    /// Marker for the composite multi-property trie (locale-invariant singleton)
    CompositePropsV1,
    CompositePropsV1Data<'static>,
    is_singleton = true,
);

// Helpers to unpack at runtime
pub mod unpack {
    use icu::properties::props::{BidiClass, GeneralCategory, GraphemeClusterBreak, LineBreak, Script};

    const SCRIPT_BITS: u32 = 8;
    const GC_BITS: u32 = 5;
    const GCB_BITS: u32 = 5;
    const BIDI_BITS: u32 = 5;
    const LB_BITS: u32 = 6;
    const IS_EMOJI_OR_PICTOGRAPH_BITS: u32 = 1;
    const IS_MANDATORY_LINE_BREAK_BITS: u32 = 1;

    const SCRIPT_SHIFT: u32 = 0;
    const GC_SHIFT: u32 = SCRIPT_SHIFT + SCRIPT_BITS;
    const GCB_SHIFT: u32 = GC_SHIFT + GC_BITS;
    const BIDI_SHIFT: u32 = GCB_SHIFT + GCB_BITS;
    const LB_SHIFT: u32 = BIDI_SHIFT + BIDI_BITS;
    const IS_EMOJI_OR_PICTOGRAPH_SHIFT: u32 = LB_SHIFT + LB_BITS;
    const IS_MANDATORY_LINE_BREAK_SHIFT: u32 = IS_EMOJI_OR_PICTOGRAPH_SHIFT + IS_EMOJI_OR_PICTOGRAPH_BITS;

    #[inline]
    fn mask(bits: u32) -> u32 {
        (1u32 << bits) - 1
    }

    #[inline]
    pub fn script(v: u32) -> Script {
        Script::from_icu4c_value(((v >> SCRIPT_SHIFT) & mask(SCRIPT_BITS)) as u16)
    }

    #[inline]
    pub fn general_category(v: u32) -> GeneralCategory {
        GeneralCategory::try_from(((v >> GC_SHIFT) & mask(GC_BITS)) as u8).unwrap()
    }

    #[inline]
    pub fn grapheme_cluster_break(v: u32) -> GraphemeClusterBreak {
        GraphemeClusterBreak::from_icu4c_value(((v >> GCB_SHIFT) & mask(GCB_BITS)) as u8)
    }

    #[inline]
    pub fn bidi_class(v: u32) -> BidiClass {
        BidiClass::from_icu4c_value(((v >> BIDI_SHIFT) & mask(BIDI_BITS)) as u8)
    }

    #[inline]
    pub fn line_break(v: u32) -> LineBreak {
        LineBreak::from_icu4c_value(((v >> LB_SHIFT) & mask(LB_BITS)) as u8)
    }

    #[inline]
    pub fn is_emoji_or_pictograph(v: u32) -> bool {
        ((v >> IS_EMOJI_OR_PICTOGRAPH_SHIFT) & mask(IS_EMOJI_OR_PICTOGRAPH_BITS)) != 0
    }

    #[inline]
    pub fn is_mandatory_linebreak(v: u32) -> bool {
        ((v >> IS_MANDATORY_LINE_BREAK_SHIFT) & mask(IS_MANDATORY_LINE_BREAK_BITS)) != 0
    }
}
