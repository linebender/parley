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
pub struct ClusterInfo {
    boundary: Boundary,
    source_char: char,
}

impl ClusterInfo {
    pub fn new(boundary: Boundary, source_char: char) -> Self {
        Self {
            boundary,
            source_char,
        }
    }

    // Returns the boundary type of the cluster.
    pub fn boundary(self) -> Boundary {
        self.boundary
    }

    // Returns the whitespace type of the cluster.
    pub fn whitespace(self) -> Whitespace {
        to_whitespace(self.source_char)
    }

    /// Returns if the cluster is a line boundary.
    pub fn is_boundary(self) -> bool {
        self.boundary != Boundary::None
    }

    /// Returns if the cluster is an emoji.
    pub fn is_emoji(self) -> bool {
        // TODO: Defer to ICU4X properties (see: https://docs.rs/icu/latest/icu/properties/props/struct.Emoji.html).
        matches!(self.source_char as u32, 0x1F600..=0x1F64F | 0x1F300..=0x1F5FF | 0x1F680..=0x1F6FF | 0x2600..=0x26FF | 0x2700..=0x27BF)
    }

    /// Returns if the cluster is any whitespace.
    pub fn is_whitespace(self) -> bool {
        self.source_char.is_whitespace()
    }

    /// Returns the cluster's original character.
    pub fn source_char(self) -> char {
        self.source_char
    }
}


// TODO: should become private when more of `parley`'s shaping is in `parley_core`
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
