// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shaped data

use crate::{
    Brush, InlineBox, ResolvedStyle,
    analysis::{Boundary, cluster::Whitespace},
};
use alloc::vec::Vec;
use core::ops::Range;
use linebender_resource_handle::FontData;

use super::ShapeSink;

#[derive(Debug, Clone, Default)]
pub struct ShapedText<B: Brush> {
    pub base_level: u8,
    pub text_len: usize,

    // Input (/ output of style resolution)
    pub styles: Vec<ResolvedStyle<B>>,
    pub inline_boxes: Vec<InlineBox>,

    // Output of shaping
    pub runs: Vec<RunData>,
    pub items: Vec<LayoutItem>,
    pub clusters: Vec<ClusterData>,
    pub glyphs: Vec<Glyph>,
    pub fonts: Vec<FontData>,
    pub coords: Vec<i16>,
}

impl<B: Brush> ShapedText<B> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<B: Brush> ShapeSink<B> for ShapedText<B> {
    fn set_scale(&mut self, _scale: f32) {
        // Do nothing
    }

    fn set_quantize(&mut self, _quantize: bool) {
        // Do nothing
    }

    fn set_base_level(&mut self, level: u8) {
        self.base_level = level;
    }

    fn set_text_len(&mut self, len: usize) {
        self.text_len = len;
    }

    fn push_coords(&mut self, coords: &[harfrust::NormalizedCoord]) -> (usize, usize) {
        let coords_start = self.coords.len();
        self.coords.extend(coords.iter().map(|c| c.to_bits()));
        let coords_end = self.coords.len();
        (coords_start, coords_end)
    }

    fn push_font(&mut self, font: &FontData) -> usize {
        self.fonts
            .iter()
            .position(|f| f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font.clone());
                index
            })
    }

    fn push_cluster(&mut self, cluster: ClusterData) {
        self.clusters.push(cluster);
    }

    fn push_glyph(&mut self, glyph: Glyph) {
        self.glyphs.push(glyph);
    }

    fn push_run(&mut self, run: RunData) {
        self.runs.push(run);
    }

    fn push_item(&mut self, item: LayoutItem) {
        self.items.push(item);
    }

    fn push_inline_box_item(&mut self, index: usize) {
        // Give the box the same bidi level as the preceding text run
        // (or else default to 0 if there is not yet a text run)
        let bidi_level = self.runs.last().map(|r| r.bidi_level).unwrap_or(0);

        self.items.push(LayoutItem {
            kind: LayoutItemKind::InlineBox,
            index,
            bidi_level,
        });
    }

    fn set_inline_boxes(&mut self, boxes: Vec<InlineBox>) -> Vec<InlineBox> {
        let mut old_box_allocation = core::mem::replace(&mut self.inline_boxes, boxes);
        old_box_allocation.clear();
        old_box_allocation
    }

    fn push_styles(&mut self, styles: &[ResolvedStyle<B>]) {
        self.styles.extend(styles.iter().cloned());
    }

    fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    fn run_count(&self) -> usize {
        self.runs.len()
    }

    fn reverse_cluster_range(&mut self, range: Range<usize>) {
        self.clusters[range].reverse();
    }

    fn clear(&mut self) {
        self.base_level = 0;
        self.text_len = 0;
        self.fonts.clear();
        self.coords.clear();
        self.styles.clear();
        self.inline_boxes.clear();
        self.runs.clear();
        self.items.clear();
        self.clusters.clear();
        self.glyphs.clear();
    }

    fn finish(&mut self) {
        // Do nothing
    }
}

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
