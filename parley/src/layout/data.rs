// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::inline_box::InlineBox;
use crate::layout::{Alignment, ContentWidths, Glyph, LineMetrics, RunMetrics, Style};
use crate::style::Brush;
use crate::util::nearly_zero;
use crate::Font;
use core::cell::OnceCell;
use core::ops::Range;
use swash::shape::Shaper;
use swash::text::cluster::{Boundary, ClusterInfo};
use swash::Synthesis;

use alloc::vec::Vec;

#[derive(Copy, Clone)]
pub(crate) struct ClusterData {
    pub(crate) info: ClusterInfo,
    pub(crate) flags: u16,
    pub(crate) style_index: u16,
    pub(crate) glyph_len: u8,
    pub(crate) text_len: u8,
    /// If `glyph_len == 0xFF`, then `glyph_offset` is a glyph identifier,
    /// otherwise, it's an offset into the glyph array with the base
    /// taken from the owning run.
    pub(crate) glyph_offset: u16,
    pub(crate) text_offset: u16,
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

#[derive(Clone)]
pub(crate) struct RunData {
    /// Index of the font for the run.
    pub(crate) font_index: usize,
    /// Font size.
    pub(crate) font_size: f32,
    /// Synthesis information for the font.
    pub(crate) synthesis: Synthesis,
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
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum BreakReason {
    #[default]
    None,
    Regular,
    Explicit,
    Emergency,
}

#[derive(Clone, Default)]
pub(crate) struct LineData {
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of line items.
    pub(crate) item_range: Range<usize>,
    /// Metrics for the line.
    pub(crate) metrics: LineMetrics,
    /// The cause of the line break.
    pub(crate) break_reason: BreakReason,
    /// Alignment.
    pub(crate) alignment: Alignment,
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

#[derive(Debug, Clone)]
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
                            line_height =
                                line_height.max(layout.styles[glyph.style_index()].line_height);
                        }
                    } else {
                        line_height = line_height
                            .max(layout.styles[cluster.style_index as usize].line_height);
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

#[derive(Debug, Clone)]
pub(crate) struct LayoutItem {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: u8,
}

#[derive(Clone)]
pub(crate) struct LayoutData<B: Brush> {
    pub(crate) scale: f32,
    pub(crate) has_bidi: bool,
    pub(crate) base_level: u8,
    pub(crate) text_len: usize,
    pub(crate) width: f32,
    pub(crate) full_width: f32,
    pub(crate) height: f32,
    pub(crate) fonts: Vec<Font>,
    pub(crate) coords: Vec<i16>,

    // Lazily calculated values
    content_widths: OnceCell<ContentWidths>,

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
}

impl<B: Brush> Default for LayoutData<B> {
    fn default() -> Self {
        Self {
            scale: 1.,
            has_bidi: false,
            base_level: 0,
            text_len: 0,
            width: 0.,
            full_width: 0.,
            content_widths: OnceCell::new(),
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
        }
    }
}

impl<B: Brush> LayoutData<B> {
    pub(crate) fn clear(&mut self) {
        self.scale = 1.;
        self.has_bidi = false;
        self.base_level = 0;
        self.text_len = 0;
        self.width = 0.;
        self.full_width = 0.;
        self.content_widths.take();
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
        synthesis: Synthesis,
        shaper: Shaper<'_>,
        bidi_level: u8,
        word_spacing: f32,
        letter_spacing: f32,
    ) {
        let font_index = self
            .fonts
            .iter()
            .position(|f| *f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font);
                index
            });
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
                info: cluster.info,
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
                    id: g.id,
                    style_index,
                    x: g.x,
                    y: g.y,
                    advance: g.advance,
                }
            }));
            glyph_count += glyph_len;
            push_components!();
        });
        flush_run!();
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

    pub(crate) fn content_widths(&self) -> ContentWidths {
        *self
            .content_widths
            .get_or_init(|| self.calculate_content_widths())
    }

    fn calculate_content_widths(&self) -> ContentWidths {
        fn whitespace_advance(cluster: Option<&ClusterData>) -> f32 {
            cluster
                .filter(|cluster| cluster.info.whitespace().is_space_or_nbsp())
                .map_or(0.0, |cluster| cluster.advance)
        }

        let mut min_width = 0.0_f32;
        let mut max_width = 0.0_f32;

        let mut running_max_width = 0.0;
        let mut prev_cluster: Option<&ClusterData> = None;
        for item in &self.items {
            match item.kind {
                LayoutItemKind::TextRun => {
                    let run = &self.runs[item.index];
                    let mut running_min_width = 0.0;
                    for cluster in &self.clusters[run.cluster_range.clone()] {
                        let boundary = cluster.info.boundary();
                        if matches!(boundary, Boundary::Line | Boundary::Mandatory) {
                            let trailing_whitespace = whitespace_advance(prev_cluster);
                            min_width = min_width.max(running_min_width - trailing_whitespace);
                            running_min_width = 0.0;
                            if boundary == Boundary::Mandatory {
                                running_max_width = 0.0;
                            }
                        }
                        running_min_width += cluster.advance;
                        prev_cluster = Some(cluster);
                    }
                    let trailing_whitespace = whitespace_advance(prev_cluster);
                    min_width = min_width.max(running_min_width - trailing_whitespace);
                    running_max_width += run.advance;
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
}
