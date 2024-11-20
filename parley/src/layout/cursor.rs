// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text selection support.

#[cfg(feature = "accesskit")]
use super::LayoutAccessibility;
use super::{Affinity, BreakReason, Brush, Cluster, Layout, Line};
#[cfg(feature = "accesskit")]
use accesskit::TextPosition;
use alloc::vec::Vec;
use core::ops::Range;
use peniko::kurbo::Rect;

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Cursor {
    index: usize,
    affinity: Affinity,
}

impl Cursor {
    pub fn from_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        if let Some(cluster) = Cluster::from_index(layout, index) {
            let index = cluster.text_range().start;
            let affinity = if cluster.is_line_break() == Some(BreakReason::Explicit) {
                Affinity::Downstream
            } else {
                affinity
            };
            Self { index, affinity }
        } else {
            Self {
                index: layout.data.text_len,
                affinity: Affinity::Upstream,
            }
        }
    }

    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let (index, affinity) =
            if let Some((cluster, is_leading)) = Cluster::from_point2(layout, x, y) {
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
        let run = line.run(run_index)?;
        let cluster = run.get(pos.character_index)?;
        let index = cluster.text_range().start;
        Some(Self {
            index,
            affinity: Affinity::Downstream,
        })
    }

    fn from_cluster<B: Brush>(layout: &Layout<B>, cluster: Cluster<B>, moving_right: bool) -> Self {
        Self::from_index(
            layout,
            cluster.text_range().start,
            affinity_for_dir(cluster.is_rtl(), moving_right),
        )
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn affinity(&self) -> Affinity {
        self.affinity
    }

    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.visual_clusters(layout);
        match (left, right) {
            (Some(left), Some(right)) => {
                if left.is_soft_line_break() {
                    if left.is_rtl() && self.affinity == Affinity::Downstream {
                        let index = if right.is_rtl() {
                            right.text_range().end
                        } else {
                            right.text_range().start
                        };
                        return Self::from_index(layout, index, Affinity::Upstream);
                    } else if !left.is_rtl() && self.affinity == Affinity::Upstream {
                        let index = if right.is_rtl() {
                            right.text_range().end
                        } else {
                            right.text_range().start
                        };
                        return Self::from_index(layout, index, Affinity::Downstream);
                    }
                }
                let index = if right.is_rtl() {
                    right.text_range().start
                } else {
                    right.text_range().end
                };
                return Self::from_index(layout, index, affinity_for_dir(right.is_rtl(), true));
            }
            (None, Some(right)) => {
                let index = if right.is_rtl() {
                    right.text_range().start
                } else {
                    right.text_range().end
                };
                return Self::from_index(layout, index, affinity_for_dir(right.is_rtl(), true));
            }
            // We can't move right here
            (Some(_left), None) => {}
            _ => {}
        }
        *self
    }

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
                    return Self::from_index(layout, index, Affinity::Downstream);
                } else if !left.is_rtl() && self.affinity == Affinity::Downstream {
                    let index = if right.is_rtl() {
                        right.text_range().end
                    } else {
                        right.text_range().start
                    };
                    return Self::from_index(layout, index, Affinity::Upstream);
                }
            }
        }
        if let Some(left) = left {
            let index = if left.is_rtl() {
                left.text_range().end
            } else {
                left.text_range().start
            };
            return Self::from_index(layout, index, affinity_for_dir(left.is_rtl(), false));
        }
        *self
    }

    pub fn next_visual_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.visual_clusters(layout);
        if let Some(mut cluster) = right.or(left) {
            while let Some(next_word) = cluster.next_visual_word() {
                cluster = next_word;
                if !cluster.is_space_or_nbsp() {
                    break;
                }
            }
            return Self::from_cluster(layout, cluster, true);
        }
        *self
    }

    pub fn previous_visual_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.visual_clusters(layout);
        if let Some(mut cluster) = left.or(right) {
            while let Some(next_word) = cluster.previous_visual_word() {
                cluster = next_word;
                if !cluster.is_space_or_nbsp() {
                    break;
                }
            }
            return Self::from_cluster(layout, cluster, false);
        }
        *self
    }

    pub fn next_logical_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.logical_clusters(layout);
        if let Some(mut cluster) = right.or(left) {
            while let Some(next_word) = cluster.next_logical_word() {
                cluster = next_word;
                if !cluster.is_space_or_nbsp() {
                    break;
                }
            }
            return Self::from_cluster(layout, cluster, true);
        }
        *self
    }

    pub fn previous_logical_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let [left, right] = self.logical_clusters(layout);
        if let Some(mut cluster) = left.or(right) {
            while let Some(next_word) = cluster.previous_logical_word() {
                cluster = next_word;
                if !cluster.is_space_or_nbsp() {
                    break;
                }
            }
            return Self::from_cluster(layout, cluster, false);
        }
        *self
    }

    pub fn geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> (Rect, Option<Rect>) {
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
                        (cursor_rect(&cluster, at_end, size), None)
                    } else {
                        (cursor_rect(&right, false, size), None)
                    }
                } else {
                    (cursor_rect(&left, true, size), None)
                }
            }
            [Some(left), None] if left.is_hard_line_break() => {
                (last_line_cursor_rect(layout, size), None)
            }
            [Some(left), _] => (cursor_rect(&left, true, size), None),
            [_, Some(right)] => (cursor_rect(&right, false, size), None),
            _ => (last_line_cursor_rect(layout, size), None),
        }
    }

    pub fn logical_clusters<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> [Option<Cluster<'a, B>>; 2] {
        let upstream = self
            .index
            .checked_sub(1)
            .and_then(|index| Cluster::from_index(layout, index));
        let downstream = Cluster::from_index(layout, self.index);
        [upstream, downstream]
    }

    pub fn visual_clusters<'a, B: Brush>(
        &self,
        layout: &'a Layout<B>,
    ) -> [Option<Cluster<'a, B>>; 2] {
        let mut clusters = [None, None];
        if self.affinity == Affinity::Upstream {
            if let Some(cluster) = self.upstream_cluster(layout) {
                if cluster.is_rtl() {
                    clusters = [cluster.previous_visual(), Some(cluster)];
                } else {
                    clusters = [Some(cluster.clone()), cluster.next_visual()];
                }
            } else if let Some(cluster) = self.downstream_cluster(layout) {
                if cluster.is_rtl() {
                    clusters = [None, Some(cluster)];
                } else {
                    clusters = [Some(cluster), None];
                }
            }
        } else if let Some(cluster) = self.downstream_cluster(layout) {
            if cluster.is_rtl() {
                clusters = [Some(cluster.clone()), cluster.next_visual()];
            } else {
                clusters = [cluster.previous_visual(), Some(cluster)];
            }
        } else if let Some(cluster) = self.upstream_cluster(layout) {
            if cluster.is_rtl() {
                clusters = [None, Some(cluster)];
            } else {
                clusters = [Some(cluster), None];
            }
        }
        clusters
    }

    fn line<B: Brush>(self, layout: &Layout<B>) -> Option<(usize, Line<'_, B>)> {
        let geometry = self.geometry(layout, 0.0).0;
        layout.line_for_offset(geometry.y0 as f32)
    }

    fn upstream_cluster<B: Brush>(self, layout: &Layout<B>) -> Option<Cluster<'_, B>> {
        self.index
            .checked_sub(1)
            .and_then(|index| Cluster::from_index(layout, index))
    }

    fn downstream_cluster<B: Brush>(self, layout: &Layout<B>) -> Option<Cluster<'_, B>> {
        Cluster::from_index(layout, self.index)
    }

    #[cfg(feature = "accesskit")]
    pub fn to_access_position<B: Brush>(
        &self,
        layout: &Layout<B>,
        layout_access: &LayoutAccessibility,
    ) -> Option<TextPosition> {
        let path = self.downstream_cluster(layout)?.path;
        let run_path = (path.line_index(), path.run_index());
        let id = layout_access.access_ids_by_run_path.get(&run_path)?;
        let character_index = path.logical_index();
        Some(TextPosition {
            node: *id,
            character_index,
        })
    }
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Selection {
    anchor: Cursor,
    focus: Cursor,
    h_pos: Option<f32>,
}

