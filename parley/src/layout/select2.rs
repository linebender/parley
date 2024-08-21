// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text selection support.

use peniko::kurbo::Rect;

use super::{Brush, Cluster, ClusterPath, ClusterSide, Layout, Line, Run};
use core::ops::Range;

/// Determines how a cursor attaches to a cluster.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum Affinity {
    /// Left side for LTR clusters and right side for RTL clusters.
    #[default]
    Downstream = 0,
    /// Right side for LTR clusters and left side for RTL clusters.
    Upstream = 1,
}

impl Affinity {
    pub fn from_rtl_and_side(is_rtl: bool, side: ClusterSide) -> Self {
        match (is_rtl, side) {
            // right edge of RTL and left edge of LTR
            (true, ClusterSide::Trailing) | (false, ClusterSide::Leading) => Affinity::Downstream,
            // left edge of RTL and right edge of LTR
            (true, ClusterSide::Leading) | (false, ClusterSide::Trailing) => Affinity::Upstream,
        }
    }

    pub fn invert(&self) -> Self {
        match self {
            Self::Downstream => Self::Upstream,
            Self::Upstream => Self::Downstream,
        }
    }

    /// Returns true if the cursor should be placed on the left side.
    pub fn is_left_side(&self, is_rtl: bool) -> bool {
        match (*self, is_rtl) {
            (Self::Upstream, true) | (Self::Downstream, false) => true,
            (Self::Upstream, false) | (Self::Downstream, true) => false,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Cursor {
    path: ClusterPath,
    index: u32,
    text_start: u32,
    text_end: u32,
    visual_offset: f32,
    is_rtl: bool,
    affinity: Affinity,
}

impl Cursor {
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let (path, side) = ClusterPath::from_point(layout, x, y);
        let affinity = Affinity::from_rtl_and_side(
            path.cluster(layout)
                .map(|cluster| cluster.is_rtl())
                .unwrap_or_default(),
            side,
        );
        Self::from_cluster_path(layout, path, affinity)
    }

    pub fn from_byte_index<B: Brush>(
        layout: &Layout<B>,
        byte_index: usize,
        affinity: Affinity,
    ) -> Self {
        let path = ClusterPath::from_byte_index(layout, byte_index);
        Self::from_cluster_path(layout, path, affinity)
    }

    fn from_cluster_path<B: Brush>(
        layout: &Layout<B>,
        path: ClusterPath,
        affinity: Affinity,
    ) -> Self {
        let (index, text_start, text_end, visual_offset, is_rtl) =
            if let Some(cluster) = path.cluster(layout) {
                let mut range = cluster.text_range();
                let index = range.start as u32;
                let mut offset = path.visual_offset(layout).unwrap_or_default();
                let is_rtl = cluster.is_rtl();
                let is_left_side = affinity.is_left_side(is_rtl);
                if !is_left_side {
                    offset += cluster.advance();
                    if !is_rtl {
                        range = path
                            .next_logical(layout)
                            .and_then(|path| path.cluster(layout))
                            .map(|cluster| cluster.text_range())
                            .unwrap_or(range.end..range.end);
                    }
                } else if is_rtl {
                    range = path
                        .next_logical(layout)
                        .and_then(|path| path.cluster(layout))
                        .map(|cluster| cluster.text_range())
                        .unwrap_or(range.end..range.end);
                }
                (
                    index,
                    range.start as u32,
                    range.end as u32,
                    offset,
                    cluster.is_rtl(),
                )
            } else {
                Default::default()
            };
        Self {
            path,
            index,
            text_start,
            text_end,
            visual_offset,
            is_rtl,
            affinity,
        }
    }

    #[must_use]
    fn refresh<B: Brush>(&self, layout: &Layout<B>) -> Self {
        Self::from_byte_index(layout, self.index as usize, self.affinity)
    }

    /// Returns the path to the target cluster.
    pub fn cluster_path(&self) -> ClusterPath {
        self.path
    }

    /// Returns the text range of the target cluster.
    pub fn text_range(&self) -> Range<usize> {
        self.text_start as usize..self.text_end as usize
    }

    /// Returns the visual offset of the target cluster along the direction of
    /// text flow.
    pub fn visual_offset(&self) -> f32 {
        self.visual_offset
    }

    /// Returns the byte index associated with the cursor.
    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub fn affinity(&self) -> Affinity {
        self.affinity
    }

    pub fn geometry<B: Brush>(&self, layout: &Layout<B>, width: f32) -> Option<Rect> {
        let metrics = *self.path.line(layout)?.metrics();
        let line_x = self.visual_offset as f64;
        Some(Rect::new(
            line_x,
            metrics.min_coord as f64,
            line_x + width as f64,
            metrics.max_coord as f64,
        ))
    }

    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>) -> Self {
        if self.affinity.is_left_side(self.is_rtl) {
            // We're moving right so we want to track right-side affinity;
            // let's swap
            Self::from_byte_index(layout, self.index as usize, self.affinity.invert())
        } else {
            if let Some(path) = self.path.next_visual(layout) {
                let next_rtl = path
                    .cluster(layout)
                    .map(|cluster| cluster.is_rtl())
                    .unwrap_or_default();
                let affinity = if self.is_rtl != next_rtl {
                    self.affinity.invert()
                } else {
                    self.affinity
                };
                Self::from_cluster_path(layout, path, affinity)
            } else {
                *self
            }
        }
    }

    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>) -> Self {
        if !self.affinity.is_left_side(self.is_rtl) {
            // We're moving left so we want to track left-side affinity;
            // let's swap
            Self::from_byte_index(layout, self.index as usize, self.affinity.invert())
        } else {
            if let Some(path) = self.path.previous_visual(layout) {
                let next_rtl = path
                    .cluster(layout)
                    .map(|cluster| cluster.is_rtl())
                    .unwrap_or_default();
                let affinity = if self.is_rtl != next_rtl {
                    self.affinity.invert()
                } else {
                    self.affinity
                };
                Self::from_cluster_path(layout, path, affinity)
            } else {
                *self
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Selection {
    anchor: Cursor,
    focus: Cursor,
    h_pos: Option<f32>,
}

impl From<Cursor> for Selection {
    fn from(value: Cursor) -> Self {
        Self {
            anchor: value,
            focus: value,
            h_pos: None,
        }
    }
}

impl Selection {
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        Cursor::from_point(layout, x, y).into()
    }

    pub fn from_byte_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        Cursor::from_byte_index(layout, index, affinity).into()
    }

    pub fn anchor(&self) -> &Cursor {
        &self.anchor
    }

    pub fn focus(&self) -> &Cursor {
        &self.focus
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor.text_start == self.focus.text_start
    }

    pub fn text_range(&self) -> Range<usize> {
        if self.anchor.text_start < self.focus.text_start {
            self.anchor.text_start as usize..self.focus.text_start as usize
        } else {
            self.focus.text_start as usize..self.anchor.text_start as usize
        }
    }

    /// Returns the index where text should be inserted based on this
    /// selection.
    pub fn insertion_index(&self) -> usize {
        self.focus.text_start as usize
    }

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
        let anchor = self.anchor.refresh(layout);
        let focus = self.focus.refresh(layout);
        Self {
            anchor,
            focus,
            h_pos: None,
        }
    }

    #[must_use]
    pub fn extend_to_point<B: Brush>(&self, layout: &Layout<B>, x: f32, y: f32) -> Self {
        let focus = Cursor::from_point(layout, x, y);
        Self {
            anchor: self.anchor,
            focus,
            h_pos: None,
        }
    }

    // #[must_use]
    // pub fn next_logical<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
    //     self.maybe_extend(
    //         Cursor::from_byte_index(layout, self.focus.text_end as usize),
    //         extend,
    //     )
    // }

    // #[must_use]
    // pub fn prev_logical<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
    //     self.maybe_extend(
    //         Cursor::from_byte_index(layout, self.focus.text_start.saturating_sub(1) as usize),
    //         extend,
    //     )
    // }

    #[must_use]
    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(self.focus.next_visual(layout), extend)
    }

