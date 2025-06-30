// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text selection support.

#[cfg(feature = "accesskit")]
use super::LayoutAccessibility;
use super::{Affinity, BreakReason, Brush, Cluster, ClusterSide, Layout, Line, LineItem};
#[cfg(feature = "accesskit")]
use accesskit::TextPosition;
use alloc::vec::Vec;
use core::ops::Range;
use peniko::kurbo::Rect;
#[cfg(feature = "accesskit")]
use swash::text::cluster::Whitespace;

/// Defines a position with a text layout.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Cursor {
    index: usize,
    affinity: Affinity,
}

impl Cursor {
    /// Creates a new cursor from the given byte index and affinity.
    pub fn from_byte_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        if let Some(cluster) = Cluster::from_byte_index(layout, index) {
            Self {
                index: cluster.text_range().start,
                affinity,
            }
        } else {
            Self {
                index: layout.data.text_len,
                affinity: Affinity::Upstream,
            }
        }
    }

    /// Creates a new cursor from the given coordinates.
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let (index, affinity) = if let Some((cluster, side)) = Cluster::from_point(layout, x, y) {
            let is_leading = side == ClusterSide::Left;
            if cluster.is_rtl() {
                if is_leading {
                    (cluster.text_range().end, Affinity::Upstream)
                } else {
                    (cluster.text_range().start, Affinity::Downstream)
                }
            } else {
                // We never want to position the cursor _after_ a hard
                // line since that cursor appears visually at the start
                // of the next line
                if is_leading || cluster.is_line_break() == Some(BreakReason::Explicit) {
                    (cluster.text_range().start, Affinity::Downstream)
                } else {
                    (cluster.text_range().end, Affinity::Upstream)
                }
            }
        } else {
            (layout.data.text_len, Affinity::Downstream)
        };
        Self { index, affinity }
    }

    #[cfg(feature = "accesskit")]
    pub fn from_access_position<B: Brush>(
        pos: &TextPosition,
        layout: &Layout<B>,
        layout_access: &LayoutAccessibility,
    ) -> Option<Self> {
        let (line_index, run_index) = *layout_access.run_paths_by_access_id.get(&pos.node)?;
        let line = layout.get(line_index)?;
        let run = line.item(run_index)?.run()?;
        let index = run
            .get(pos.character_index)
            .map(|cluster| cluster.text_range().start)
            .unwrap_or(layout.data.text_len);
        Some(Self::from_byte_index(layout, index, Affinity::Downstream))
    }

    fn from_cluster<B: Brush>(
        layout: &Layout<B>,
        cluster: Cluster<'_, B>,
        moving_right: bool,
    ) -> Self {
        Self::from_byte_index(
            layout,
            cluster.text_range().start,
            affinity_for_dir(cluster.is_rtl(), moving_right),
        )
    }

    /// Returns the logical text index of the cursor.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the affinity of the cursor.
    ///
    /// This defines the direction from which the cursor entered its current
    /// position and affects the visual location of the rendered cursor.
    pub fn affinity(&self) -> Affinity {
        self.affinity
    }

    /// Returns a new cursor that is guaranteed to be within the bounds of the
    /// given layout.
    #[must_use]
    pub fn refresh<B: Brush>(&self, layout: &Layout<B>) -> Self {
        Self::from_byte_index(layout, self.index, self.affinity)
    }

    /// Returns a new cursor that is positioned at the previous cluster boundary
    /// in visual order.
    #[must_use]
    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.visual_clusters(layout);
        if let (Some(left), Some(right)) = (&left, &right) {
            if left.is_soft_line_break() {
                if left.is_rtl() && self.affinity == Affinity::Upstream {
                    let index = if right.is_rtl() {
                        left.text_range().start
                    } else {
                        left.text_range().end
                    };
                    return Self::from_byte_index(layout, index, Affinity::Downstream);
                } else if !left.is_rtl() && self.affinity == Affinity::Downstream {
                    let index = if right.is_rtl() {
                        right.text_range().end
                    } else {
                        right.text_range().start
                    };
                    return Self::from_byte_index(layout, index, Affinity::Upstream);
                }
            }
        }
        if let Some(left) = left {
            let index = if left.is_rtl() {
                left.text_range().end
            } else {
                left.text_range().start
            };
            return Self::from_byte_index(layout, index, affinity_for_dir(left.is_rtl(), false));
        }
        *self
    }

    /// Returns a new cursor that is positioned at the next cluster boundary
    /// in visual order.
    #[must_use]
    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.visual_clusters(layout);
        if let (Some(left), Some(right)) = (&left, &right) {
            if left.is_soft_line_break() {
                if left.is_rtl() && self.affinity == Affinity::Downstream {
                    let index = if right.is_rtl() {
                        right.text_range().end
                    } else {
                        right.text_range().start
                    };
                    return Self::from_byte_index(layout, index, Affinity::Upstream);
                } else if !left.is_rtl() && self.affinity == Affinity::Upstream {
                    let index = if right.is_rtl() {
                        right.text_range().end
                    } else {
                        right.text_range().start
                    };
                    return Self::from_byte_index(layout, index, Affinity::Downstream);
                }
            }
            let index = if right.is_rtl() {
                right.text_range().start
            } else {
                right.text_range().end
            };
            return Self::from_byte_index(layout, index, affinity_for_dir(right.is_rtl(), true));
        }
        if let Some(right) = right {
            let index = if right.is_rtl() {
                right.text_range().start
            } else {
                right.text_range().end
            };
            return Self::from_byte_index(layout, index, affinity_for_dir(right.is_rtl(), true));
        }
        *self
    }

    /// Returns a new cursor that is positioned at the next word boundary
    /// in visual order.
    #[must_use]
    pub fn next_visual_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let mut cur = *self;
        loop {
            let next = cur.next_visual(layout);
            if next == cur {
                break;
            }
            cur = next;
            let [Some(left), Some(right)] = cur.visual_clusters(layout) else {
                break;
            };
            if left.is_rtl() {
                if left.is_word_boundary() && !left.is_space_or_nbsp() {
                    break;
                }
            } else if right.is_word_boundary() && !left.is_space_or_nbsp() {
                break;
            }
        }
        cur
    }

    /// Returns a new cursor that is positioned at the previous word boundary
    /// in visual order.
    #[must_use]
    pub fn previous_visual_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let mut cur = *self;
        loop {
            let next = cur.previous_visual(layout);
            if next == cur {
                break;
            }
            cur = next;
            let [Some(left), Some(right)] = cur.visual_clusters(layout) else {
                break;
            };
            if left.is_rtl() {
                if left.is_word_boundary()
                    && (left.is_space_or_nbsp()
                        || (right.is_word_boundary() && !right.is_space_or_nbsp()))
                {
                    break;
                }
            } else if right.is_word_boundary() && !right.is_space_or_nbsp() {
                break;
            }
        }
        cur
    }

    /// Returns a new cursor that is positioned at the next word boundary
    /// in logical order.
    #[must_use]
    pub fn next_logical_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.logical_clusters(layout);
        if let Some(cluster) = right.or(left) {
            let start = cluster.clone();
            let cluster = cluster.next_logical_word().unwrap_or(cluster);
            if cluster.path == start.path {
                return Self::from_byte_index(layout, usize::MAX, Affinity::Downstream);
            }
            return Self::from_cluster(layout, cluster, true);
        }
        *self
    }

    /// Returns a new cursor that is positioned at the previous word boundary
    /// in logical order.
    #[must_use]
    pub fn previous_logical_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.logical_clusters(layout);
        if let Some(cluster) = left.or(right) {
            let cluster = cluster.previous_logical_word().unwrap_or(cluster);
            return Self::from_cluster(layout, cluster, true);
        }
        *self
    }

    /// Returns a rectangle that represents the visual geometry of the cursor
    /// in layout space.
    ///
    /// The `width` parameter defines the width of the resulting rectangle.
    pub fn geometry<B: Brush>(&self, layout: &Layout<B>, width: f32) -> Rect {
        match self.visual_clusters(layout) {
            [Some(left), Some(right)] => {
                if left.is_end_of_line() {
                    if left.is_soft_line_break() {
                        let (cluster, at_end) = if left.is_rtl()
                            && self.affinity == Affinity::Downstream
                            || !left.is_rtl() && self.affinity == Affinity::Upstream
                        {
                            (left, true)
                        } else {
                            (right, false)
                        };
                        cursor_rect(&cluster, at_end, width)
                    } else {
                        cursor_rect(&right, false, width)
                    }
                } else {
                    cursor_rect(&left, true, width)
                }
            }
            [Some(left), None] if left.is_hard_line_break() => last_line_cursor_rect(layout, width),
            [Some(left), _] => cursor_rect(&left, true, width),
            [_, Some(right)] => cursor_rect(&right, false, width),
            _ => last_line_cursor_rect(layout, width),
        }
    }

    /// Returns the pair of clusters that logically bound the cursor
    /// position.
    ///
    /// The order in the array is upstream followed by downstream.
    pub fn logical_clusters<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> [Option<Cluster<'a, B>>; 2] {
        let upstream = self
            .index
            .checked_sub(1)
            .and_then(|index| Cluster::from_byte_index(layout, index));
        let downstream = Cluster::from_byte_index(layout, self.index);
        [upstream, downstream]
    }

    /// Returns the pair of clusters that visually bound the cursor
    /// position.
    ///
    /// The order in the array is left followed by right.
    pub fn visual_clusters<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> [Option<Cluster<'a, B>>; 2] {
        if self.affinity == Affinity::Upstream {
            if let Some(cluster) = self.upstream_cluster(layout) {
                if cluster.is_rtl() {
                    [cluster.previous_visual(), Some(cluster)]
                } else {
                    [Some(cluster.clone()), cluster.next_visual()]
                }
            } else if let Some(cluster) = self.downstream_cluster(layout) {
                if cluster.is_rtl() {
                    [None, Some(cluster)]
                } else {
                    [Some(cluster), None]
                }
            } else {
                [None, None]
            }
        } else if let Some(cluster) = self.downstream_cluster(layout) {
            if cluster.is_rtl() {
                [Some(cluster.clone()), cluster.next_visual()]
            } else {
                [cluster.previous_visual(), Some(cluster)]
            }
        } else if let Some(cluster) = self.upstream_cluster(layout) {
            if cluster.is_rtl() {
                [None, Some(cluster)]
            } else {
                [Some(cluster), None]
            }
        } else {
            [None, None]
        }
    }

    fn line<B: Brush>(self, layout: &Layout<B>) -> Option<(usize, Line<'_, B>)> {
        let geometry = self.geometry(layout, 0.0);
        layout.line_for_offset(geometry.y0 as f32)
    }

    fn upstream_cluster<B: Brush>(self, layout: &Layout<B>) -> Option<Cluster<'_, B>> {
        self.index
            .checked_sub(1)
            .and_then(|index| Cluster::from_byte_index(layout, index))
    }

    fn downstream_cluster<B: Brush>(self, layout: &Layout<B>) -> Option<Cluster<'_, B>> {
        Cluster::from_byte_index(layout, self.index)
    }

    #[cfg(feature = "accesskit")]
    pub fn to_access_position<B: Brush>(
        &self,
        layout: &Layout<B>,
        layout_access: &LayoutAccessibility,
    ) -> Option<TextPosition> {
        if layout.data.text_len == 0 {
            // If the text is empty, just return the first node with a
            // character index of 0.
            return Some(TextPosition {
                node: *layout_access.access_ids_by_run_path.get(&(0, 0))?,
                character_index: 0,
            });
        }
        // Prefer the downstream cluster except at the end of the text
        // where we'll choose the upstream cluster and add 1 to the
        // character index.
        let (offset, path) = self
            .downstream_cluster(layout)
            .map(|cluster| (0, cluster.path))
            .or_else(|| {
                self.upstream_cluster(layout)
                    .map(|cluster| (1, cluster.path))
            })?;
        // If we're at the end of the layout and the layout ends with a newline
        // then make sure we use the "phantom" run at the end so that
        // AccessKit has correct visual geometry for the cursor.
        let (run_path, character_index) = if self.index == layout.data.text_len
            && layout
                .data
                .clusters
                .last()
                .map(|cluster| cluster.info.whitespace() == Whitespace::Newline)
                .unwrap_or_default()
        {
            ((path.line_index() + 1, 0), 0)
        } else {
            (
                (path.line_index(), path.run_index()),
                path.logical_index() + offset,
            )
        };
        let id = layout_access.access_ids_by_run_path.get(&run_path)?;
        Some(TextPosition {
            node: *id,
            character_index,
        })
    }
}