impl Selection {
    pub fn new(anchor: Cursor, focus: Cursor) -> Self {
        Self {
            anchor,
            focus,
            h_pos: None,
        }
    }

    pub fn from_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        Cursor::from_index(layout, index, affinity).into()
    }

    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        Cursor::from_point(layout, x, y).into()
    }

    /// Creates a new selection bounding the word at the given point.
    pub fn word_from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let [left, right] = Cursor::from_point(layout, x, y).logical_clusters(layout);
        if let Some(mut cluster) = left.or(right) {
            if !cluster.is_word_boundary() {
                if let Some(prev) = cluster.previous_logical_word() {
                    cluster = prev;
                }
            }
            let anchor = Cursor::from_cluster(layout, cluster, false);
            let focus = anchor.next_logical_word(layout);
            Self {
                anchor,
                focus,
                h_pos: None,
            }
        } else {
            Self::default()
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
        Some(Self {
            anchor,
            focus,
            h_pos: None,
        })
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }

    pub fn anchor(&self) -> Cursor {
        self.anchor
    }

    pub fn focus(&self) -> Cursor {
        self.focus
    }

    /// Returns a new collapsed selection at the position of the current
    /// focus.
    #[must_use]
    pub fn collapse(&self) -> Self {
        Self {
            anchor: self.focus,
            focus: self.focus,
            h_pos: self.h_pos,
        }
    }

    #[must_use]
    pub fn refresh<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let anchor = Cursor::from_index(layout, self.anchor.index, self.anchor.affinity);
        let focus = Cursor::from_index(layout, self.focus.index, self.focus.affinity);
        Self {
            anchor,
            focus,
            h_pos: self.h_pos,
        }
    }

    pub fn text_range(&self) -> Range<usize> {
        let start = self.anchor.index().min(self.focus.index());
        let end = self.focus.index().max(self.anchor.index());
        start..end
    }

    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if !self.is_collapsed() && !extend {
            let anchor_geom = self.anchor.geometry(layout, 0.0).0;
            let focus_geom = self.focus.geometry(layout, 0.0).0;
            let new_focus = if (anchor_geom.y0, anchor_geom.x0) > (focus_geom.y0, focus_geom.x0) {
                self.anchor
            } else {
                self.focus
            };
            return Self {
                anchor: new_focus,
                focus: new_focus,
                h_pos: None,
            };
        }
        self.maybe_extend(self.focus.next_visual(layout), extend)
    }

    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if !self.is_collapsed() && !extend {
            let anchor_geom = self.anchor.geometry(layout, 0.0).0;
            let focus_geom = self.focus.geometry(layout, 0.0).0;
            let new_focus = if (anchor_geom.y0, anchor_geom.x0) < (focus_geom.y0, focus_geom.x0) {
                self.anchor
            } else {
                self.focus
            };
            return Self {
                anchor: new_focus,
                focus: new_focus,
                h_pos: None,
            };
        }
        self.maybe_extend(self.focus.previous_visual(layout), extend)
    }

    /// Returns a new selection with the focus moved to the next word.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn next_visual_word<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(self.focus.next_visual_word(layout), extend)
    }

    /// Returns a new selection with the focus moved to the previous word.
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
        let geometry = self.focus.geometry(layout, 0.0).0;
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
            .unwrap_or_else(|| self.focus.geometry(layout, 0.0).0.x0 as f32);
        let y = line.metrics().max_coord - line.metrics().ascent * 0.5;
        let new_focus = Cursor::from_point(layout, h_pos, y);
        let h_pos = Some(h_pos);
        if extend {
            Self {
                anchor: self.anchor,
                focus: new_focus,
                h_pos,
            }
        } else {
            Self {
                anchor: new_focus,
                focus: new_focus,
                h_pos,
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
                Cursor::from_index(layout, line.text_range().start, Affinity::Downstream),
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
                    Cluster::from_index(layout, line.text_range().end - 1)
                        .map(|cluster| (cluster.text_range().start, Affinity::Downstream))
                })
                .flatten()
                .unwrap_or_else(|| (line.text_range().end, Affinity::Upstream));
            self.maybe_extend(Cursor::from_index(layout, index, affinity), extend)
        } else {
            *self
        }
    }

    /// Returns a new selection with the focus extended to the given point.
    #[must_use]
    pub fn extend_to_point<B: Brush>(&self, layout: &Layout<B>, x: f32, y: f32) -> Self {
        let focus = Cursor::from_point(layout, x, y);
        Self {
            anchor: self.anchor,
            focus,
            h_pos: None,
        }
    }

    /// Returns a vector containing the rectangles which represent the visual
    /// geometry of this selection for the given layout.
    ///
    /// This is a convenience method built on [`geometry_with`](Self::geometry_with).
    pub fn geometry<B: Brush>(&self, layout: &Layout<B>) -> Vec<Rect> {
        let mut rects = Vec::new();
        self.geometry_with(layout, |rect| rects.push(rect));
        rects
    }

    /// Invokes `f` with the sequence of rectangles which represent the visual
    /// geometry of this selection for the given layout.
    ///
    /// This avoids allocation if the intent is to render the rectangles
    /// immediately.
    pub fn geometry_with<B: Brush>(&self, layout: &Layout<B>, mut f: impl FnMut(Rect)) {
        // Ensure we add some visual indicator for selected empty
        // lines.
        // Make this configurable?
        const MIN_RECT_WIDTH: f64 = 4.0;
        if self.is_collapsed() {
            return;
        }
        let mut start = self.anchor;
        let mut end = self.focus;
        if start.index > end.index {
            core::mem::swap(&mut start, &mut end);
        }
        let text_range = start.index..end.index;
        // let line_start_ix = start.path.line_index();
        // let line_end_ix = end.path.line_index();
        let line_start_ix = 0;
        let line_end_ix = layout.len() + 1;
        for line_ix in line_start_ix..=line_end_ix {
            let Some(line) = layout.get(line_ix) else {
                continue;
            };
            let metrics = line.metrics();
            let line_min = metrics.min_coord as f64;
            let line_max = metrics.max_coord as f64;
            //if line_ix == line_start_ix || line_ix == line_end_ix {
            // We only need to run the expensive logic on the first and
            // last lines
            let mut start_x = metrics.offset as f64;
            let mut cur_x = start_x;
            for run in line.runs() {
                for cluster in run.visual_clusters() {
                    let advance = cluster.advance() as f64;
                    if text_range.contains(&cluster.text_range().start) {
                        cur_x += advance;
                    } else {
                        if cur_x != start_x {
                            let width = (cur_x - start_x).max(MIN_RECT_WIDTH);
                            f(Rect::new(start_x as _, line_min, start_x + width, line_max));
                        }
                        cur_x += advance;
                        start_x = cur_x;
                    }
                }
            }
            if cur_x != start_x {
                let width = (cur_x - start_x).max(MIN_RECT_WIDTH);
                f(Rect::new(start_x, line_min, start_x + width, line_max));
            }
            // } else {
            //     let x = metrics.offset as f64;
            //     let width = (metrics.advance as f64).max(MIN_RECT_WIDTH);
            //     f(Rect::new(x, line_min, x + width, line_max));
            // }
        }
    }

    pub(crate) fn maybe_extend(&self, focus: Cursor, extend: bool) -> Self {
        if extend {
            Self {
                anchor: self.anchor,
                focus,
                h_pos: None,
            }
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

impl From<Cursor> for Selection {
    fn from(value: Cursor) -> Self {
        Self::new(value, value)
    }
}

fn cursor_rect<B: Brush>(cluster: &Cluster<B>, at_end: bool, size: f32) -> Rect {
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
