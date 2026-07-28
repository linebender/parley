// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! An [`accesskit`] integration for Parley.
//!
//! It shows one reasonable way to expose a text layout, or a [`PlainEditor`],
//! to assistive technologies, and it is meant to be copied and adapted.
//!
//! Start with [`LayoutAccessibility`] for read-only text, and with
//! [`PlainEditorAccessibility`] for editable text.
//!
//! [`PlainEditor`]: parley::editing::PlainEditor

use std::collections::{HashMap, HashSet};

use accesskit::{
    Action, Node, NodeId, Rect, Role, TextAlign, TextDirection, TextPosition, TextSelection,
    TreeUpdate,
};
use parley::editing::{Cursor, PlainEditorDriver, Selection};
use parley::layout::{Affinity, Alignment, Cluster, ClusterPath, Layout, LineMetrics, Run, Style};
use parley::style::{Brush, FontStyle};
use skrifa::{
    FontRef,
    raw::{TableProvider, types::NameId},
};

/// The maximum number of characters in a single `TextRun` node.
///
/// AccessKit's `word_starts` are `u8`s, so a span can't describe more characters than that.
const MAX_CHARACTERS_PER_SPAN: usize = u8::MAX as usize + 1;

/// Maps a Parley [`Layout`] onto a tree of AccessKit [`Role::TextRun`] nodes.
///
/// The same instance should be reused across accessibility passes for a given layout, so
/// that unchanged parts of the text keep their node IDs. Call [`Self::build_nodes`] on
/// every pass, then use the cursor and selection conversions to translate positions
/// between Parley and AccessKit.
#[derive(Clone, Default, Debug)]
pub struct LayoutAccessibility {
    // We define a span as a sequence of clusters, in logical order, that all
    // have an identical style. For each span we create an AccessKit node
    // with the `TextRun` role, and these nodes are in logical order.
    // The following two fields maintain a two-way mapping between spans
    // and AccessKit node IDs, where each span is identified by the path to
    // its first cluster, or a span path for short. These maps are maintained by
    // `LayoutAccessibility::build_nodes`, which ensures that removed spans are removed
    // from the maps on the next accessibility pass.
    access_ids_by_span_path: HashMap<ClusterPath, NodeId>,
    span_paths_by_access_id: HashMap<NodeId, ClusterPath>,
    // Map from cluster path to span path. This allows `cursor_to_access_position`
    // to complete in O(1), rather than worst-case O(n) where n is the length
    // of the run. It also means that the logic for when to start a new span,
    // including the limitation on the number of characters per span,
    // only needs to live in `build_nodes`.
    span_paths_by_cluster_path: HashMap<ClusterPath, ClusterPath>,
}