/// Defines a range within a text layout.
#[derive(Copy, Clone, Default, Debug)]
pub struct Selection {
    anchor: Cursor,
    focus: Cursor,
    anchor_base: AnchorBase,
    h_pos: Option<f32>,
}

impl Selection {
    /// Creates a new selection from the given anchor and focus cursors.
    pub fn new(anchor: Cursor, focus: Cursor) -> Self {
        Self {
            anchor,
            focus,
            anchor_base: Default::default(),
            h_pos: None,
        }
    }

    /// Creates a new collapsed selection from the given byte index and
    /// affinity.
    pub fn from_byte_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        Cursor::from_byte_index(layout, index, affinity).into()
    }

    /// Creates a new collapsed selection from the given point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        Cursor::from_point(layout, x, y).into()
    }

    /// Creates a new selection bounding the word at the given coordinates.
    pub fn word_from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        if let Some((mut cluster, _)) = Cluster::from_point(layout, x, y) {
            if !cluster.is_word_boundary() {
                if let Some(prev) = cluster.previous_logical_word() {
                    cluster = prev;
                }
            }
            let anchor = Cursor::from_cluster(layout, cluster.clone(), !cluster.is_rtl());
            let focus = anchor.next_logical_word(layout);
            Self {
                anchor,
                focus,
                anchor_base: AnchorBase::Word(anchor, focus),
                h_pos: None,
            }
        } else {
            Cursor::from_byte_index(layout, layout.data.text_len, Affinity::Upstream).into()
        }
    }

    /// Creates a new selection bounding the line at the given coordinates.
    pub fn line_from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let Self { anchor, focus, .. } = Self::from_point(layout, x, y)
            .line_start(layout, false)
            .line_end(layout, true);
        Self {
            anchor,
            focus,
            anchor_base: AnchorBase::Line(anchor, focus),
            h_pos: None,
        }
    }

    /// Creates a new selection bounding the "logical" line at the given coordinates.
    ///
    /// That is, the line as defined by line break characters, rather than due to soft-wrapping.
    pub fn hard_line_from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let Self { anchor, focus, .. } = Self::from_point(layout, x, y)
            .hard_line_start(layout, false)
            .hard_line_end(layout, true);
        Self {
            anchor,
            focus,
            anchor_base: AnchorBase::Line(anchor, focus),
            h_pos: None,
        }
    }

    #[cfg(feature = "accesskit")]
    pub fn from_access_selection<B: Brush>(
        selection: &accesskit::TextSelection,
        layout: &Layout<B>,
        layout_access: &LayoutAccessibility,
    ) -> Option<Self> {
        let anchor = Cursor::from_access_position(&selection.anchor, layout, layout_access)?;
        let focus = Cursor::from_access_position(&selection.focus, layout, layout_access)?;
        Some(Self::new(anchor, focus))
    }

    /// Returns `true` if the anchor and focus of the selection are the same.
    ///
    /// This means that the selection represents a single position rather than
    /// a range.
    pub fn is_collapsed(&self) -> bool {
        self.anchor.index == self.focus.index
    }

    /// Returns the anchor of the selection.
    ///
    /// In a non-collapsed selection, this indicates where the selection was
    /// initiated.
    pub fn anchor(&self) -> Cursor {
        self.anchor
    }

    /// Returns the focus of the selection.
    ///
    /// In a non-collapsed selection, this indicates the current position.
    pub fn focus(&self) -> Cursor {
        self.focus
    }

    /// Returns a new collapsed selection at the position of the current
    /// focus.
    #[must_use]
    pub fn collapse(&self) -> Self {
        self.focus.into()
    }

    /// Returns a new selection that is guaranteed to be within the bounds of
    /// the given layout.
    #[must_use]
    pub fn refresh<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let anchor = self.anchor.refresh(layout);
        let focus = self.focus.refresh(layout);
        let anchor_base = match self.anchor_base {
            AnchorBase::Cluster => AnchorBase::Cluster,
            AnchorBase::Word(start, end) => {
                AnchorBase::Word(start.refresh(layout), end.refresh(layout))
            }
            AnchorBase::Line(start, end) => {
                AnchorBase::Line(start.refresh(layout), end.refresh(layout))
            }
        };
        let h_pos = self.h_pos;
        Self {
            anchor,
            focus,
            anchor_base,
            h_pos,
        }
    }

    /// Returns the underlying text range of the selection.
    pub fn text_range(&self) -> Range<usize> {
        let start = self.anchor.index().min(self.focus.index());
        let end = self.focus.index().max(self.anchor.index());
        start..end
    }

    /// Returns a new selection with the focus at the next cluster in visual
    /// order.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if !self.is_collapsed() && !extend {
            let anchor_geom = self.anchor.geometry(layout, 0.0);
            let focus_geom = self.focus.geometry(layout, 0.0);
            let new_focus = if (anchor_geom.y0, anchor_geom.x0) > (focus_geom.y0, focus_geom.x0) {
                self.anchor
            } else {
                self.focus
            };
            new_focus.into()
        } else {
            self.maybe_extend(self.focus.next_visual(layout), extend)
        }
    }

    /// Returns a new selection with the focus at the previous cluster in visual
    /// order.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if !self.is_collapsed() && !extend {
            let anchor_geom = self.anchor.geometry(layout, 0.0);
            let focus_geom = self.focus.geometry(layout, 0.0);
            let new_focus = if (anchor_geom.y0, anchor_geom.x0) < (focus_geom.y0, focus_geom.x0) {
                self.anchor
            } else {
                self.focus
            };
            new_focus.into()
        } else {
            self.maybe_extend(self.focus.previous_visual(layout), extend)
        }
    }

    /// Returns a new selection with the focus moved to the next word in visual
    /// order.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn next_visual_word<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(self.focus.next_visual_word(layout), extend)
    }

    /// Returns a new selection with the focus moved to the previous word in
    /// visual order.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn previous_visual_word<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(self.focus.previous_visual_word(layout), extend)
    }

    /// Returns a new selection with the focus moved to the next line. The
    /// current horizontal position will be maintained.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn next_line<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.move_lines(layout, 1, extend)
    }

    /// Returns a new selection with the focus moved to the previous line. The
    /// current horizontal position will be maintained.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn previous_line<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.move_lines(layout, -1, extend)
    }

    /// Returns a new selection with the focus moved the specified number of
    /// lines.
    ///
    /// The sign of the `delta` parameter determines the direction to move with
    /// negative values moving toward previous lines and positive ones moving
    /// toward next lines.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn move_lines<B: Brush>(&self, layout: &Layout<B>, delta: isize, extend: bool) -> Self {
        if delta == 0 {
            return *self;
        }
        let line_limit = layout.len().saturating_sub(1);
        let geometry = self.focus.geometry(layout, 0.0);
        let line_index = layout
            .line_for_offset(geometry.y0 as f32)
            .map(|(ix, _)| ix)
            .unwrap_or(line_limit);
        let new_line_index = line_index.saturating_add_signed(delta);
        if delta < 0 && line_index.checked_add_signed(delta).is_none() && line_limit > 0 {
            return self
                .move_to_line(layout, 0, extend)
                .line_start(layout, extend);
        } else if delta > 0 && new_line_index > line_limit {
            return self
                .move_to_line(layout, line_limit, extend)
                .line_end(layout, extend);
        }
        self.move_to_line(layout, new_line_index, extend)
    }

    #[must_use]
    fn move_to_line<B: Brush>(&self, layout: &Layout<B>, line_index: usize, extend: bool) -> Self {
        let Some(line) = layout.get(line_index) else {
            return *self;
        };
        let h_pos = self
            .h_pos
            .unwrap_or_else(|| self.focus.geometry(layout, 0.0).x0 as f32);
        let y = line.metrics().max_coord - line.metrics().ascent * 0.5;
        let new_focus = Cursor::from_point(layout, h_pos, y);
        let h_pos = Some(h_pos);
        if extend {
            Self {
                anchor: self.anchor,
                focus: new_focus,
                h_pos,
                ..Default::default()
            }
        } else {
            Self {
                anchor: new_focus,
                focus: new_focus,
                h_pos,
                ..Default::default()
            }
        }
    }

    /// Returns a new selection with the focus moved to the start of the
    /// current line.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn line_start<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some((_, line)) = self.focus.line(layout) {
            self.maybe_extend(
                Cursor::from_byte_index(layout, line.text_range().start, Affinity::Downstream),
                extend,
            )
        } else {
            *self
        }
    }

    /// Returns a new selection with the focus moved to just after the previous hard line break.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn hard_line_start<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some((mut hard_line_start_index, line)) = self.focus.line(layout) {
            let mut result_byte_index = line.text_range().start;
            loop {
                if hard_line_start_index == 0 {
                    break;
                }
                let prev_index = hard_line_start_index - 1;
                let Some(line) = layout.get(prev_index) else {
                    unreachable!(
                        "{hard_line_start_index} is a valid line in the layout, but {prev_index} isn't, despite the latter being smaller.\n\
                        The layout has {} lines.",
                        layout.len()
                    );
                };
                if matches!(line.break_reason(), BreakReason::Explicit) {
                    // The start of the line 'hard_line_start_index' is the target point.
                    break;
                }
                result_byte_index = line.text_range().start;
                hard_line_start_index = prev_index;
            }
            self.maybe_extend(
                Cursor::from_byte_index(layout, result_byte_index, Affinity::Downstream),
                extend,
            )
        } else {
            *self
        }
    }

    /// Returns a new selection with the focus moved to the end of the
    /// current line.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn line_end<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some((_, line)) = self.focus.line(layout) {
            let (index, affinity) = (line.break_reason() == BreakReason::Explicit)
                .then(|| {
                    Cluster::from_byte_index(layout, line.text_range().end - 1)
                        .map(|cluster| (cluster.text_range().start, Affinity::Downstream))
                })
                .flatten()
                .unwrap_or_else(|| (line.text_range().end, Affinity::Upstream));
            self.maybe_extend(Cursor::from_byte_index(layout, index, affinity), extend)
        } else {
            *self
        }
    }

    /// Returns a new selection with the focus moved to just before the next hard line break.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn hard_line_end<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some((mut hard_line_end_index, line)) = self.focus.line(layout) {
            let mut result_byte_index = line.text_range().end;
            // If we're already on the last line of the hard line, use that.
            if !matches!(line.break_reason(), BreakReason::Explicit) {
                // Otherwise, check if any of the following lines are the last line of the hard line.
                loop {
                    let next_index = hard_line_end_index + 1;
                    if let Some(line) = layout.get(next_index) {
                        result_byte_index = line.text_range().end;
                        hard_line_end_index = next_index;
                        if matches!(line.break_reason(), BreakReason::Explicit) {
                            // result_byte_index is the last byte of the previous line, so is the value we need
                            break;
                        }
                    } else {
                        // We hit the end of text. Select to the end of the "final" line, which was not an EOF.
                        return self.maybe_extend(
                            Cursor::from_byte_index(layout, result_byte_index, Affinity::Upstream),
                            extend,
                        );
                    }
                }
            }

            // We want to select to "before" the newline character in the hard line, so we have downstream affinity on the boundary before it.
            self.maybe_extend(
                Cursor::from_byte_index(layout, result_byte_index - 1, Affinity::Downstream),
                extend,
            )
        } else {
            *self
        }
    }

    /// Returns a new selection with the focus extended to the given point.
    ///
    /// If the initial selection was created from a word or line, then the new
    /// selection will be extended at the same granularity.
    #[must_use]
    pub fn extend_to_point<B: Brush>(&self, layout: &Layout<B>, x: f32, y: f32) -> Self {
        match self.anchor_base {
            AnchorBase::Cluster => Self::new(self.anchor, Cursor::from_point(layout, x, y)),
            AnchorBase::Word(start, end) => {
                let target = Self::word_from_point(layout, x, y);
                let [anchor, focus] =
                    cursor_min_max(layout, [target.anchor, target.focus, start, end]);
                Self {
                    anchor,
                    focus,
                    anchor_base: self.anchor_base,
                    h_pos: None,
                }
            }
            AnchorBase::Line(start, end) => {
                let target = Self::line_from_point(layout, x, y);
                let [anchor, focus] =
                    cursor_min_max(layout, [target.anchor, target.focus, start, end]);
                Self {
                    anchor,
                    focus,
                    anchor_base: self.anchor_base,
                    h_pos: None,
                }
            }
        }
    }

    /// Returns a new selection with the focus extended to the given point.
    #[must_use]
    pub fn extend_to_precise_point<B: Brush>(&self, layout: &Layout<B>, x: f32, y: f32) -> Self {
        let cluster = Cursor::from_point(layout, x, y);
        match self.anchor_base {
            AnchorBase::Cluster => Self::new(self.anchor, cluster),
            AnchorBase::Word(start, end) => {
                let [anchor, focus] = cursor_min_max(layout, [cluster, cluster, start, end]);
                Self {
                    anchor,
                    focus,
                    anchor_base: self.anchor_base,
                    h_pos: None,
                }
            }
            AnchorBase::Line(start, end) => {
                let [anchor, focus] = cursor_min_max(layout, [cluster, cluster, start, end]);
                Self {
                    anchor,
                    focus,
                    anchor_base: self.anchor_base,
                    h_pos: None,
                }
            }
        }
    }

    /// Returns a new selection with the current anchor and the focus set to
    /// the given value.
    #[must_use]
    pub fn extend(&self, focus: Cursor) -> Self {
        Self::new(self.anchor, focus)
    }

    /// Returns a vector containing the rectangles which represent the visual
    /// geometry of this selection for the given layout, and the indices of the
    /// lines to which they belong.
    ///
    /// This is a convenience method built on [`geometry_with`](Self::geometry_with).
    pub fn geometry<B: Brush>(&self, layout: &Layout<B>) -> Vec<(Rect, usize)> {
        let mut rects = Vec::new();
        self.geometry_with(layout, |rect, line_idx| rects.push((rect, line_idx)));
        rects
    }

    /// Invokes `f` with the sequence of rectangles which represent the visual
    /// geometry of this selection for the given layout, and the indices of the
    /// lines to which they belong.
    ///
    /// This avoids allocation if the intent is to render the rectangles
    /// immediately.
    pub fn geometry_with<B: Brush>(&self, layout: &Layout<B>, mut f: impl FnMut(Rect, usize)) {
        const NEWLINE_WHITESPACE_WIDTH_RATIO: f64 = 0.25;
        if self.is_collapsed() {
            return;
        }
        let mut start = self.anchor;
        let mut end = self.focus;
        if start.index > end.index {
            core::mem::swap(&mut start, &mut end);
        }
        let text_range = start.index..end.index;
        let line_start_ix = start.line(layout).map(|(ix, _)| ix).unwrap_or(0);
        let line_end_ix = end
            .line(layout)
            .map(|(ix, _)| ix)
            .unwrap_or(layout.len() + 1);
        for line_ix in line_start_ix..=line_end_ix {
            let Some(line) = layout.get(line_ix) else {
                continue;
            };
            let metrics = line.metrics();
            let line_min = metrics.min_coord as f64;
            let line_max = metrics.max_coord as f64;
            // Trailing whitespace to indicate that the newline character at the
            // end of this line is selected. It's based on the ascent and
            // descent so it doesn't change with the line height.
            //
            // TODO: the width of this whitespace should be the width of a space
            // (U+0020) character.
            let newline_whitespace = if line.break_reason() == BreakReason::Explicit {
                (metrics.ascent as f64 + metrics.descent as f64) * NEWLINE_WHITESPACE_WIDTH_RATIO
            } else {
                0.0
            };
            if line_ix == line_start_ix || line_ix == line_end_ix {
                // We only need to run the expensive logic on the first and
                // last lines
                let mut start_x = metrics.offset as f64;
                let mut cur_x = start_x;
                let mut cluster_count = 0;
                let mut box_advance = 0.0;
                let mut have_seen_any_runs = false;
                for item in line.items_nonpositioned() {
                    match item {
                        LineItem::Run(run) => {
                            have_seen_any_runs = true;
                            for cluster in run.visual_clusters() {
                                let advance = cluster.advance() as f64 + box_advance;
                                box_advance = 0.0;
                                if text_range.contains(&cluster.text_range().start) {
                                    cluster_count += 1;
                                    cur_x += advance;
                                } else {
                                    if cur_x != start_x {
                                        f(Rect::new(start_x, line_min, cur_x, line_max), line_ix);
                                    }
                                    cur_x += advance;
                                    start_x = cur_x;
                                }
                            }
                        }
                        LineItem::InlineBox(inline_box) => {
                            box_advance += inline_box.width as f64;
                            // HACK: Don't display selections for inline boxes
                            // if they're the first thing in the line. This
                            // makes the selection match the cursor position.
                            if !have_seen_any_runs {
                                cur_x += box_advance;
                                box_advance = 0.0;
                                start_x = cur_x;
                            }
                        }
                    }
                }
                let mut end_x = cur_x;
                if line_ix != line_end_ix || (cluster_count != 0 && metrics.advance == 0.0) {
                    end_x += newline_whitespace;
                }
                if end_x != start_x {
                    f(Rect::new(start_x, line_min, end_x, line_max), line_ix);
                }
            } else {
                let x = metrics.offset as f64;
                let width = metrics.advance as f64;
                f(
                    Rect::new(x, line_min, x + width + newline_whitespace, line_max),
                    line_ix,
                );
            }
        }
    }

    pub(crate) fn maybe_extend(&self, focus: Cursor, extend: bool) -> Self {
        if extend {
            Self::new(self.anchor, focus)
        } else {
            focus.into()
        }
    }

    #[cfg(feature = "accesskit")]
    pub fn to_access_selection<B: Brush>(
        &self,
        layout: &Layout<B>,
        layout_access: &LayoutAccessibility,
    ) -> Option<accesskit::TextSelection> {
        let anchor = self.anchor.to_access_position(layout, layout_access)?;
        let focus = self.focus.to_access_position(layout, layout_access)?;
        Some(accesskit::TextSelection { anchor, focus })
    }
}

