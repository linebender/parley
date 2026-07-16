// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::layout::Style;
use crate::layout::data::BreakReason;
use crate::layout::data::{LayoutItemKind, LineData};
use crate::layout::layout::Layout;
use crate::layout::run::Run;
use crate::style::Brush;
use crate::{InlineBox, InlineBoxKind};

use core::ops::Range;
use parley_core::Glyph;

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

    /// Returns the line item at the specified index.
    pub(crate) fn item(&self, index: usize) -> Option<LineItem<'a, B>> {
        let original_index = index;
        let index = self.data.item_range.start + index;
        if index >= self.data.item_range.end {
            return None;
        }
        let item = self.layout.data.line_items.get(index)?;

        Some(match item.kind {
            LayoutItemKind::TextRun => LineItem::Run(Run {
                layout: self.layout,
                line_index: self.index,
                index: original_index as u32,
                data: self.layout.data.runs.get(item.index)?,
                line_data: Some(item),
            }),
            LayoutItemKind::InlineBox => {
                LineItem::InlineBox(self.layout.data.inline_boxes.get(item.index)?)
            }
        })
    }

    /// Returns an iterator over the runs for the line.
    pub fn runs(&self) -> impl Iterator<Item = Run<'a, B>> + 'a + Clone + use<'a, B> {
        self.items_nonpositioned().filter_map(|item| item.run())
    }

    /// Returns an iterator over the non-glyph runs and inline boxes for the line.
    pub(crate) fn items_nonpositioned(
        &self,
    ) -> impl Iterator<Item = LineItem<'a, B>> + Clone + use<'a, B> {
        let copy = self.clone();
        let line_items = &copy.layout.data.line_items[self.data.item_range.clone()];
        line_items
            .iter()
            .enumerate()
            .map(move |(item_index, line_data)| match line_data.kind {
                LayoutItemKind::TextRun => LineItem::Run(Run {
                    layout: copy.layout,
                    line_index: copy.index,
                    index: item_index as u32,
                    data: &copy.layout.data.runs[line_data.index],
                    line_data: Some(line_data),
                }),
                LayoutItemKind::InlineBox => {
                    LineItem::InlineBox(&copy.layout.data.inline_boxes[line_data.index])
                }
            })
    }

    /// Returns an iterator over the glyph runs for the line.
    pub fn items(
        &self,
    ) -> impl Iterator<Item = PositionedLayoutItem<'a, B>> + 'a + Clone + use<'a, B> {
        GlyphRunIter {
            line: self.clone(),
            item_index: 0,
            glyph_start: 0,
            offset: 0.,
        }
    }
}

/// Metrics information for a line.
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct LineMetrics {
    /// The absolute line height (in layout units).
    pub line_height: f32,
    /// Offset to the baseline.
    pub baseline: f32,
    /// Offset for alignment.
    pub offset: f32,
    /// Full advance of the line, including trailing whitespace.
    pub advance: f32,
    /// Advance of trailing whitespace.
    pub trailing_whitespace: f32,
    /// Minimum coordinate in the line direction.
    ///
    /// For horizontal text, this would be the left of the line.
    pub inline_min_coord: f32,
    /// Maximum coordinate in the line direction.
    ///
    /// For horizontal text, this would be the right of the line.
    pub inline_max_coord: f32,
    /// Minimum coordinate in the direction orthogonal to line
    /// direction.
    ///
    /// In CSS parlance, this is the start of the "line box." See, e.g., CSS 2.1 § 9.4.2
    /// <https://www.w3.org/TR/CSS2/visuren.html#line-box>. The line box is tall enough to cover all
    /// its inline boxes, but note glyphs may overflow their box height, especially when the
    /// typographic line height is small. See also [`Self::content_block_min_coord`].
    ///
    /// For horizontal text, this would be the top of the line.
    pub block_min_coord: f32,
    /// Maximum coordinate in the direction orthogonal to line
    /// direction.
    ///
    /// For horizontal text, this would be the bottom of the line.
    pub block_max_coord: f32,
    /// Minimum coordinate of the line's content in the direction orthogonal to line direction.
    ///
    /// This covers, roughly, the line's glyphs and inline boxes. This does not take into account
    /// typographic leading, but only the typographic ascent and descent. In case of negative
    /// leading, the content block can be larger than the line's block. Glyphs can overflow the
    /// content area: tall diacritics can exceed the area, stacking combining marks can easily
    /// exceed the area, and some fonts will simply have glyphs that exceed this.
    ///
    /// For horizontal text, this would be the top of the line's content.
    pub content_block_min_coord: f32,
    /// Maximum coordinate of the line's content in the direction orthogonal to line direction.
    ///
    /// See [`LineMetrics::content_block_min_coord`] for a caveat about glyphs overflowing the
    /// content area.
    ///
    /// For horizontal text, this would be the bottom of the line's content.
    pub content_block_max_coord: f32,
}