impl LayoutAccessibility {
    /// Push a `TextRun` node for every style span in `layout` onto `update`, as children
    /// of `parent_node`.
    ///
    /// `text` must be the source text that `layout` was built from. `x_offset` and
    /// `y_offset` are added to the layout-relative coordinates of every node, and should
    /// place the layout within the coordinate space of the accessibility tree.
    ///
    /// `next_node_id` is called to allocate IDs for spans that weren't present on the
    /// previous pass. `set_brush_properties` is called once per span, and should map the
    /// span's brush onto AccessKit's color and text decoration properties; Parley's brush
    /// type is opaque, so only the caller can do this.
    #[allow(
        clippy::too_many_arguments,
        reason = "the layout, the tree being built, and the caller's hooks are all needed"
    )]
    pub fn build_nodes<B: Brush>(
        &mut self,
        text: &str,
        layout: &Layout<B>,
        update: &mut TreeUpdate,
        parent_node: &mut Node,
        mut next_node_id: impl FnMut() -> NodeId,
        x_offset: f64,
        y_offset: f64,
        set_brush_properties: impl Fn(&mut Node, &Style<B>),
    ) {
        self.span_paths_by_cluster_path.clear();
        // Build a set of node IDs for the runs encountered in this pass.
        let mut ids = HashSet::<NodeId>::new();
        // Reuse scratch space for storing a sorted list of runs.
        let mut runs = Vec::new();
        let alignment = layout.alignment();

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
                let mut span_path = ClusterPath::new(line_index as u32, run.index() as u32, 0);
                let (mut id, mut node) =
                    self.span_id_and_node(&mut next_node_id, &mut ids, &run, alignment, span_path);

                if let Some((last_id, mut last_node)) = last_node.take() {
                    link_spans(last_id, &mut last_node, id, &mut node);
                    add_span(update, parent_node, last_id, last_node);
                }

                let mut prev_style_index: Option<u16> = None;
                let mut span_text = String::new();
                let mut character_lengths = Vec::new();
                let mut span_offset = 0.0;
                let mut span_advance = 0.0;
                let mut character_positions = Vec::new();
                let mut character_widths = Vec::new();
                let mut word_starts = Vec::new();

                for cluster in run.clusters() {
                    let style_index = cluster.style_index();
                    if let Some(prev_index) = prev_style_index
                        && (prev_index != style_index
                            || character_lengths.len() >= MAX_CHARACTERS_PER_SPAN)
                    {
                        prev_style_index = None;
                        finish_span(
                            &mut node,
                            x_offset,
                            y_offset,
                            metrics,
                            run_offset,
                            span_offset,
                            span_advance,
                            span_text.clone(),
                            character_lengths.clone(),
                            character_positions.clone(),
                            character_widths.clone(),
                            word_starts.clone(),
                        );
                        span_offset += span_advance;
                        span_advance = 0.0;
                        span_text.clear();
                        character_lengths.clear();
                        character_positions.clear();
                        character_widths.clear();
                        word_starts.clear();
                        (id, node) = {
                            let (old_id, mut old_node) = (id, node);
                            span_path = cluster.path();
                            let (new_id, mut new_node) = self.span_id_and_node(
                                &mut next_node_id,
                                &mut ids,
                                &run,
                                alignment,
                                span_path,
                            );
                            link_spans(old_id, &mut old_node, new_id, &mut new_node);
                            add_span(update, parent_node, old_id, old_node);
                            (new_id, new_node)
                        };
                    }

                    if prev_style_index.is_none() {
                        prev_style_index = Some(style_index);
                        let style = cluster.style();
                        set_brush_properties(&mut node, style);
                        if let Some(locale) = &style.locale {
                            node.set_language(locale.as_str());
                        }
                    }

                    let cluster_text = &text[cluster.text_range()];
                    span_text.push_str(cluster_text);
                    if cluster.is_word_boundary() && !cluster.is_space_or_nbsp() {
                        word_starts.push(character_lengths.len() as _);
                    }
                    character_lengths.push(cluster_text.len() as _);
                    character_positions.push(span_advance);
                    character_widths.push(cluster.advance());
                    span_advance += cluster.advance();
                    self.span_paths_by_cluster_path
                        .insert(cluster.path(), span_path);
                }

                finish_span(
                    &mut node,
                    x_offset,
                    y_offset,
                    metrics,
                    run_offset,
                    span_offset,
                    span_advance,
                    span_text,
                    character_lengths,
                    character_positions,
                    character_widths,
                    word_starts,
                );
                last_node = Some((id, node));
            }

            if let Some((id, node)) = last_node {
                add_span(update, parent_node, id, node);
            }
        }

        // Remove mappings for spans that no longer exist.
        self.span_paths_by_access_id.retain(|access_id, span_path| {
            let keep = ids.contains(access_id);
            if !keep {
                self.access_ids_by_span_path.remove(span_path);
            }
            keep
        });
    }

    /// Convert an AccessKit position within the nodes built by [`Self::build_nodes`] into
    /// a Parley [`Cursor`].
    ///
    /// Returns `None` if the position doesn't refer to one of those nodes.
    pub fn cursor_from_access_position<B: Brush>(
        &self,
        pos: &TextPosition,
        layout: &Layout<B>,
    ) -> Option<Cursor> {
        let span_path = self.span_paths_by_access_id.get(&pos.node)?;
        let run = span_path.run(layout)?;
        let index = run
            .get(span_path.logical_index() + pos.character_index)
            .map(|cluster| cluster.text_range().start)
            .unwrap_or(layout.text_len());
        Some(Cursor::from_byte_index(layout, index, Affinity::Downstream))
    }

    /// Convert a Parley [`Cursor`] into a position within the nodes built by
    /// [`Self::build_nodes`].
    ///
    /// Returns `None` if [`Self::build_nodes`] hasn't been called for this layout yet.
    pub fn cursor_to_access_position<B: Brush>(
        &self,
        cursor: Cursor,
        layout: &Layout<B>,
    ) -> Option<TextPosition> {
        if layout.text_len() == 0 {
            // If the text is empty, just return the first node with a
            // character index of 0.
            return Some(TextPosition {
                node: *self
                    .access_ids_by_span_path
                    .get(&ClusterPath::new(0, 0, 0))?,
                character_index: 0,
            });
        }
        // Prefer the downstream cluster except at the end of the text
        // where we'll choose the upstream cluster and add 1 to the
        // character index.
        let (offset, path) = cursor
            .downstream_cluster(layout)
            .map(|cluster| (0, cluster.path()))
            .or_else(|| {
                cursor
                    .upstream_cluster(layout)
                    .map(|cluster| (1, cluster.path()))
            })?;
        // If we're at the end of the layout and the layout ends with a newline
        // then make sure we use the "phantom" run at the end so that
        // AccessKit has correct visual geometry for the cursor.
        let (span_path, character_index) =
            if cursor.index() == layout.text_len() && ends_with_hard_line_break(layout) {
                (ClusterPath::new(path.line_index() as u32 + 1, 0, 0), 0)
            } else {
                let span_path = self.span_paths_by_cluster_path.get(&path)?;
                (
                    *span_path,
                    path.logical_index() - span_path.logical_index() + offset,
                )
            };
        let id = self.access_ids_by_span_path.get(&span_path)?;
        Some(TextPosition {
            node: *id,
            character_index,
        })
    }

    /// Convert an AccessKit selection over the nodes built by [`Self::build_nodes`] into
    /// a Parley [`Selection`].
    ///
    /// Returns `None` if either endpoint doesn't refer to one of those nodes.
    pub fn selection_from_access_selection<B: Brush>(
        &self,
        selection: &TextSelection,
        layout: &Layout<B>,
    ) -> Option<Selection> {
        let anchor = self.cursor_from_access_position(&selection.anchor, layout)?;
        let focus = self.cursor_from_access_position(&selection.focus, layout)?;
        Some(Selection::new(anchor, focus))
    }

    /// Convert a Parley [`Selection`] into a selection over the nodes built by
    /// [`Self::build_nodes`].
    ///
    /// Returns `None` if [`Self::build_nodes`] hasn't been called for this layout yet.
    pub fn selection_to_access_selection<B: Brush>(
        &self,
        selection: &Selection,
        layout: &Layout<B>,
    ) -> Option<TextSelection> {
        let anchor = self.cursor_to_access_position(selection.anchor(), layout)?;
        let focus = self.cursor_to_access_position(selection.focus(), layout)?;
        Some(TextSelection { anchor, focus })
    }

    fn span_id_and_node<B: Brush>(
        &mut self,
        next_node_id: &mut impl FnMut() -> NodeId,
        ids: &mut HashSet<NodeId>,
        run: &Run<'_, B>,
        alignment: Option<Alignment>,
        span_path: ClusterPath,
    ) -> (NodeId, Node) {
        // If we encountered this same span path in the previous
        // accessibility pass, reuse the same AccessKit ID. Otherwise,
        // allocate a new one. This enables stable node IDs when merely
        // updating the content of existing spans.
        let id = self
            .access_ids_by_span_path
            .get(&span_path)
            .copied()
            .unwrap_or_else(|| {
                let id = (*next_node_id)();
                self.access_ids_by_span_path.insert(span_path, id);
                self.span_paths_by_access_id.insert(id, span_path);
                id
            });
        ids.insert(id);
        let mut node = Node::new(Role::TextRun);
        node.set_text_direction(if run.is_rtl() {
            TextDirection::RightToLeft
        } else {
            TextDirection::LeftToRight
        });

        let font = run.font();
        if let Ok(font_ref) = FontRef::from_index(font.font.data.as_ref(), font.font.index)
            && let Ok(name) = font_ref.name()
        {
            for n in name.name_record().iter() {
                if n.name_id.get() == NameId::FAMILY_NAME {
                    if let Ok(string) = n.string(name.string_data()) {
                        node.set_font_family(string.to_string());
                    }
                    break;
                }
            }
        }
        node.set_font_size(run.font_size());
        let attrs = run.font_attrs();
        node.set_font_weight(attrs.weight.value());
        if matches!(attrs.style, FontStyle::Italic) {
            node.set_italic();
        }
        if let Some(align) = alignment {
            node.set_text_align(match align {
                Alignment::Start => {
                    if run.is_rtl() {
                        TextAlign::Right
                    } else {
                        TextAlign::Left
                    }
                }
                Alignment::End => {
                    if run.is_rtl() {
                        TextAlign::Left
                    } else {
                        TextAlign::Right
                    }
                }
                Alignment::Left => TextAlign::Left,
                Alignment::Center => TextAlign::Center,
                Alignment::Right => TextAlign::Right,
                Alignment::Justify => TextAlign::Justify,
            });
        }

        (id, node)
    }
}

