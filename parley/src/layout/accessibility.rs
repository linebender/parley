// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use accesskit::{Node, NodeId, Role, TextDirection, TreeUpdate};
use hashbrown::{HashMap, HashSet};

use crate::Layout;
use crate::style::Brush;

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
        // Reuse scratch space for storing a sorted list of runs.
        let mut runs = Vec::new();

        for (line_index, line) in layout.lines().enumerate() {
            let metrics = line.metrics();
            // Defer adding each run node until we reach either the next run
            // or the end of the line. That way, we can set relations between
            // runs in a line and do anything special that might be required
            // for the last run in a line.
            let mut last_node: Option<(NodeId, Node)> = None;

            // Iterate over the runs from left to right, computing their offsets,
            // then sort them into text order.
            runs.clear();
            runs.reserve(line.len());
            {
                let mut run_offset = metrics.offset;
                for run in line.runs() {
                    let advance = run.advance();
                    runs.push((run, run_offset));
                    run_offset += advance;
                }
            }
            runs.sort_by_key(|(r, _)| r.text_range().start);

            for (run, run_offset) in runs.drain(..) {
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