impl PartialEq for Selection {
    fn eq(&self, other: &Self) -> bool {
        self.anchor == other.anchor && self.focus == other.focus
    }
}

impl Eq for Selection {}

impl From<Cursor> for Selection {
    fn from(value: Cursor) -> Self {
        Self::new(value, value)
    }
}

#[derive(Copy, Clone, Default, Debug)]
enum AnchorBase {
    #[default]
    Cluster,
    Word(Cursor, Cursor),
    Line(Cursor, Cursor),
}

fn cursor_rect<B: Brush>(cluster: &Cluster<'_, B>, at_end: bool, size: f32) -> Rect {
    let line_x = (cluster.visual_offset().unwrap_or_default()
        + at_end.then(|| cluster.advance()).unwrap_or_default()) as f64;
    let line = cluster.line();
    let metrics = line.metrics();
    Rect::new(
        line_x,
        metrics.min_coord as f64,
        line_x + size as f64,
        metrics.max_coord as f64,
    )
}

fn last_line_cursor_rect<B: Brush>(layout: &Layout<B>, size: f32) -> Rect {
    if let Some(line) = layout.get(layout.len().saturating_sub(1)) {
        let metrics = line.metrics();
        Rect::new(
            0.0,
            metrics.min_coord as f64,
            size as f64,
            metrics.max_coord as f64,
        )
    } else {
        Rect::default()
    }
}

fn affinity_for_dir(is_rtl: bool, moving_right: bool) -> Affinity {
    match (is_rtl, moving_right) {
        (true, true) | (false, false) => Affinity::Downstream,
        _ => Affinity::Upstream,
    }
}

/// Given four cursors, return the left-most and right-most cursors from
/// the set.
///
/// Used for extending word and line selections.
fn cursor_min_max<B: Brush>(layout: &Layout<B>, cursors: [Cursor; 4]) -> [Cursor; 2] {
    let cursor_pos = cursors
        .map(|cursor| (cursor, cursor.geometry(layout, 0.0)))
        .map(|(cursor, rect)| (cursor, (rect.y0, rect.x0)));
    let mut min = cursor_pos[0];
    let mut max = cursor_pos[0];
    for pos in cursor_pos {
        if pos.1 < min.1 {
            min = pos;
        }
        if pos.1 > max.1 {
            max = pos;
        }
    }
    [min.0, max.0]
}