/// Exposes a [`PlainEditor`] to assistive technologies.
///
/// This wraps [`LayoutAccessibility`] with the editor-specific parts: the text selection,
/// and handling of AccessKit's [`Action::SetTextSelection`].
///
/// [`PlainEditor`]: parley::editing::PlainEditor
#[derive(Clone, Default, Debug)]
pub struct PlainEditorAccessibility {
    layout_access: LayoutAccessibility,
}

impl PlainEditorAccessibility {
    /// Perform an accessibility update for the editor driven by `driver`.
    ///
    /// `node` is the node for the text input itself; the spans are pushed onto `update`
    /// as its children. See [`LayoutAccessibility::build_nodes`] for the remaining
    /// arguments.
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors `LayoutAccessibility::build_nodes`"
    )]
    pub fn build_nodes<B: Brush>(
        &mut self,
        driver: &mut PlainEditorDriver<'_, B>,
        update: &mut TreeUpdate,
        node: &mut Node,
        next_node_id: impl FnMut() -> NodeId,
        x_offset: f64,
        y_offset: f64,
        set_brush_properties: impl Fn(&mut Node, &Style<B>),
    ) {
        driver.refresh_layout();
        let editor = &*driver.editor;
        let layout = editor
            .try_layout()
            .expect("the layout was just refreshed, so it is up to date");
        self.layout_access.build_nodes(
            editor.raw_text(),
            layout,
            update,
            node,
            next_node_id,
            x_offset,
            y_offset,
            set_brush_properties,
        );
        // The IME can ask for the caret to be hidden, in which case there is no
        // selection to report.
        if editor.is_cursor_visible() {
            if let Some(selection) = self
                .layout_access
                .selection_to_access_selection(editor.raw_selection(), layout)
            {
                node.set_text_selection(selection);
            }
        } else {
            node.clear_text_selection();
        }
        node.add_action(Action::SetTextSelection);
    }

    /// Handle an AccessKit action request targeting the editor driven by `driver`.
    ///
    /// Returns `true` if the request was handled.
    pub fn handle_action_request<B: Brush>(
        &self,
        driver: &mut PlainEditorDriver<'_, B>,
        request: &accesskit::ActionRequest,
    ) -> bool {
        if request.action == Action::SetTextSelection
            && let Some(accesskit::ActionData::SetTextSelection(selection)) = &request.data
        {
            self.select_from_access_selection(driver, selection);
            return true;
        }
        false
    }

    /// Set the editor's selection from a selection over the nodes built by
    /// [`Self::build_nodes`].
    pub fn select_from_access_selection<B: Brush>(
        &self,
        driver: &mut PlainEditorDriver<'_, B>,
        selection: &TextSelection,
    ) {
        driver.refresh_layout();
        let selection = driver.editor.try_layout().and_then(|layout| {
            self.layout_access
                .selection_from_access_selection(selection, layout)
        });
        if let Some(selection) = selection {
            driver.set_selection(selection);
        }
    }
}

