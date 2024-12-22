// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ops::Range;

use crate::inputs::LineMetrics;

use crate::outputs::{Alignment, LayoutData};
use crate::outputs::{BreakReason, Brush, Glyph, Layout, Run, Style};

/// Line in a text layout.
#[derive(Copy, Clone)]
pub struct Line<'a, B: Brush> {
    pub(crate) layout: &'a Layout<B>,
    pub(crate) index: u32,
    pub(crate) data: &'a LineData,
}

impl<'a, B: Brush> Line<'a, B> {
    /// Returns the metrics for the line.
    pub fn metrics(&self) -> &LineMetrics {
        &self.data.metrics
    }

    pub fn break_reason(&self) -> BreakReason {
        self.data.break_reason
    }

    /// Returns the range of text for the line.
    pub fn text_range(&self) -> Range<usize> {
        self.data.text_range.clone()
    }

    /// Returns the number of items in the line.
    pub fn len(&self) -> usize {
        self.data.item_range.len()
    }

    /// Returns `true` if the line is empty.
    pub fn is_empty(&self) -> bool {
        self.data.item_range.is_empty()
    }

    /// Returns the run at the specified index.
    pub(crate) fn item(&self, index: usize) -> Option<&LineItemData> {
        let index = self.data.item_range.start + index;
        if index >= self.data.item_range.end {
            return None;
        }
        let item = self.layout.data.line_items.get(index)?;
        Some(item)
    }

    /// Returns the run at the specified index.
    pub fn run(&self, index: usize) -> Option<Run<'a, B>> {
        let original_index = index;
        let index = self.data.item_range.start + index;
        if index >= self.data.item_range.end {
            return None;
        }
        let item = self.layout.data.line_items.get(index)?;

        if item.kind == LayoutItemKind::TextRun {
            Some(Run {
                layout: self.layout,
                line_index: self.index,
                index: original_index as u32,
                data: self.layout.data.runs.get(item.index)?,
                line_data: Some(item),
            })
        } else {
            None
        }
    }

    /// Returns an iterator over the runs for the line.
    // TODO: provide iterator over inline_boxes and items
    pub fn runs(&self) -> impl Iterator<Item = Run<'a, B>> + 'a + Clone {
        let copy = self.clone();
        let line_items = &copy.layout.data.line_items[self.data.item_range.clone()];
        line_items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.kind == LayoutItemKind::TextRun)
            .map(move |(index, line_data)| Run {
                layout: copy.layout,
                line_index: copy.index,
                index: index as u32,
                data: &copy.layout.data.runs[line_data.index],
                line_data: Some(line_data),
            })
    }

    /// Returns an iterator over the glyph runs for the line.
    pub fn items(&self) -> impl Iterator<Item = PositionedLayoutItem<'a, B>> + 'a + Clone {
        GlyphRunIter {
            line: self.clone(),
            item_index: 0,
            glyph_start: 0,
            offset: 0.,
        }
    }
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

/// The computed result of an item (glyph run or inline box) within a layout
#[derive(Clone)]
pub enum PositionedLayoutItem<'a, B: Brush> {
    GlyphRun(GlyphRun<'a, B>),
    InlineBox(PositionedInlineBox),
}

/// The computed position of an inline box within a layout
#[derive(Debug, Clone)]
pub struct PositionedInlineBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub id: u64,
}

/// Sequence of fully positioned glyphs with the same style.
#[derive(Clone)]
pub struct GlyphRun<'a, B: Brush> {
    run: Run<'a, B>,
    style: &'a Style<B>,
    glyph_start: usize,
    glyph_count: usize,
    offset: f32,
    baseline: f32,
    advance: f32,
}

impl<'a, B: Brush> GlyphRun<'a, B> {
    /// Returns the underlying run.
    pub fn run(&self) -> &Run<'a, B> {
        &self.run
    }

    /// Returns the associated style.
    pub fn style(&self) -> &Style<B> {
        self.style
    }

    /// Returns the offset to the baseline.
    pub fn baseline(&self) -> f32 {
        self.baseline
    }

    /// Returns the offset to the first glyph along the baseline.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Returns the total advance of the run.
    pub fn advance(&self) -> f32 {
        self.advance
    }

    /// Returns an iterator over the glyphs in the run.
    pub fn glyphs(&'a self) -> impl Iterator<Item = Glyph> + 'a + Clone {
        self.run
            .visual_clusters()
            .flat_map(|cluster| cluster.glyphs())
            .skip(self.glyph_start)
            .take(self.glyph_count)
    }

    /// Returns an iterator over the fully positioned glyphs in the run.
    pub fn positioned_glyphs(&'a self) -> impl Iterator<Item = Glyph> + 'a + Clone {
        let mut offset = self.offset;
        let baseline = self.baseline;
        self.glyphs().map(move |mut g| {
            g.x += offset;
            g.y += baseline;
            offset += g.advance;
            g
        })
    }
}

#[derive(Clone)]
struct GlyphRunIter<'a, B: Brush> {
    line: Line<'a, B>,
    item_index: usize,
    glyph_start: usize,
    offset: f32,
}

impl<'a, B: Brush> Iterator for GlyphRunIter<'a, B> {
    type Item = PositionedLayoutItem<'a, B>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.line.item(self.item_index)?;
            match item.kind {
                LayoutItemKind::InlineBox => {
                    let inline_box = &self.line.layout.data.inline_boxes[item.index];

                    let x = self.offset + self.line.data.metrics.offset;

                    self.item_index += 1;
                    self.glyph_start = 0;
                    self.offset += item.advance;
                    return Some(PositionedLayoutItem::InlineBox(PositionedInlineBox {
                        x,
                        y: self.line.data.metrics.baseline - inline_box.height,
                        width: inline_box.width,
                        height: inline_box.height,
                        id: inline_box.id,
                    }));
                }

                LayoutItemKind::TextRun => {
                    let run = self.line.run(self.item_index)?;
                    let mut iter = run
                        .visual_clusters()
                        .flat_map(|c| c.glyphs())
                        .skip(self.glyph_start);

                    if let Some(first) = iter.next() {
                        let mut advance = first.advance;
                        let style_index = first.style_index();
                        let mut glyph_count = 1;
                        for glyph in iter.take_while(|g| g.style_index() == style_index) {
                            glyph_count += 1;
                            advance += glyph.advance;
                        }
                        let style = run.layout.data.styles.get(style_index)?;
                        let glyph_start = self.glyph_start;
                        self.glyph_start += glyph_count;
                        let offset = self.offset;
                        self.offset += advance;
                        return Some(PositionedLayoutItem::GlyphRun(GlyphRun {
                            run,
                            style,
                            glyph_start,
                            glyph_count,
                            offset: offset + self.line.data.metrics.offset,
                            baseline: self.line.data.metrics.baseline,
                            advance,
                        }));
                    }
                    self.item_index += 1;
                    self.glyph_start = 0;
                }
            }
        }
    }
}
