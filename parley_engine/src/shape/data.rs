// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![expect(missing_docs, reason = "Deferred")]

use crate::{Boundary, shape::Whitespace};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ClusterData {
    pub info: ClusterInfo,
    /// Cluster flags (see impl methods for details).
    pub flags: u16,
    /// Style index for this cluster.
    pub style_index: u16,
    /// Number of glyphs in this cluster (0xFF = single glyph stored inline)
    pub glyph_len: u8,
    /// Number of text bytes in this cluster
    pub text_len: u8,
    /// If `glyph_len == 0xFF`, then `glyph_offset` is a glyph identifier,
    /// otherwise, it's an offset into the glyph array with the base
    /// taken from the owning run.
    pub glyph_offset: u32,
    /// Offset into the text for this cluster
    pub text_offset: u16,
    /// Advance width for this cluster
    pub advance: f32,
}

impl ClusterData {
    pub const LIGATURE_START: u16 = 1;
    pub const LIGATURE_COMPONENT: u16 = 2;

    #[inline(always)]
    pub fn is_ligature_start(self) -> bool {
        self.flags & Self::LIGATURE_START != 0
    }

    #[inline(always)]
    pub fn is_ligature_component(self) -> bool {
        self.flags & Self::LIGATURE_COMPONENT != 0
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Character {
    pub text_byte_start: u32,
    pub info: ClusterInfo,
    pub grapheme_start: bool,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShapedCluster {
    /// The first character of this cluster, as an index into [`ShapedText::characters`].
    ///
    /// Note this is not a character index into the source text: the shaped character array only
    /// contains the characters of runs that were actually shaped. Mapping back to the source text
    /// goes through [`Character::text_byte_start`].
    pub char_start: u32,

    /// One past the last character of this cluster, as an index into the shaped character array.
    pub char_end: u32,

    /// Style index for this cluster.
    pub style_index: u16,

    /// Cluster flags (see impl methods for details).
    pub flags: u16,

    /// If `glyph_len == 0xFF`, then `glyph_offset` is a glyph identifier, otherwise, it's an index
    /// into the glyph array.
    pub glyph_offset: u32,

    /// Number of glyphs in this cluster (0xFF = single glyph stored inline)
    pub glyph_len: u8,

    /// Advance width for this cluster
    pub advance: f32,
}

impl ShapedCluster {
    pub(crate) const GRAPHEME_START: u16 = 1 << 0;
    pub(crate) const SAFE_TO_BREAK: u16 = 1 << 1;

    /// Whether the logical start of this shaped cluster is also the start of a grapheme.
    #[inline(always)]
    pub fn is_grapheme_start(self) -> bool {
        self.flags & Self::GRAPHEME_START != 0
    }

    /// Whether the logical start of this shaped cluster is also the start of a grapheme.
    #[inline(always)]
    pub fn is_safe_to_break_before(self) -> bool {
        self.flags & Self::SAFE_TO_BREAK != 0
    }

    /// The number of graphemes this cluster overlaps.
    pub(crate) fn graphemes_overlapped(&self, characters: &[Character]) -> u32 {
        let start = self.char_start as usize + 1;
        let end = self.char_end as usize;
        let mut graphemes = 1;
        for character in &characters[start..end] {
            graphemes += u32::from(character.grapheme_start);
        }
        graphemes
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ClusterInfo {
    boundary: Boundary,
    source_char: char,
}

impl ClusterInfo {
    #[inline(always)]
    pub fn new(boundary: Boundary, source_char: char) -> Self {
        Self {
            boundary,
            source_char,
        }
    }

    // Returns the boundary type of the cluster.
    #[inline(always)]
    pub fn boundary(self) -> Boundary {
        self.boundary
    }

    // Returns the whitespace type of the cluster.
    #[inline(always)]
    pub fn whitespace(self) -> Whitespace {
        to_whitespace(self.source_char)
    }

    /// Returns if the cluster is a line boundary.
    #[inline]
    pub fn is_boundary(self) -> bool {
        self.boundary != Boundary::None
    }

    /// Returns if the cluster is an emoji.
    #[inline]
    pub fn is_emoji(self) -> bool {
        // TODO: Defer to ICU4X properties (see: https://docs.rs/icu/latest/icu/properties/props/struct.Emoji.html).
        matches!(self.source_char as u32, 0x1F600..=0x1F64F | 0x1F300..=0x1F5FF | 0x1F680..=0x1F6FF | 0x2600..=0x26FF | 0x2700..=0x27BF)
    }

    /// Returns if the cluster is any whitespace.
    #[inline(always)]
    pub fn is_whitespace(self) -> bool {
        self.source_char.is_whitespace()
    }

    /// Returns the cluster's original character.
    #[inline(always)]
    pub fn source_char(self) -> char {
        self.source_char
    }
}

// TODO: should become private when more of `parley`'s shaping is in `parley_engine`
#[inline]
pub const fn to_whitespace(c: char) -> Whitespace {
    const LINE_SEPARATOR: char = '\u{2028}';
    const PARAGRAPH_SEPARATOR: char = '\u{2029}';

    match c {
        ' ' => Whitespace::Space,
        '\t' => Whitespace::Tab,
        '\n' | '\r' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR => Whitespace::Newline,
        '\u{00A0}' => Whitespace::NoBreakSpace,
        _ => Whitespace::None,
    }
}
