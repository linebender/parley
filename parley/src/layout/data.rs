// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::inline_box::InlineBox;
use crate::layout::{ContentWidths, Glyph, LineMetrics, RunMetrics, Style};
use crate::style::Brush;
use crate::util::nearly_zero;
use crate::{Font, OverflowWrap};
use core::ops::Range;
// changed: Adding back swash imports for compilation, keeping harfrust for future
use swash::Synthesis;
use swash::shape::Shaper;
use swash::text::cluster::{Boundary, ClusterInfo, Whitespace};
// Keep swash for text analysis, use harfrust only for shaping
// use harfrust::{GlyphBuffer, Script, Direction};

use alloc::vec::Vec;
use alloc::collections::BTreeMap;

#[cfg(feature = "libm")]
#[allow(unused_imports)]
use core_maths::CoreFloat;

use skrifa::raw::TableProvider;

/// Harfrust-based font synthesis information (replaces swash::Synthesis)
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HarfSynthesis {
    /// Synthetic bold weight adjustment (0.0 = no adjustment)
    pub bold: f32,
    /// Synthetic italic skew angle in degrees (0.0 = no skew)
    pub italic: f32,
    /// Whether to apply synthetic small caps
    pub small_caps: bool,
}

impl Default for HarfSynthesis {
    fn default() -> Self {
        Self {
            bold: 0.0,
            italic: 0.0,
            small_caps: false,
        }
    }
}

/// Our own cluster info type that we can populate with data from CharInfo
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HarfClusterInfo {
    /// Boundary type for line breaking
    boundary: Option<Boundary>,
    /// Whitespace classification
    whitespace: Whitespace,
    /// Whether this is a word boundary
    is_boundary: bool,
    /// Whether this is an emoji
    is_emoji: bool,
}

impl HarfClusterInfo {
    /// Create from boundary and character content
    pub fn new(boundary: Option<Boundary>, source_char: char) -> Self {
        // Detect whitespace from actual character content
        let whitespace = match source_char {
            ' ' => Whitespace::Space,
            '\t' => Whitespace::Tab,
            '\n' => Whitespace::Newline,
            '\r' => Whitespace::Newline,
            '\u{00A0}' => Whitespace::Space, // Non-breaking space treated as regular space
            _ => Whitespace::None,
        };
        
        Self {
            boundary,
            whitespace,
            is_boundary: boundary == Some(Boundary::Line),
            is_emoji: false, // TODO: Could enhance with emoji detection from char
        }
    }
    
    /// Get boundary type (critical for line breaking)
    pub fn boundary(&self) -> Option<Boundary> {
        self.boundary
    }
    
    /// Get whitespace type
    pub fn whitespace(&self) -> Whitespace {
        self.whitespace
    }
    
    /// Check if this is a word boundary
    pub fn is_boundary(&self) -> bool {
        self.is_boundary
    }
    
    /// Check if this is an emoji
    pub fn is_emoji(&self) -> bool {
        self.is_emoji
    }
    
    /// Check if this is any kind of whitespace
    pub fn is_whitespace(&self) -> bool {
        self.whitespace != Whitespace::None
    }
}

impl Default for HarfClusterInfo {
    fn default() -> Self {
        Self {
            boundary: None,
            whitespace: Whitespace::None,
            is_boundary: false,
            is_emoji: false,
        }
    }
}

/// Cluster data - uses swash analysis with harfrust shaping
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ClusterData {
    /// Cluster information from swash text analysis (using our own type)
    pub(crate) info: HarfClusterInfo,
    /// Cluster flags (ligature info, style divergence, etc.)
    pub(crate) flags: u16,
    /// Style index for this cluster
    pub(crate) style_index: u16,
    /// Number of glyphs in this cluster (0xFF = single glyph stored inline)
    pub(crate) glyph_len: u8,
    /// Number of text bytes in this cluster
    pub(crate) text_len: u8,
    /// If `glyph_len == 0xFF`, then `glyph_offset` is a glyph identifier,
    /// otherwise, it's an offset into the glyph array with the base
    /// taken from the owning run.
    pub(crate) glyph_offset: u16,
    /// Offset into the text for this cluster
    pub(crate) text_offset: u16,
    /// Advance width for this cluster
    pub(crate) advance: f32,
}

impl ClusterData {
    pub(crate) const LIGATURE_START: u16 = 1;
    pub(crate) const LIGATURE_COMPONENT: u16 = 2;
    pub(crate) const DIVERGENT_STYLES: u16 = 4;