impl LineMetrics {
    /// Returns the size of the line
    pub fn size(&self) -> f32 {
        self.line_height
    }
}

/// A line item and its corresponding data (a run or inline box). Unlike a
/// [`PositionedLayoutItem`], runs are not guaranteed to be split by style.
pub(crate) enum LineItem<'a, B: Brush> {
    Run(Run<'a, B>),
    InlineBox(&'a InlineBox),
}

impl<'a, B: Brush> LineItem<'a, B> {
    pub(crate) fn run(self) -> Option<Run<'a, B>> {
        match self {
            LineItem::Run(run) => Some(run),
            _ => None,
        }
    }
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
    /// The baseline of the box in pixels, relative to the top of the box (i.e. to [`Self::y`]).
    ///
    /// This is `None` if the box does not specify a baseline, in which case its bottom edge is
    /// aligned with the text baseline.
    pub baseline: Option<f32>,
    pub id: u64,
    pub kind: InlineBoxKind,
}

/// Sequence of fully positioned glyphs with the same style.
#[derive(Clone)]
pub struct GlyphRun<'a, B: Brush> {
    run: Run<'a, B>,
    style_index: u16,
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
    ///
    /// See also [`Self::style_index`].
    pub fn style(&self) -> &Style<B> {
        &self.run.layout.styles()[usize::from(self.style_index)]
    }

    /// Returns the associated style index.
    ///
    /// See also [`Self::style`].
    pub fn style_index(&self) -> u16 {
        self.style_index
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
            match item {
                LineItem::InlineBox(inline_box) => {
                    let x = self.offset
                        + self.line.data.metrics.inline_min_coord
                        + self.line.data.metrics.offset;

                    self.item_index += 1;
                    self.glyph_start = 0;
                    if inline_box.kind == InlineBoxKind::InFlow {
                        self.offset += inline_box.width;
                    }
                    return Some(PositionedLayoutItem::InlineBox(PositionedInlineBox {
                        x,
                        y: self.line.data.metrics.baseline
                            - inline_box.baseline.unwrap_or(inline_box.height),
                        width: inline_box.width,
                        height: inline_box.height,
                        baseline: inline_box.baseline,
                        id: inline_box.id,
                        kind: inline_box.kind,
                    }));
                }
                LineItem::Run(run) => {
                    let mut glyphs = run
                        .visual_clusters()
                        .flat_map(|c| c.glyphs().map(|glyph| (glyph, c.data.style_index)))
                        .skip(self.glyph_start);

                    if let Some((first_glyph, first_style_index)) = glyphs.next() {
                        let mut advance = first_glyph.advance;
                        let mut glyph_count = 1;
                        for (glyph, _) in
                            glyphs.take_while(|(_, style_index)| *style_index == first_style_index)
                        {
                            glyph_count += 1;
                            advance += glyph.advance;
                        }
                        let glyph_start = self.glyph_start;
                        self.glyph_start += glyph_count;
                        let offset = self.offset;
                        self.offset += advance;
                        return Some(PositionedLayoutItem::GlyphRun(GlyphRun {
                            run,
                            style_index: first_style_index,
                            glyph_start,
                            glyph_count,
                            offset: offset
                                + self.line.data.metrics.inline_min_coord
                                + self.line.data.metrics.offset,
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
