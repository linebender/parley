// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shaped data

use crate::analysis::{Boundary, cluster::Whitespace};
use core::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutItemKind {
    TextRun,
    InlineBox,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutItem {
    /// Whether the item is a run or an inline box
    pub kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub index: usize,
    /// Bidi level for the item (used for reordering)
    pub bidi_level: u8,
}

/// `HarfRust`-based run data
#[derive(Clone, Debug, PartialEq)]
pub struct RunData {
    /// Index of the font for the run.
    pub font_index: usize,
    /// Font size.
    pub font_size: f32,
    /// Font attributes, needed for accessibility.
    pub font_attrs: fontique::Attributes,
    /// Synthesis for rendering (contains variation settings)
    pub synthesis: fontique::Synthesis,
    /// Range of normalized coordinates in the layout data.
    pub coords_range: Range<usize>,
    /// Range of the source text.
    pub text_range: Range<usize>,
    /// Bidi level for the run.
    pub bidi_level: u8,
    /// Range of clusters.
    pub cluster_range: Range<usize>,
    /// Base for glyph indices.
    pub glyph_start: usize,
    /// Metrics for the run.
    pub metrics: RunMetrics,
    /// Additional word spacing.
    pub word_spacing: f32,
    /// Additional letter spacing.
    pub letter_spacing: f32,
    /// Total advance of the run.
    pub advance: f32,
}

/// Metrics information for a run.
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct RunMetrics {
    /// Typographic ascent.
    pub ascent: f32,
    /// Typographic descent.
    pub descent: f32,
    /// Typographic leading.
    pub leading: f32,
    /// Offset of the top of underline decoration from the baseline.
    pub underline_offset: f32,
    /// Thickness of the underline decoration.
    pub underline_size: f32,
    /// Offset of the top of strikethrough decoration from the baseline.
    pub strikethrough_offset: f32,
    /// Thickness of the strikethrough decoration.
    pub strikethrough_size: f32,
    /// The line height
    pub line_height: f32,
    /// Distance from the baseline to the top of short lowercase letters.
    pub x_height: Option<f32>,
    /// Distance from the baseline to the top of capital letters.
    pub cap_height: Option<f32>,
}

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

    #[inline(always)]
    pub fn text_range(self, run: &RunData) -> Range<usize> {
        let start = run.text_range.start + self.text_offset as usize;
        start..start + self.text_len as usize
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ClusterInfo {
    boundary: Boundary,
    source_char: char,
}

pub(super) const fn to_whitespace(c: char) -> Whitespace {
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

/// Glyph with an offset and advance.
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub style_index: u16,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

impl Glyph {
    /// Returns the index into the layout style collection.
    pub fn style_index(&self) -> usize {
        self.style_index as usize
    }
}
