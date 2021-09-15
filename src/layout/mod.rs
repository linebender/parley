//! Layout types.

mod cluster;
mod line;
mod run;

pub mod cursor;

use super::data::*;
use super::font::Font;
use super::style::Brush;
use core::ops::Range;
use swash::text::cluster::{Boundary, ClusterInfo};
use swash::{GlyphId, NormalizedCoord, Synthesis};

pub use cursor::Cursor;
pub use line::greedy::BreakLines;
pub use line::{GlyphRun, LineMetrics};
pub use run::RunMetrics;

/// Alignment of a layout.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Alignment {
    Start,
    Middle,
    End,
}

impl Default for Alignment {
    fn default() -> Self {
        Self::Start
    }
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

    /// Returns the style collection for the layout.
    pub fn styles(&self) -> &[Style<B>] {
        &self.data.styles
    }

    /// Returns the number of lines in the layout.
    pub fn len(&self) -> usize {
        self.data.lines.len()
    }

    /// Returns true if the layout is empty.
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

    /// Breaks all lines with the specified maximum advance and alignment.
    pub fn break_all_lines(&mut self, max_advance: Option<f32>, alignment: Alignment) {
        self.break_lines()
            .break_remaining(max_advance.unwrap_or(f32::MAX), alignment)
    }

    /// Returns an iterator over the runs in the layout.
    pub fn runs(&self) -> impl Iterator<Item = Run<B>> + '_ + Clone {
        self.data.runs.iter().map(move |data| Run {
            layout: &self.data,
            data,
            line_data: None,
        })
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
    line_data: Option<&'a LineRunData>,
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