/// Returns `true` if the layout's text ends with a hard line break, meaning the layout has
/// a trailing "phantom" line that a cursor at the end of the text belongs to.
fn ends_with_hard_line_break<B: Brush>(layout: &Layout<B>) -> bool {
    layout
        .text_len()
        .checked_sub(1)
        .and_then(|index| Cluster::from_byte_index(layout, index))
        .is_some_and(|cluster| cluster.is_hard_line_break())
}

fn link_spans(prev_id: NodeId, prev: &mut Node, next_id: NodeId, next: &mut Node) {
    prev.set_next_on_line(next_id);
    next.set_previous_on_line(prev_id);
}

#[allow(
    clippy::too_many_arguments,
    reason = "internal helper, split for clarity"
)]
fn finish_span(
    node: &mut Node,
    x_offset: f64,
    y_offset: f64,
    metrics: &LineMetrics,
    run_offset: f32,
    span_offset: f32,
    span_advance: f32,
    span_text: String,
    character_lengths: Vec<u8>,
    character_positions: Vec<f32>,
    character_widths: Vec<f32>,
    word_starts: Vec<u8>,
) {
    node.set_bounds(Rect {
        x0: x_offset + (run_offset + span_offset) as f64,
        y0: y_offset + metrics.content_block_min_coord as f64,
        x1: x_offset + (run_offset + span_offset + span_advance) as f64,
        y1: y_offset + metrics.content_block_max_coord as f64,
    });
    node.set_value(span_text);
    node.set_character_lengths(character_lengths);
    node.set_character_positions(character_positions);
    node.set_character_widths(character_widths);
    node.set_word_starts(word_starts);
}

fn add_span(update: &mut TreeUpdate, parent_node: &mut Node, id: NodeId, node: Node) {
    update.nodes.push((id, node));
    parent_node.push_child(id);
}
