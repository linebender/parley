// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![expect(missing_docs, reason = "Deferred")]

use core::ops::Range;

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
    /// Style index for this character.
    pub style_index: u16,
    pub grapheme_start: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShapedClusterFlags(u16);

impl ShapedClusterFlags {
    const GLYPH_LEN_MASK: u16 = 0x00FF;
    const INLINE_GLYPH: u16 = 1 << 8;
    const GRAPHEME_START: u16 = 1 << 9;
    const SAFE_TO_BREAK_BEFORE: u16 = 1 << 10;

    #[inline(always)]
    pub(crate) const fn new(glyph_len: u8) -> Self {
        Self(glyph_len as u16)
    }

    #[inline(always)]
    pub(crate) const fn with_inline_glyph(mut self, set: bool) -> Self {
        self.0 = self.0 & !Self::INLINE_GLYPH | if set { Self::INLINE_GLYPH } else { 0 };
        self
    }

    #[inline(always)]
    pub(crate) const fn with_grapheme_start(mut self, set: bool) -> Self {
        self.0 = self.0 & !Self::GRAPHEME_START | if set { Self::GRAPHEME_START } else { 0 };
        self
    }

    #[inline(always)]
    pub(crate) const fn with_safe_to_break_before(mut self, set: bool) -> Self {
        self.0 =
            self.0 & !Self::SAFE_TO_BREAK_BEFORE | if set { Self::SAFE_TO_BREAK_BEFORE } else { 0 };
        self
    }

    #[inline(always)]
    const fn glyph_len(self) -> u8 {
        (self.0 & Self::GLYPH_LEN_MASK) as u8
    }

    #[inline(always)]
    const fn has_inline_glyph(self) -> bool {
        self.0 & Self::INLINE_GLYPH != 0
    }

    #[inline(always)]
    const fn is_grapheme_start(self) -> bool {
        self.0 & Self::GRAPHEME_START != 0
    }

    #[inline(always)]
    const fn is_safe_to_break_before(self) -> bool {
        self.0 & Self::SAFE_TO_BREAK_BEFORE != 0
    }
}

/// A span of characters and the glyphs they shaped into.
///
/// Shaping may reorder, compose, decompose, etc.; there is no finer-grained correspondence between
/// characters and glyphs. For example, a base letter with a combining mark may shape into a single
/// glyph, a single character may shape into multiple glyphs, and a ligature may combine multiple
/// characters into shared glyphs.
///
/// Shaped cluster boundaries are not necessarily [`Grapheme`][crate::Grapheme] boundaries. The
/// shared boundaries are encoded by [`Atom`][crate::Atom]s.
///
/// For more information about clusters, see [HarfBuzz's documentation][harfbuzz].
///
/// [harfbuzz]: https://harfbuzz.github.io/working-with-harfbuzz-clusters.html
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShapedCluster {
    /// The first character of this cluster, as an index into [`ShapedText::characters`](crate::ShapedText::characters).
    ///
    /// Note this is not a character index into the source text: the shaped character array only
    /// contains the characters of runs that were actually shaped. Mapping back to the source text
    /// goes through [`Character::text_byte_start`].
    pub(crate) chars_range: (u32, u32),

    /// Style index for this cluster.
    pub style_index: u16,

    /// The index into the glyph array where this cluster's glyphs start.
    ///
    /// If [`Self::has_inline_glyph`] is `true`, this is a glyph identifier instead. For more, see
    /// the documentation on that method.
    pub glyph_offset: u32,

    // /// Number of glyphs in this cluster (0xFF = single glyph stored inline)
    // pub glyph_len: u8,
    pub(crate) flags: ShapedClusterFlags,

    /// Advance width for this cluster
    pub advance: f32,
}

impl ShapedCluster {
    /// The character range of this slice.
    ///
    /// This indexes into [`Self::characters`].
    #[inline(always)]
    pub fn chars_range(&self) -> Range<u32> {
        self.chars_range.0..self.chars_range.1
    }

    /// The number of glyphs of this cluster.
    #[inline(always)]
    pub fn glyph_len(self) -> u8 {
        if self.has_inline_glyph() {
            1
        } else {
            self.flags.glyph_len()
        }
    }

    /// Whether this cluster's glyph is stored inline in [`Self::glyph_offset`].
    ///
    /// This is only possible if [`Self::glyph_len`] is one, and the glyph has no offset.
    /// [`Self::glyph_offset`] then encodes the glyph identifier rather than an index into the glyph
    /// array. The glyph's advance then is this cluster's advance.
    #[inline(always)]
    pub fn has_inline_glyph(self) -> bool {
        self.flags.has_inline_glyph()
    }

    /// Whether this shaped cluster's logical start also starts a grapheme.
    #[inline(always)]
    pub fn is_grapheme_start(self) -> bool {
        self.flags.is_grapheme_start()
    }

    /// Whether breaking logically before this shaped cluster requires reshaping.
    ///
    /// Note that if this shaped cluster does not start a grapheme (see
    /// [`Self::is_grapheme_start`]), you have to reshape regardless of this value.
    #[inline(always)]
    pub fn is_safe_to_break_before(self) -> bool {
        self.flags.is_safe_to_break_before()
    }

    /// The number of graphemes this cluster overlaps.
    pub(crate) fn graphemes_overlapped(&self, characters: &[Character]) -> u32 {
        let start = self.chars_range().start as usize + 1;
        let end = self.chars_range().end as usize;
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
