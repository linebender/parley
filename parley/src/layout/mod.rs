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
use accesskit::{NodeBuilder, NodeId, Role, TextPosition, TreeUpdate};
#[cfg(feature = "accesskit")]
use alloc::collections::{BTreeMap, BTreeSet};
use core::{cmp::Ordering, ops::Range};
use data::*;
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
    // are maintained by `TextLayout::accessibility`, which ensures that removed
    // runs are removed from the maps on the next accessibility pass.
    access_ids_by_run_path: BTreeMap<(usize, usize), NodeId>,
    run_paths_by_access_id: BTreeMap<NodeId, (usize, usize)>,

    // This map duplicates the character lengths stored in the run nodes.
    // This is necessary because this information is needed during the
    // access event pass, after the previous tree update has already been
    // pushed to AccessKit. AccessKit deliberately doesn't let toolkits access
    // the current tree state, because the ideal AccessKit backend would push
    // tree updates to assistive technologies and not retain a tree in memory.
    character_lengths_by_access_id: BTreeMap<NodeId, Box<[u8]>>,
}

#[cfg(feature = "accesskit")]
impl LayoutAccessibility {
    pub fn build_nodes<B: Brush>(
        &mut self,
        text: &str,
        layout: &Layout<B>,
        update: &mut TreeUpdate,
        parent_node: &mut NodeBuilder,
        mut next_node_id: impl FnMut() -> NodeId,
    ) {
        // Build a set of node IDs for the runs encountered in this pass.
        let mut ids = BTreeSet::<NodeId>::new();

        for (line_index, line) in layout.lines().enumerate() {
            // Defer adding each run node until we reach either the next run
            // or the end of the line. That way, we can set relations between
            // runs in a line and do anything special that might be required
            // for the last run in a line.
            let mut last_node: Option<(NodeId, NodeBuilder)> = None;

            for (run_index, run) in line.runs().enumerate() {
                let run_path = (line_index, run_index);
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
                let mut node = NodeBuilder::new(Role::InlineTextBox);

                if let Some((last_id, mut last_node)) = last_node.take() {
                    last_node.set_next_on_line(id);
                    node.set_previous_on_line(last_id);
                    update.nodes.push((last_id, last_node.build()));
                    parent_node.push_child(last_id);
                }

                // TODO: bounding rectangle and character position/width
                let run_text = &text[run.text_range()];
                node.set_value(run_text);

                let mut character_lengths = Vec::new();
                let mut word_lengths = Vec::new();
                let mut last_word_start = 0;

                for cluster in run.clusters() {
                    let cluster_text = &text[cluster.text_range()];
                    if cluster.is_word_boundary() && !character_lengths.is_empty() {
                        word_lengths.push((character_lengths.len() - last_word_start) as _);
                        last_word_start = character_lengths.len();
                    }
                    character_lengths.push(cluster_text.len() as _);
                }

                word_lengths.push((character_lengths.len() - last_word_start) as _);
                self.character_lengths_by_access_id
                    .insert(id, character_lengths.clone().into());
                node.set_character_lengths(character_lengths);
                node.set_word_lengths(word_lengths);

                last_node = Some((id, node));
            }

            if let Some((id, node)) = last_node {
                update.nodes.push((id, node.build()));
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
            self.character_lengths_by_access_id.remove(&id);
        }
        for run_path in run_paths_to_remove {
            self.access_ids_by_run_path.remove(&run_path);
        }
    }

    pub fn access_position_from_offset<B: Brush>(
        &self,
        text: &str,
        layout: &Layout<B>,
        offset: usize,
        affinity: Affinity,
    ) -> Option<TextPosition> {
        debug_assert!(offset <= text.len(), "offset out of range");

        for (line_index, line) in layout.lines().enumerate() {
            let range = line.text_range();
            if !(range.contains(&offset)
                || (offset == range.end
                    && (affinity == Affinity::Upstream || line_index == layout.len() - 1)))
            {
                continue;
            }

            for (run_index, run) in line.runs().enumerate() {
                let range = run.text_range();
                if !(range.contains(&offset)
                    || (offset == range.end
                        && (affinity == Affinity::Upstream
                            || (line_index == layout.len() - 1 && run_index == line.len() - 1))))
                {
                    continue;
                }

                let run_offset = offset - range.start;
                let run_path = (line_index, run_index);
                let id = *self.access_ids_by_run_path.get(&run_path).unwrap();
                let character_lengths = self.character_lengths_by_access_id.get(&id).unwrap();
                let mut length_sum = 0_usize;
                for (character_index, length) in character_lengths.iter().copied().enumerate() {
                    if run_offset == length_sum {
                        return Some(TextPosition {
                            node: id,
                            character_index,
                        });
                    }
                    length_sum += length as usize;
                }
                return Some(TextPosition {
                    node: id,
                    character_index: character_lengths.len(),
                });
            }
        }

        if cfg!(debug_assertions) {
            panic!(
                "offset {} not within the range of any run; text length: {}",
                offset,
                text.len()
            );
        }
        None
    }

    pub fn offset_from_access_position<B: Brush>(
        &self,
        layout: &Layout<B>,
        pos: TextPosition,
    ) -> Option<(usize, Affinity)> {
        let character_lengths = self.character_lengths_by_access_id.get(&pos.node)?;
        if pos.character_index > character_lengths.len() {
            return None;
        }
        let run_path = *self.run_paths_by_access_id.get(&pos.node)?;
        let (line_index, run_index) = run_path;
        let line = layout.get(line_index)?;
        let run = line.run(run_index)?;
        let offset = run.text_range().start
            + character_lengths[..pos.character_index]
                .iter()
                .copied()
                .map(usize::from)
                .sum::<usize>();
        let affinity = if pos.character_index == character_lengths.len() && line_index < run.len() {
            Affinity::Upstream
        } else {
            Affinity::Downstream
        };
        Some((offset, affinity))
    }
}
