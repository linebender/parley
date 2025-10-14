use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, Script};
use swash::text::cluster::Boundary;
use crate::bidi::BidiLevel;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct CharInfo {
    // TODO(conor) - Shift attribute comments up from `impl CharInfo`
    pub boundary: Boundary,
    pub bidi_embed_level: BidiLevel,
    pub script: Script,
    pub grapheme_cluster_break: GraphemeClusterBreak,
    pub is_control: bool,
    pub contributes_to_shaping: bool,
    pub force_normalize: bool,
}

impl CharInfo {
    // TODO(conor) Simpler construction, avoid impl?
    pub(crate) fn new(
        boundary: Boundary,
        bidi_embed_level: BidiLevel,
        script: Script,
        grapheme_cluster_break: GraphemeClusterBreak,
        is_control: bool,
        contributes_to_shaping: bool,
        force_normalize: bool,
    ) -> Self {
        Self {
            boundary,
            bidi_embed_level,
            script,
            grapheme_cluster_break,
            is_control,
            contributes_to_shaping,
            force_normalize,
        }
    }
}