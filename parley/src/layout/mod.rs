// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout types.

mod alignment;
mod cluster;
mod line;
mod run;

pub(crate) mod data;

pub mod cursor;

use self::alignment::align;

use super::style::Brush;
use crate::{Font, InlineBox};
use core::{cmp::Ordering, ops::Range};
use data::*;
use swash::text::cluster::{Boundary, ClusterInfo};
use swash::{GlyphId, NormalizedCoord, Synthesis};

pub use cursor::Cursor;
pub use line::greedy::BreakLines;
pub use line::{GlyphRun, LineMetrics, PositionedInlineBox, PositionedLayoutItem};
pub use run::RunMetrics;

/// Alignment of a layout.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Alignment {
    #[default]
    Start,
    Middle,
    End,
    Justified,
}

/// Text layout.
#[derive(Clone)]
pub struct Layout<B: Brush> {
    pub(crate) data: LayoutData<B>,
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
    pub fn get(&self, index: usize) -> Option<Line<B>> {
        Some(Line {
            layout: &self.data,
            data: self.data.lines.get(index)?,
        })
    }

    pub fn inline_boxes(&self) -> &[InlineBox] {
        &self.data.inline_boxes
    }

    pub fn inline_boxes_mut(&mut self) -> &mut [InlineBox] {
        &mut self.data.inline_boxes
    }

    /// Returns an iterator over the lines in the layout.
    pub fn lines(&self) -> impl Iterator<Item = Line<B>> + '_ + Clone {
        self.data.lines.iter().map(move |data| Line {
            layout: &self.data,
            data,
        })
    }

    /// Returns line breaker to compute lines for the layout.
    pub fn break_lines(&mut self) -> BreakLines<B> {
        BreakLines::new(&mut self.data)
    }

    /// Breaks all lines with the specified maximum advance.
    pub fn break_all_lines(&mut self, max_advance: Option<f32>, alignment: Alignment) {
        self.break_lines()
            .break_remaining(max_advance.unwrap_or(f32::MAX), alignment);
    }

    // Apply to alignment to layout relative to the specified container width. If container_width is not
    // specified then the max line length is used.
    pub fn align(&mut self, container_width: Option<f32>, alignment: Alignment) {
        align(&mut self.data, container_width, alignment);
    }

    /// Returns an iterator over the runs in the layout.
    pub fn runs(&self) -> impl Iterator<Item = Run<B>> + '_ + Clone {
        self.data.runs.iter().map(move |data| Run {
            layout: &self.data,
            data,
            line_data: None,
        })
    }

    /// Returns the index and `Line` object for the line containing the
    /// given byte `index` in the source text.
    pub(crate) fn line_for_byte_index(&self, index: usize) -> Option<(usize, Line<B>)> {
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
    /// For horizontal text, this is a vertical or y offset.
    pub(crate) fn line_for_offset(&self, offset: f32) -> Option<(usize, Line<B>)> {
        if offset < 0.0 {
            return Some((0, self.get(0)?));
        }
        let maybe_line_index = self.data.lines.binary_search_by(|line| {
            if offset < line.metrics.min_coord {
                Ordering::Greater
            } else if offset > line.metrics.max_coord {
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

/// Sequence of clusters with a single font and style.
#[derive(Copy, Clone)]
pub struct Run<'a, B: Brush> {
    layout: &'a LayoutData<B>,
    data: &'a RunData,
    line_data: Option<&'a LineItemData>,
}

/// Atomic unit of text.
#[derive(Copy, Clone)]
pub struct Cluster<'a, B: Brush> {
    run: Run<'a, B>,
    data: &'a ClusterData,
}

/// Glyph with an offset and advance.
#[derive(Copy, Clone, Default, Debug)]
pub struct Glyph {
    pub id: GlyphId,
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

/// Line in a text layout.
#[derive(Copy, Clone)]
pub struct Line<'a, B: Brush> {
    layout: &'a LayoutData<B>,
    data: &'a LineData,
}

/// Style properties.
#[derive(Clone, Debug)]
pub struct Style<B: Brush> {
    /// Brush for drawing glyphs.
    pub brush: B,
    /// Underline decoration.
    pub underline: Option<Decoration<B>>,
    /// Strikethrough decoration.
    pub strikethrough: Option<Decoration<B>>,
    /// Absolute line height in layout units (style line height * font size)
    pub(crate) line_height: f32,
}

/// Underline or strikethrough decoration.
#[derive(Clone, Debug)]
pub struct Decoration<B: Brush> {
    /// Brush used to draw the decoration.
    pub brush: B,
    /// Offset of the decoration from the baseline. If `None`, use the metrics
    /// of the containing run.
    pub offset: Option<f32>,
    /// Thickness of the decoration. If `None`, use the metrics of the
    /// containing run.
    pub size: Option<f32>,
}