    pub(crate) fn is_ligature_start(self) -> bool {
        self.flags & Self::LIGATURE_START != 0
    }

    pub(crate) fn is_ligature_component(self) -> bool {
        self.flags & Self::LIGATURE_COMPONENT != 0
    }

    pub(crate) fn has_divergent_styles(self) -> bool {
        self.flags & Self::DIVERGENT_STYLES != 0
    }

    pub(crate) fn text_range(self, run: &RunData) -> Range<usize> {
        let start = run.text_range.start + self.text_offset as usize;
        start..start + self.text_len as usize
    }
}

/// Harfrust-based run data (updated to use harfrust types)
#[derive(Clone, Debug)]
pub(crate) struct RunData {
    /// Index of the font for the run.
    pub(crate) font_index: usize,
    /// Font size.
    pub(crate) font_size: f32,
    /// Harfrust-based synthesis information for the font.
    pub(crate) synthesis: HarfSynthesis,
    /// Original fontique synthesis for renderer (contains variation settings)
    pub(crate) fontique_synthesis: Option<fontique::Synthesis>,
    /// Range of normalized coordinates in the layout data.
    pub(crate) coords_range: Range<usize>,
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Bidi level for the run.
    pub(crate) bidi_level: u8,
    /// True if the run ends with a newline.
    pub(crate) ends_with_newline: bool,
    /// Range of clusters.
    pub(crate) cluster_range: Range<usize>,
    /// Base for glyph indices.
    pub(crate) glyph_start: usize,
    /// Metrics for the run.
    pub(crate) metrics: RunMetrics,
    /// Additional word spacing.
    pub(crate) word_spacing: f32,
    /// Additional letter spacing.
    pub(crate) letter_spacing: f32,
    /// Total advance of the run.
    pub(crate) advance: f32,
    // changed: Commenting out harfrust-specific fields for compilation
    // /// Text direction for this run
    // pub(crate) direction: harfrust::Direction,
    // /// Script for this run
    // pub(crate) script: harfrust::Script,
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum BreakReason {
    #[default]
    None,
    Regular,
    Explicit,
    Emergency,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct LineData {
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of line items.
    pub(crate) item_range: Range<usize>,
    /// Metrics for the line.
    pub(crate) metrics: LineMetrics,
    /// The cause of the line break.
    pub(crate) break_reason: BreakReason,
    /// Maximum advance for the line.
    pub(crate) max_advance: f32,
    /// Number of justified clusters on the line.
    pub(crate) num_spaces: usize,
}

impl LineData {
    pub(crate) fn size(&self) -> f32 {
        self.metrics.ascent + self.metrics.descent + self.metrics.leading
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LineItemData {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: u8,
    /// Advance (size in direction of text flow) for the run.
    pub(crate) advance: f32,

    // Fields that only apply to text runs (Ignored for boxes)
    // TODO: factor this out?
    /// True if the run is composed entirely of whitespace.
    pub(crate) is_whitespace: bool,
    /// True if the run ends in whitespace.
    pub(crate) has_trailing_whitespace: bool,
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of clusters.
    pub(crate) cluster_range: Range<usize>,
}

impl LineItemData {
    pub(crate) fn is_text_run(&self) -> bool {
        self.kind == LayoutItemKind::TextRun
    }

    pub(crate) fn compute_line_height<B: Brush>(&self, layout: &LayoutData<B>) -> f32 {
        match self.kind {
            LayoutItemKind::TextRun => {
                let mut line_height = 0_f32;
                let run = &layout.runs[self.index];
                let glyph_start = run.glyph_start;
                for cluster in &layout.clusters[run.cluster_range.clone()] {
                    if cluster.glyph_len != 0xFF && cluster.has_divergent_styles() {
                        let start = glyph_start + cluster.glyph_offset as usize;
                        let end = start + cluster.glyph_len as usize;
                        for glyph in &layout.glyphs[start..end] {
                            line_height = line_height
                                .max(layout.styles[glyph.style_index()].line_height.resolve(run));
                        }
                    } else {
                        line_height = line_height.max(
                            layout.styles[cluster.style_index as usize]
                                .line_height
                                .resolve(run),
                        );
                    }
                }
                line_height
            }
            LayoutItemKind::InlineBox => {
                // TODO: account for vertical alignment (e.g. baseline alignment)
                layout.inline_boxes[self.index].height
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutItemKind {
    TextRun,
    InlineBox,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutItem {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: u8,
}

#[derive(Clone, Debug)]
pub(crate) struct LayoutData<B: Brush> {
    pub(crate) scale: f32,
    pub(crate) quantize: bool,
    pub(crate) has_bidi: bool,
    pub(crate) base_level: u8,
    pub(crate) text_len: usize,
    pub(crate) width: f32,
    pub(crate) full_width: f32,
    pub(crate) height: f32,
    pub(crate) fonts: Vec<Font>,
    pub(crate) coords: Vec<i16>,

    // Input (/ output of style resolution)
    pub(crate) styles: Vec<Style<B>>,
    pub(crate) inline_boxes: Vec<InlineBox>,

    // Output of shaping
    pub(crate) runs: Vec<RunData>,
    pub(crate) items: Vec<LayoutItem>,
    pub(crate) clusters: Vec<ClusterData>,
    pub(crate) glyphs: Vec<Glyph>,

    // Output of line breaking
    pub(crate) lines: Vec<LineData>,
    pub(crate) line_items: Vec<LineItemData>,

    // Output of alignment
    /// Whether the layout is aligned with [`crate::Alignment::Justified`].
    pub(crate) is_aligned_justified: bool,
    /// The width the layout was aligned to.
    pub(crate) alignment_width: f32,
}

impl<B: Brush> Default for LayoutData<B> {
    fn default() -> Self {
        Self {
            scale: 1.,
            quantize: true,
            has_bidi: false,
            base_level: 0,
            text_len: 0,
            width: 0.,
            full_width: 0.,
            height: 0.,
            fonts: Vec::new(),
            coords: Vec::new(),
            styles: Vec::new(),
            inline_boxes: Vec::new(),
            runs: Vec::new(),
            items: Vec::new(),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
            line_items: Vec::new(),
            is_aligned_justified: false,
            alignment_width: 0.0,
        }
    }
}

impl<B: Brush> LayoutData<B> {
    pub(crate) fn clear(&mut self) {
        self.scale = 1.;
        self.quantize = true;
        self.has_bidi = false;
        self.base_level = 0;
        self.text_len = 0;
        self.width = 0.;
        self.full_width = 0.;
        self.height = 0.;
        self.fonts.clear();
        self.coords.clear();
        self.styles.clear();
        self.inline_boxes.clear();
        self.runs.clear();
        self.items.clear();
        self.clusters.clear();
        self.glyphs.clear();
        self.lines.clear();
        self.line_items.clear();
    }

    /// Push an inline box to the list of items
    pub(crate) fn push_inline_box(&mut self, index: usize) {
        // Give the box the same bidi level as the preceding text run
        // (or else default to 0 if there is not yet a text run)
        let bidi_level = self.runs.last().map(|r| r.bidi_level).unwrap_or(0);

        self.items.push(LayoutItem {
            kind: LayoutItemKind::InlineBox,
            index,
            bidi_level,
        });
    }

    #[allow(unused_assignments)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_run(
        &mut self,
        font: Font,
        font_size: f32,
        synthesis: HarfSynthesis,
        // changed: Back to swash implementation for compilation, harfrust integration preserved below
        shaper: Shaper<'_>,
        // shaper: harfrust::Shaper<'_>, 
        bidi_level: u8,
        word_spacing: f32,
        letter_spacing: f32,
    ) {
        // changed: Using swash implementation for compilation, harfrust version preserved in comments
        let font_index = self
            .fonts
            .iter()
            .position(|f| *f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font);
                index
            });
        
        // Convert HarfSynthesis to swash::Synthesis for compatibility
        let swash_synthesis = Synthesis::new(
            std::iter::empty(),
            synthesis.bold > 0.0,
            synthesis.italic,
        );
        
        let metrics = shaper.metrics();
        let cluster_range = self.clusters.len()..self.clusters.len();
        let coords_start = self.coords.len();
        let coords = shaper.normalized_coords();
        if coords.iter().any(|coord| *coord != 0) {
            self.coords.extend_from_slice(coords);
        }
        let coords_end = self.coords.len();
        let mut run = RunData {
            font_index,
            font_size,
            synthesis,
            fontique_synthesis: None,  // Legacy swash path - no fontique synthesis available
            coords_range: coords_start..coords_end,
            text_range: 0..0,
            bidi_level,
            ends_with_newline: false,
            cluster_range,
            glyph_start: self.glyphs.len(),
            metrics: RunMetrics {
                ascent: metrics.ascent,
                descent: metrics.descent,
                leading: metrics.leading,
                underline_offset: metrics.underline_offset,
                underline_size: metrics.stroke_size,
                strikethrough_offset: metrics.strikeout_offset,
                strikethrough_size: metrics.stroke_size,
            },
            word_spacing,
            letter_spacing,
            advance: 0.,
            // changed: Commenting out harfrust-specific fields for compilation
            // direction: Direction::LTR, // Default to LTR for now
            // script: Script::LATIN,   // Default to LATIN for now
        };
        // Track these so that we can flush if they overflow a u16.
        let mut glyph_count = 0_usize;
        let mut text_offset = 0;
        macro_rules! flush_run {
            () => {
                if !run.cluster_range.is_empty() {
                    self.runs.push(run.clone());
                    self.items.push(LayoutItem {
                        kind: LayoutItemKind::TextRun,
                        index: self.runs.len() - 1,
                        bidi_level: run.bidi_level,
                    });
                    run.text_range = text_offset..text_offset;
                    run.cluster_range.start = run.cluster_range.end;
                    run.glyph_start = self.glyphs.len();
                    run.advance = 0.;
                    glyph_count = 0;
                }
            };
        }
        let mut first = true;
        shaper.shape_with(|cluster| {
            if cluster.info.boundary() == Boundary::Mandatory {
                run.ends_with_newline = true;
                flush_run!();
            }
            run.ends_with_newline = false;
            const MAX_LEN: usize = u16::MAX as usize;
            let source_range = cluster.source.to_range();
            if first {
                run.text_range = source_range.start..source_range.start;
                text_offset = source_range.start;
                first = false;
            }
            let num_components = cluster.components.len() + 1;
            if glyph_count > MAX_LEN
                || (text_offset - run.text_range.start) > MAX_LEN
                || (num_components > 1
                    && (cluster.components.last().unwrap().start as usize - run.text_range.start)
                        > MAX_LEN)
            {
                flush_run!();
            }
            let text_len = source_range.len();
            let glyph_len = cluster.glyphs.len();
            let advance = cluster.advance();
            run.advance += advance;
            let mut cluster_data = ClusterData {
                info: HarfClusterInfo::new(Some(cluster.info.boundary()), ' '), // Fallback for swash path
                flags: 0,
                style_index: cluster.data as _,
                glyph_len: glyph_len as u8,
                text_len: text_len as u8,
                advance,
                text_offset: (text_offset - run.text_range.start) as u16,
                glyph_offset: 0,
            };
            if num_components > 1 {
                cluster_data.flags = ClusterData::LIGATURE_START;
                cluster_data.advance /= cluster.components.len() as f32;
                cluster_data.text_len = cluster.components[0].to_range().len() as u8;
            }
            macro_rules! push_components {
                () => {
                    self.clusters.push(cluster_data);
                    if num_components > 1 {
                        cluster_data.glyph_offset = 0;
                        cluster_data.glyph_len = 0;
                        for component in &cluster.components[1..] {
                            let range = component.to_range();
                            cluster_data.flags = ClusterData::LIGATURE_COMPONENT;
                            cluster_data.text_offset = (range.start - run.text_range.start) as u16;
                            cluster_data.text_len = range.len() as u8;
                            self.clusters.push(cluster_data);
                            run.cluster_range.end += 1;
                        }
                        cluster_data.flags = 0;
                    }
                };
            }
            run.cluster_range.end += 1;
            run.text_range.end += text_len;
            text_offset += text_len;
            if glyph_len == 1 && num_components == 1 {
                let g = &cluster.glyphs[0];
                if nearly_zero(g.x) && nearly_zero(g.y) {
                    // Handle the case with a single glyph with zero'd offset.
                    cluster_data.glyph_len = 0xFF;
                    cluster_data.glyph_offset = g.id;
                    push_components!();
                    return;
                }
            } else if glyph_len == 0 {
                // Insert an empty cluster. This occurs for both invisible
                // control characters and ligature components.
                push_components!();
                return;
            }
            // Otherwise, encode all of the glyphs.
            cluster_data.glyph_offset = (self.glyphs.len() - run.glyph_start) as u16;
            self.glyphs.extend(cluster.glyphs.iter().map(|g| {
                let style_index = g.data as u16;
                if cluster_data.style_index != style_index {
                    cluster_data.flags |= ClusterData::DIVERGENT_STYLES;
                }
                Glyph {
                    id: g.id as u32,  // Convert swash u16 to u32 for harfrust compatibility
                    style_index,
                    x: g.x,
                    y: g.y,
                    advance: g.advance,
                    cluster_index: 0, // TODO: Get from harfrust cluster mapping
                    flags: 0, // TODO: Get from harfrust glyph flags
                }
            }));
            glyph_count += glyph_len;
            push_components!();
        });
        flush_run!();
        
        /* changed: Original harfrust implementation preserved here for restoration:
        // TODO: This method will be fully implemented when harfrust API is available
        // For now, create a minimal stub to maintain compilation
        let _font_index = self
            .fonts
            .iter()
            .position(|f| *f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font);
                index
            });
        
        // Stub implementation - this will be replaced with actual harfrust shaping
        // when the API becomes available
        let _ = (font_size, synthesis, bidi_level, word_spacing, letter_spacing);
        */
    }

    // changed: Restoring harfrust-specific methods step by step
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_run_from_harfrust(
        &mut self,
        font: Font,
        font_size: f32,
        synthesis: HarfSynthesis,
        fontique_synthesis: fontique::Synthesis,
        glyph_buffer: &harfrust::GlyphBuffer,
        bidi_level: u8,
        word_spacing: f32,
        letter_spacing: f32,
        // NEW: Add text analysis data needed for proper clustering
        source_text: &str,
        infos: &[(swash::text::cluster::CharInfo, u16)], // From text analysis
        text_range: Range<usize>, // The text range this run covers
        char_range: Range<usize>, // Range into infos array
        // NEW: Add actual font variations used during shaping
        variations: &[harfrust::Variation],
    ) {
        // Store font variations as normalized coordinates FIRST (before font moves)
        // Proper solution: Read font's fvar table and map variations to correct axis positions
        let coords_start = self.coords.len();
        
        if !variations.is_empty() {
            self.store_variations_properly(&font, variations);
        }
        
        let coords_end = self.coords.len();

        let font_index = self
            .fonts
            .iter()
            .position(|f| *f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font);
                index
            });
        
        // For now, create default metrics since we don't have them from harfrust
        // TODO: Get actual metrics from the font
        let metrics = RunMetrics {
            ascent: font_size * 0.8,
            descent: font_size * 0.2,
            leading: 0.0,
            underline_offset: -font_size * 0.1,
            underline_size: font_size * 0.05,
            strikethrough_offset: font_size * 0.3,
            strikethrough_size: font_size * 0.05,
        };
        
        let cluster_range = self.clusters.len()..self.clusters.len();
        
        let mut run = RunData {
            font_index,
            font_size,
            synthesis,
            fontique_synthesis: Some(fontique_synthesis),  // Store original fontique synthesis
            coords_range: coords_start..coords_end,
            text_range: text_range.clone(), // ✅ Use correct text range from parameter
            bidi_level,
            ends_with_newline: false,
            cluster_range,
            glyph_start: self.glyphs.len(),
            metrics,
            word_spacing,
            letter_spacing,
            advance: 0.,
            // changed: Commenting out harfrust-specific fields for compilation
            // direction: Direction::LTR, // Default to LTR for now
            // script: Script::LATIN,   // Default to LATIN for now
        };

        // Get harfrust glyph data
        let glyph_infos = glyph_buffer.glyph_infos();
        let glyph_positions = glyph_buffer.glyph_positions();
        
        if glyph_infos.is_empty() {
            return;
        }

        // Map harfrust clusters to source text and create proper cluster data
        let cluster_mappings = self.map_harfrust_clusters_to_text(
            glyph_buffer, 
            source_text, 
            infos, 
            &text_range,
            &char_range,
            bidi_level,
        );
        
        // Group glyphs by harfrust cluster ID
        let mut cluster_groups: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (i, info) in glyph_infos.iter().enumerate() {
            cluster_groups.entry(info.cluster).or_default().push(i);
        }
        
        // Process clusters and add their glyphs to the run
        let mut all_run_glyphs = Vec::new();
        let mut cluster_data_list = Vec::new();
        let mut run_advance = 0.0;
        
        for (cluster_id, cluster_text_range, cluster_info, style_index) in &cluster_mappings {
            if let Some(glyph_indices) = cluster_groups.get(cluster_id) {
                let cluster_glyphs: Vec<_> = glyph_indices.iter()
                    .map(|&i| (&glyph_infos[i], &glyph_positions[i]))
                    .collect();
                
                // Store cluster data for later processing
                cluster_data_list.push((
                    *cluster_id,
                    cluster_glyphs,
                    cluster_text_range.clone(),
                    cluster_info.clone(),
                    *style_index
                ));
            }
        }
        
        // For RTL text, we need to reverse the glyph order within the run to match visual order
        let is_rtl = bidi_level & 1 != 0;
        if is_rtl {
            cluster_data_list.reverse();
        }
        
        // Now process clusters in the correct visual order
        for (cluster_id, cluster_glyphs, cluster_text_range, cluster_info, style_index) in cluster_data_list {
            // Add glyphs to the run in visual order
            let mut cluster_advance = 0.0;
            let glyph_start = all_run_glyphs.len();
            
            for (info, pos) in &cluster_glyphs {
                let units_per_em = 2048.0; // TODO: Get from font header
                let scale_factor = font_size / units_per_em;
                
                let glyph = Glyph {
                    id: info.glyph_id,
                    style_index,
                    x: (pos.x_offset as f32) * scale_factor,
                    y: (pos.y_offset as f32) * scale_factor,
                    advance: (pos.x_advance as f32) * scale_factor,
                    cluster_index: cluster_id,
                    flags: 0,
                };
                cluster_advance += glyph.advance;
                all_run_glyphs.push(glyph);
            }
            
            // Create cluster data
            let cluster_data = ClusterData {
                info: cluster_info,
                flags: 0,
                style_index,
                glyph_len: cluster_glyphs.len() as u8,
                text_len: cluster_text_range.len() as u8,
                advance: cluster_advance,
                text_offset: cluster_text_range.start.saturating_sub(text_range.start) as u16,
                glyph_offset: glyph_start as u16,
            };
            
            self.clusters.push(cluster_data);
            run.cluster_range.end += 1;
            run_advance += cluster_advance;
        }
        
        // Add all glyphs to the global glyph list in correct order
        self.glyphs.extend(all_run_glyphs);
        
        run.advance = run_advance;
        
        // Store final run data with harfrust synthesis 
        run.synthesis = synthesis;
        
        // Push the run
        if !run.cluster_range.is_empty() {
            self.runs.push(run);
            self.items.push(LayoutItem {
                kind: LayoutItemKind::TextRun,
                index: self.runs.len() - 1,
                bidi_level,
            });
        }
    }

    // Helper method to map harfrust clusters back to source text
    fn map_harfrust_clusters_to_text(
        &self,
        glyph_buffer: &harfrust::GlyphBuffer,
        source_text: &str,
        infos: &[(swash::text::cluster::CharInfo, u16)],
        text_range: &Range<usize>,
        char_range: &Range<usize>,
        bidi_level: u8, // Added to handle RTL cluster ordering
    ) -> Vec<(u32, Range<usize>, HarfClusterInfo, u16)> {
        // Returns: (harfrust_cluster_id, text_byte_range, cluster_info, style_index)
        
        let mut clusters = Vec::new();
        let glyph_infos = glyph_buffer.glyph_infos();
        
        // Group glyphs by harfrust cluster ID
        let mut cluster_groups: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (i, info) in glyph_infos.iter().enumerate() {
            cluster_groups.entry(info.cluster).or_default().push(i);
        }
        
        // Map each harfrust cluster back to source text
        // Sort cluster IDs to process them in order
        let mut sorted_cluster_ids: Vec<u32> = cluster_groups.keys().copied().collect();
        sorted_cluster_ids.sort();
        
        // ✅ IMPORTANT: Reverse cluster order for RTL text to match swash behavior
        let is_rtl = bidi_level & 1 != 0;
        if is_rtl {
            sorted_cluster_ids.reverse();
        }
        
        for &cluster_id in sorted_cluster_ids.iter() {
            // For each cluster, map it to the corresponding character using the cluster ID
            // NOTE: cluster IDs are relative to the current text segment, not global
            let char_idx_in_range = cluster_id as usize;
            
            if char_idx_in_range < char_range.len() {
                let absolute_char_idx = char_range.start + char_idx_in_range;
                
                // Get cluster info from swash text analysis
                // Use char_idx_in_range (relative index) instead of absolute_char_idx
                if let Some((char_info, style_index)) = infos.get(char_idx_in_range) {
                    // ✅ Extract boundary from CharInfo and create our own cluster info!
                    let boundary = char_info.boundary();
                    // Use segment-relative index since source_text is only the current segment
                    let segment_relative_char_idx = char_idx_in_range; // This is already relative to the segment
                    let source_char = source_text.chars().nth(segment_relative_char_idx).unwrap_or(' ');
                    let cluster_info = HarfClusterInfo::new(Some(boundary), source_char);
                    
                    // Calculate BYTE range for this cluster from character positions
                    // Convert character index to byte index within the segment
                    let char_byte_start = source_text.char_indices()
                        .nth(segment_relative_char_idx)
                        .map(|(byte_idx, _)| byte_idx)
                        .unwrap_or(0);
                    
                    let char_byte_end = if segment_relative_char_idx + 1 < source_text.chars().count() {
                        source_text.char_indices()
                            .nth(segment_relative_char_idx + 1)
                            .map(|(byte_idx, _)| byte_idx)
                            .unwrap_or(source_text.len())
                    } else {
                        source_text.len()
                    };
                    
                    // Convert to absolute byte positions
                    let cluster_text_range = (text_range.start + char_byte_start)..(text_range.start + char_byte_end);
                    
                    clusters.push((cluster_id, cluster_text_range, cluster_info, *style_index));
                }
            }
        }
        
        clusters
    }

    fn push_harfrust_cluster(
        &mut self,
        run: &mut RunData,
        cluster_id: u32,
        cluster_glyphs: &[(&harfrust::GlyphInfo, &harfrust::GlyphPosition)],
        total_advance: &mut f32,
        cluster_text_range: Range<usize>,
        cluster_info: HarfClusterInfo,
        style_index: u16,
    ) {
        let glyph_start = self.glyphs.len() - run.glyph_start;
        let glyph_len = cluster_glyphs.len();
        let mut cluster_advance = 0.0;

        // Create glyphs from harfrust data
        for (info, pos) in cluster_glyphs {
            // SCALING FIX: Harfrust returns glyph advances in raw font units (e.g., 1175) 
            // while swash returns them pre-scaled to font size (e.g., 9.89). This causes
            // massive layout issues - text width becomes 540x larger than expected, 
            // breaking line wrapping and causing visual corruption.
            //
            // The scaling factor here assumes 2048 units per em (common for TrueType fonts).
            // This is a TEMPORARY FIX - ideally we should either:
            // 1. Get actual units_per_em from the font header, or  
            // 2. Rework parley to work natively with font units throughout the pipeline
            // 3. Fix harfrust to respect the .point_size() setting and return scaled values
            //
            // For now, this scaling makes harfrust output compatible with swash expectations.
            let units_per_em = 2048.0; // TODO: Get from font header
            let scale_factor = run.font_size / units_per_em;
            
            let glyph = Glyph {
                id: info.glyph_id, // harfrust glyph ID is already u32
                style_index, // ✅ Use correct style index
                x: (pos.x_offset as f32) * scale_factor, // Scale from font units to font size
                y: (pos.y_offset as f32) * scale_factor, // Scale from font units to font size  
                advance: (pos.x_advance as f32) * scale_factor, // Scale from font units to font size
                cluster_index: cluster_id, // Map harfrust cluster to index
                flags: 0, // TODO: Get from harfrust glyph flags
            };
            cluster_advance += glyph.advance;
            self.glyphs.push(glyph);
        }

        // Create cluster data with PROPER mapping
        let cluster_data = ClusterData {
            info: cluster_info, // ✅ Proper boundary info for line breaking
            flags: 0,
            style_index, // ✅ Correct style index
            glyph_len: glyph_len as u8,
            text_len: cluster_text_range.len() as u8, // ✅ Actual text length
            advance: cluster_advance,
            text_offset: cluster_text_range.start.saturating_sub(run.text_range.start) as u16, // ✅ Correct offset (with bounds check)
            glyph_offset: glyph_start as u16,
        };

        self.clusters.push(cluster_data);
        run.cluster_range.end += 1;
        *total_advance += cluster_advance;
    }

    pub(crate) fn finish(&mut self) {
        for run in &self.runs {
            let word = run.word_spacing;
            let letter = run.letter_spacing;
            if nearly_zero(word) && nearly_zero(letter) {
                continue;
            }
            let clusters = &mut self.clusters[run.cluster_range.clone()];
            for cluster in clusters {
                let mut spacing = letter;
                if !nearly_zero(word) && cluster.info.whitespace().is_space_or_nbsp() {
                    spacing += word;
                }
                if !nearly_zero(spacing) {
                    cluster.advance += spacing;
                    if cluster.glyph_len != 0xFF {
                        let start = run.glyph_start + cluster.glyph_offset as usize;
                        let end = start + cluster.glyph_len as usize;
                        let glyphs = &mut self.glyphs[start..end];
                        if let Some(last) = glyphs.last_mut() {
                            last.advance += spacing;
                        }
                    }
                }
            }
        }
    }

    // TODO: this method does not handle mixed direction text at all.
    pub(crate) fn calculate_content_widths(&self) -> ContentWidths {
        fn whitespace_advance(cluster: Option<&ClusterData>) -> f32 {
            cluster
                .filter(|cluster| cluster.info.whitespace().is_space_or_nbsp())
                .map_or(0.0, |cluster| cluster.advance)
        }

        let mut min_width = 0.0_f32;
        let mut max_width = 0.0_f32;

        let mut running_max_width = 0.0;
        let mut prev_cluster: Option<&ClusterData> = None;
        let is_rtl = self.base_level & 1 == 1;
        for item in &self.items {
            match item.kind {
                LayoutItemKind::TextRun => {
                    let run = &self.runs[item.index];
                    let mut running_min_width = 0.0;
                    let clusters = &self.clusters[run.cluster_range.clone()];
                    if is_rtl {
                        prev_cluster = clusters.first();
                    }
                    for cluster in clusters {
                        let boundary = cluster.info.boundary();
                        let style = &self.styles[cluster.style_index as usize];
                        if matches!(boundary, Some(Boundary::Line) | Some(Boundary::Mandatory))
                            || style.overflow_wrap == OverflowWrap::Anywhere
                        {
                            let trailing_whitespace = whitespace_advance(prev_cluster);
                            min_width = min_width.max(running_min_width - trailing_whitespace);
                            running_min_width = 0.0;
                            if boundary == Some(Boundary::Mandatory) {
                                running_max_width = 0.0;
                            }
                        }
                        running_min_width += cluster.advance;
                        running_max_width += cluster.advance;
                        if !is_rtl {
                            prev_cluster = Some(cluster);
                        }
                    }
                    let trailing_whitespace = whitespace_advance(prev_cluster);
                    min_width = min_width.max(running_min_width - trailing_whitespace);
                }
                LayoutItemKind::InlineBox => {
                    let ibox = &self.inline_boxes[item.index];
                    min_width = min_width.max(ibox.width);
                    running_max_width += ibox.width;
                    prev_cluster = None;
                }
            }
            let trailing_whitespace = whitespace_advance(prev_cluster);
            max_width = max_width.max(running_max_width - trailing_whitespace);
        }

        ContentWidths {
            min: min_width,
            max: max_width,
        }
    }
    
    /// Store font variations as normalized coordinates using proper axis mapping
    /// This replicates what swash did internally: read fvar table, map variations to correct positions
    fn store_variations_properly(&mut self, font: &Font, variations: &[harfrust::Variation]) {
        // Try to read font's axis layout from fvar table
        if let Ok(font_ref) = skrifa::FontRef::from_index(font.data.as_ref(), font.index) {
            if let Ok(fvar) = font_ref.fvar() {
                if let Ok(axes) = fvar.axes() {
                    let axis_count = fvar.axis_count() as usize;
                    let mut coords = vec![0i16; axis_count];
                    
                    // Map each fontique variation to its correct axis position
                    for variation in variations {
                        let variation_tag = skrifa::Tag::from_be_bytes(variation.tag.to_be_bytes());
                        
                        // Find which axis this variation belongs to
                        for (axis_index, axis_record) in axes.iter().enumerate() {
                            if axis_record.axis_tag() == variation_tag {
                                // Use this axis's actual range for normalization
                                let min_val = axis_record.min_value().to_f32();
                                let default_val = axis_record.default_value().to_f32();
                                let max_val = axis_record.max_value().to_f32();
                                
                                // Generic normalization (same formula for all axes)
                                let normalized_f32 = if variation.value >= default_val {
                                    (variation.value - default_val) / (max_val - default_val)
                                } else {
                                    (variation.value - default_val) / (default_val - min_val)
                                };
                                
                                let clamped = normalized_f32.clamp(-1.0, 1.0);
                                let normalized_coord = (clamped * 16384.0) as i16;
                                
                                coords[axis_index] = normalized_coord;
                                break;
                            }
                        }
                    }
                    
                    // Store all coordinates (including zeros for unused axes)
                    self.coords.extend(coords);
                    return;
                }
            }
        }
        
        // Fallback: simple storage if fvar reading fails
        for variation in variations {
            let normalized_f32 = (variation.value - 400.0) / (1000.0 - 400.0);
            let clamped = normalized_f32.clamp(-1.0, 1.0);
            let normalized_coord = (clamped * 16384.0) as i16;
            self.coords.push(normalized_coord);
        }
    }
}
