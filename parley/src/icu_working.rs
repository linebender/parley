use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, Script};
use swash::text::cluster::Boundary;
use crate::bidi::BidiLevel;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct CharInfo {
    // TODO(conor) - Shift attribute comments up from `impl CharInfo`
    pub ch: char, // TODO(conor) temporary, most likely
    pub boundary: Boundary,
    pub bidi_embed_level: BidiLevel,
    pub script: swash::text::Script,
    pub script_icu: Script,
    pub general_category: GeneralCategory,
    pub grapheme_cluster_break: GraphemeClusterBreak,
}

impl CharInfo {
    // TODO(conor) Simpler construction, avoid impl?
    pub(crate) fn new(
        ch: char,
        boundary: Boundary,
        bidi_embed_level: BidiLevel,
        script: swash::text::Script,
        script_icu: Script,
        general_category: GeneralCategory,
        grapheme_cluster_break: GraphemeClusterBreak,
    ) -> Self {
        Self {
            ch,
            boundary,
            bidi_embed_level,
            script,
            script_icu,
            general_category,
            grapheme_cluster_break,
        }
    }
}