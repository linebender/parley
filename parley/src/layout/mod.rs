// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout types.

mod alignment;
mod cluster;
mod line;
mod run;

pub(crate) mod data;

pub mod cursor;

pub mod editor;

use self::alignment::align;

use super::style::Brush;
use crate::{Font, InlineBox};
#[cfg(feature = "accesskit")]
use accesskit::{Node, NodeId, Role, TextDirection, TreeUpdate};
#[cfg(feature = "accesskit")]
use alloc::vec::Vec;
use core::{cmp::Ordering, ops::Range};
use data::*;
#[cfg(feature = "accesskit")]
use hashbrown::{HashMap, HashSet};
use swash::text::cluster::{Boundary, ClusterInfo};
use swash::{GlyphId, NormalizedCoord, Synthesis};

pub use cluster::{Affinity, ClusterPath};
pub use cursor::{Cursor, Selection, VisualMode};
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
    pub fn lines(&self) -> impl Iterator<Item = Line<B>> + '_ + Clone {
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
    pub fn break_lines(&mut self) -> BreakLines<B> {
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
    layout: &'a Layout<B>,
    line_index: u32,
    index: u32,
    data: &'a RunData,
    line_data: Option<&'a LineItemData>,
}

/// Atomic unit of text.
#[derive(Copy, Clone)]
pub struct Cluster<'a, B: Brush> {
    path: ClusterPath,
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
    layout: &'a Layout<B>,
    index: u32,
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

#[cfg(feature = "accesskit")]
#[derive(Clone, Default)]
pub struct LayoutAccessibility {
    // The following two fields maintain a two-way mapping between runs
    // and AccessKit node IDs, where each run is identified by its line index
    // and run index within that line, or a run path for short. These maps
    // are maintained by `LayoutAccess::build_nodes`, which ensures that removed
    // runs are removed from the maps on the next accessibility pass.
    pub(crate) access_ids_by_run_path: HashMap<(usize, usize), NodeId>,
    pub(crate) run_paths_by_access_id: HashMap<NodeId, (usize, usize)>,
}

#[cfg(feature = "accesskit")]
impl LayoutAccessibility {
    #[allow(clippy::too_many_arguments)]
    pub fn build_nodes<B: Brush>(
        &mut self,
        text: &str,
        layout: &Layout<B>,
        update: &mut TreeUpdate,
        parent_node: &mut Node,
        mut next_node_id: impl FnMut() -> NodeId,
        x_offset: f64,
        y_offset: f64,
    ) {
        // Build a set of node IDs for the runs encountered in this pass.
        let mut ids = HashSet::<NodeId>::new();

        for (line_index, line) in layout.lines().enumerate() {
            let metrics = line.metrics();
            // Defer adding each run node until we reach either the next run
            // or the end of the line. That way, we can set relations between
            // runs in a line and do anything special that might be required
            // for the last run in a line.
            let mut last_node: Option<(NodeId, Node)> = None;

            // Iterate over the runs from left to right, computing their offsets,
            // then sort them into text order.
            let runs = {
                let mut run_offset = metrics.offset;
                let mut runs = Vec::with_capacity(line.len());
                for run in line.runs() {
                    let advance = run.advance();
                    runs.push((run, run_offset));
                    run_offset += advance;
                }
                runs.sort_by_key(|(r, _)| r.text_range().start);
                runs
            };

            for (run, run_offset) in runs {
                let run_path = (line_index, run.index());
                // If we encountered this same run path in the previous
                // accessibility pass, reuse the same AccessKit ID. Otherwise,
                // allocate a new one. This enables stable node IDs when merely
                // updating the content of existing runs.
                let id = self
                    .access_ids_by_run_path
                    .get(&run_path)
                    .copied()
                    .unwrap_or_else(|| {
                        let id = next_node_id();
                        self.access_ids_by_run_path.insert(run_path, id);
                        self.run_paths_by_access_id.insert(id, run_path);
                        id
                    });
                ids.insert(id);
                let mut node = Node::new(Role::TextRun);

                if let Some((last_id, mut last_node)) = last_node.take() {
                    last_node.set_next_on_line(id);
                    node.set_previous_on_line(last_id);
                    update.nodes.push((last_id, last_node));
                    parent_node.push_child(last_id);
                }

                node.set_bounds(accesskit::Rect {
                    x0: x_offset + run_offset as f64,
                    y0: y_offset + metrics.min_coord as f64,
                    x1: x_offset + (run_offset + run.advance()) as f64,
                    y1: y_offset + metrics.max_coord as f64,
                });
                node.set_text_direction(if run.is_rtl() {
                    TextDirection::RightToLeft
                } else {
                    TextDirection::LeftToRight
                });

                let run_text = &text[run.text_range()];
                node.set_value(run_text);

                let mut character_lengths = Vec::new();
                let mut cluster_offset = 0.0;
                let mut character_positions = Vec::new();
                let mut character_widths = Vec::new();
                let mut word_lengths = Vec::new();
                let mut last_word_start = 0;

                for cluster in run.clusters() {
                    let cluster_text = &text[cluster.text_range()];
                    if cluster.is_word_boundary()
                        && !cluster.is_space_or_nbsp()
                        && !character_lengths.is_empty()
                    {
                        word_lengths.push((character_lengths.len() - last_word_start) as _);
                        last_word_start = character_lengths.len();
                    }
                    character_lengths.push(cluster_text.len() as _);
                    character_positions.push(cluster_offset);
                    character_widths.push(cluster.advance());
                    cluster_offset += cluster.advance();
                }

                word_lengths.push((character_lengths.len() - last_word_start) as _);
                node.set_character_lengths(character_lengths);
                node.set_character_positions(character_positions);
                node.set_character_widths(character_widths);
                node.set_word_lengths(word_lengths);

                last_node = Some((id, node));
            }

            if let Some((id, node)) = last_node {
                update.nodes.push((id, node));
                parent_node.push_child(id);
            }
        }

        // Remove mappings for runs that no longer exist.
        let mut ids_to_remove = Vec::<NodeId>::new();
        let mut run_paths_to_remove = Vec::<(usize, usize)>::new();
        for (access_id, run_path) in self.run_paths_by_access_id.iter() {
            if !ids.contains(access_id) {
                ids_to_remove.push(*access_id);
                run_paths_to_remove.push(*run_path);
            }
        }
        for id in ids_to_remove {
            self.run_paths_by_access_id.remove(&id);
        }
        for run_path in run_paths_to_remove {
            self.access_ids_by_run_path.remove(&run_path);
        }
    }
}
