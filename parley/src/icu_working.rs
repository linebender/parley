use icu_properties::props::{GraphemeClusterBreak, Script};
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