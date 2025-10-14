use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, Script};
use swash::text::cluster::Boundary;
use crate::bidi::BidiLevel;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct CharInfo {
    /// The line/word breaking boundary classification of this character.
    pub boundary: Boundary,
    /// The bidirectional embedding level of the character (even = LTR, odd = RTL).
    pub bidi_embed_level: BidiLevel,
    /// The Unicode script this character belongs to.
    pub script: Script,
    /// The grapheme cluster boundary property of this character.
    pub grapheme_cluster_break: GraphemeClusterBreak,
    /// Whether this character belongs to the "Control" general category in Unicode.
    pub is_control: bool,
    /// Whether this character contributes to text shaping in Parley.
    pub contributes_to_shaping: bool,
    /// Whether to apply NFC normalization before attempting cluster form variations during
    /// Parley's font selection.
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