    #[must_use]
    pub fn prev_visual<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(self.focus.previous_visual(layout), extend)
    }

    fn maybe_extend(&self, focus: Cursor, extend: bool) -> Self {
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

    #[must_use]
    pub fn line_start<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some(line) = self.focus.path.line(layout) {
            self.maybe_extend(
                Cursor::from_byte_index(layout, line.text_range().start, Affinity::Downstream),
                extend,
            )
        } else {
            *self
        }
    }

    #[must_use]
    pub fn line_end<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some(line) = self.focus.path.line(layout) {
            self.maybe_extend(
                Cursor::from_byte_index(
                    layout,
                    line.text_range().end.saturating_sub(1),
                    Affinity::Upstream,
                ),
                extend,
            )
        } else {
            *self
        }
    }

    #[must_use]
    pub fn next_line<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.move_line(layout, 1, extend).unwrap_or(*self)
    }

    #[must_use]
    pub fn prev_line<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.move_line(layout, -1, extend).unwrap_or(*self)
    }

    fn move_line<B: Brush>(
        &self,
        layout: &Layout<B>,
        line_delta: isize,
        extend: bool,
    ) -> Option<Self> {
        // let line_index = self
        //     .focus
        //     .placement
        //     .primary_position()
        //     .line_index
        //     .saturating_add_signed(line_delta);
        let line_index = self
            .focus
            .path
            .line_index()
            .saturating_add_signed(line_delta);
        let line = layout.get(line_index)?;
        let y = line.metrics().baseline - line.metrics().ascent * 0.5;
        let h_pos = self.h_pos.unwrap_or(self.focus.visual_offset);
        let new_focus = Cursor::from_point(layout, h_pos, y);
        let h_pos = Some(h_pos);
        Some(if extend {
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
        })
    }

    pub fn visual_focus<B: Brush>(&self, layout: &Layout<B>) -> Option<Rect> {
        self.focus.geometry(layout, 1.5)
    }

    // pub fn visual_alternate_focus<B: Brush>(
    //     &self,
    //     layout: &Layout<B>,
    // ) -> Option<peniko::kurbo::Line> {
    //     visual_for_cursor(layout, self.focus.placement.alternate_position())
    // }

    // pub fn visual_anchor<B: Brush>(&self, layout: &Layout<B>) -> Option<peniko::kurbo::Line> {
    //     self.anchor.path.visual_line(layout).map(|line| {
    //         let metrics = line.metrics();
    //         let line_min = (metrics.baseline - metrics.ascent - metrics.leading * 0.5) as f64;
    //         let line_max = line_min + metrics.line_height as f64;
    //         let line_x = self.anchor.offset as f64;
    //         peniko::kurbo::Line::new((line_x, line_min - 10.0), (line_x, line_max - 10.0))
    //     })
    // }

    pub fn geometry<B: Brush>(&self, layout: &Layout<B>) -> Vec<Rect> {
        let mut rects = Vec::new();
        self.geometry_with(layout, |rect| rects.push(rect));
        rects
    }

    pub fn geometry_with<B: Brush>(&self, layout: &Layout<B>, mut f: impl FnMut(Rect)) {
        // Ensure we add some visual indicator for selected empty
        // lines.
        const MIN_RECT_WIDTH: f64 = 4.0;
        if self.is_collapsed() {
            return;
        }
        let mut start = self.anchor;
        let mut end = self.focus;
        if start.text_start > end.text_start {
            core::mem::swap(&mut start, &mut end);
        }
        let text_range = start.text_start..end.text_start;
        let line_start_ix = start.path.line_index();
        let line_end_ix = end.path.line_index();
        for line_ix in line_start_ix..=line_end_ix {
            let Some(line) = layout.get(line_ix as usize) else {
                continue;
            };
            let metrics = line.metrics();
            let line_min = metrics.min_coord as f64;
            let line_max = metrics.max_coord as f64;
            if line_ix == line_start_ix || line_ix == line_end_ix {
                // We only need to run the expensive logic on the first and
                // last lines
                let mut start_x = metrics.offset as f64;
                let mut cur_x = start_x;
                for run in line.runs() {
                    for cluster in run.visual_clusters() {
                        let advance = cluster.advance() as f64;
                        if text_range.contains(&(cluster.text_range().start as u32)) {
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
            } else {
                let x = metrics.offset as f64;
                let width = (metrics.advance as f64).max(MIN_RECT_WIDTH);
                f(Rect::new(x, line_min, x + width, line_max));
            }
        }
    }
}
