// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::cmp::Ordering;
use swash::shape::Shaper;
use swash::text::cluster::Boundary;
use swash::Synthesis;

use crate::inputs::break_lines::BreakLines;
use crate::inputs::style::Brush;
use crate::{Font, InlineBox};

use crate::outputs::align;
use crate::outputs::run::RunMetrics;
use crate::outputs::{
    Alignment, ClusterData, Glyph, LayoutItem, LayoutItemKind, Line, LineData, LineItemData,
    RunData, Style,
};
use crate::util::nearly_zero;

/// Text layout.
#[derive(Clone)]
pub struct Layout<B: Brush> {
    pub(crate) data: LayoutData<B>,
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
}

impl<B: Brush> Layout<B> {
    /// Creates an empty layout.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the scale factor provided when creating the layout.
    pub fn scale(&self) -> f32 {
        self.data.scale
    }

    /// Returns the style collection for the layout.
    pub fn styles(&self) -> &[Style<B>] {
        &self.data.styles
    }

    /// Returns the width of the layout.
    pub fn width(&self) -> f32 {
        self.data.width
    }

    /// Returns the width of the layout, including the width of any trailing
    /// whitespace.
    pub fn full_width(&self) -> f32 {
        self.data.full_width
    }

    /// Returns the height of the layout.
    pub fn height(&self) -> f32 {
        self.data.height
    }

    /// Returns the number of lines in the layout.
    pub fn len(&self) -> usize {
        self.data.lines.len()
    }

    /// Returns `true` if the layout is empty.
    pub fn is_empty(&self) -> bool {
        self.data.lines.is_empty()
    }

    /// Returns the line at the specified index.
    pub fn get(&self, index: usize) -> Option<Line<'_, B>> {
        Some(Line {
            index: index as u32,
            layout: self,
            data: self.data.lines.get(index)?,
        })
    }

    /// Returns true if the dominant direction of the layout is right-to-left.
    pub fn is_rtl(&self) -> bool {
        self.data.base_level & 1 != 0
    }

    pub fn inline_boxes(&self) -> &[InlineBox] {
        &self.data.inline_boxes
    }

    pub fn inline_boxes_mut(&mut self) -> &mut [InlineBox] {
        &mut self.data.inline_boxes
    }

    /// Returns an iterator over the lines in the layout.
    pub fn lines(&self) -> impl Iterator<Item = Line<'_, B>> + '_ + Clone {
        self.data
            .lines
            .iter()
            .enumerate()
            .map(move |(index, data)| Line {
                index: index as u32,
                layout: self,
                data,
            })
    }

    /// Returns line breaker to compute lines for the layout.
    pub fn break_lines(&mut self) -> BreakLines<'_, B> {
        BreakLines::new(self)
    }

    /// Breaks all lines with the specified maximum advance.
    pub fn break_all_lines(&mut self, max_advance: Option<f32>) {
        self.break_lines()
            .break_remaining(max_advance.unwrap_or(f32::MAX));
    }

    // Apply to alignment to layout relative to the specified container width. If container_width is not
    // specified then the max line length is used.
    pub fn align(&mut self, container_width: Option<f32>, alignment: Alignment) {
        align(&mut self.data, container_width, alignment);
    }

    /// Returns the index and `Line` object for the line containing the
    /// given byte `index` in the source text.
    pub(crate) fn line_for_byte_index(&self, index: usize) -> Option<(usize, Line<'_, B>)> {
        let line_index = self
            .data
            .lines
            .binary_search_by(|line| {
                if index < line.text_range.start {
                    Ordering::Greater
                } else if index >= line.text_range.end {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            })
            .ok()?;
        Some((line_index, self.get(line_index)?))
    }

    /// Returns the index and `Line` object for the line containing the
    /// given `offset`.
    ///
    /// The offset is specified in the direction orthogonal to line direction.
    /// For horizontal text, this is a vertical or y offset. If the offset is
    /// on a line boundary, it is considered to be contained by the later line.
    pub(crate) fn line_for_offset(&self, offset: f32) -> Option<(usize, Line<'_, B>)> {
        if offset < 0.0 {
            return Some((0, self.get(0)?));
        }
        let maybe_line_index = self.data.lines.binary_search_by(|line| {
            if offset < line.metrics.min_coord {
                Ordering::Greater
            } else if offset >= line.metrics.max_coord {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        });
        let line_index = match maybe_line_index {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        Some((line_index, self.get(line_index)?))
    }
}

impl<B: Brush> Default for Layout<B> {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}
