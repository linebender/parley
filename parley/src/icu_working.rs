use swash::text::cluster::Boundary;
use crate::bidi::BidiLevel;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct CharInfo {
    // TODO(conor) - Shift attribute comments up from `impl CharInfo`
    pub boundary: Boundary,
    pub bidi_embed_level: BidiLevel,
    pub script: swash::text::Script,
}

impl CharInfo {
    pub(crate) fn new(boundary: Boundary, bidi_embed_level: BidiLevel, script: swash::text::Script) -> Self {
        Self { boundary, bidi_embed_level, script }
    }
}