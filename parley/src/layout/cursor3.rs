// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text selection support.

use peniko::kurbo::Rect;

use super::{Affinity, Brush, Cluster, ClusterPath, ClusterSide, Layout, Line, Run};
use core::ops::Range;

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub enum CursorMode {
    #[default]
    Strong,
    Weak,
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Cursor {
    pub index: CursorIndex,
    text_start: u32,
    text_end: u32,
}

impl Cursor {
    pub fn from_point<B: Brush>(
        layout: &Layout<B>,
        mode: Option<CursorMode>,
        x: f32,
        y: f32,
    ) -> Self {
        let (path, affinity) = ClusterPath::from_point(layout, x, y);
        if let Some(cluster) = path.cluster(layout) {
            let index = if affinity.is_visually_leading(cluster.is_rtl()) {
                cluster.text_range().start
            } else {
                // path.next_logical(layout).and_then(|p| p.cluster(layout)).and_then(|c| c.text_range().start))
                cluster.text_range().end
            };
            Self::from_byte_index(layout, mode, index, Affinity::Downstream)
        } else {
            Self::default()
        }
    }

    pub fn from_byte_index<B: Brush>(
        layout: &Layout<B>,
        mode: Option<CursorMode>,
        index: usize,
        affinity: Affinity,
    ) -> Self {
        Self::from_cursor_index(layout, CursorIndex::new(layout, index, affinity))
    }

    fn from_cursor_index<B: Brush>(layout: &Layout<B>, index: CursorIndex) -> Self {
        let range = index.text_range(layout);
        Self {
            index,
            text_start: range.start as u32,
            text_end: range.end as u32,
        }
    }

    pub fn index(&self) -> usize {
        self.index.index as usize
    }

    pub fn affinity(&self) -> Affinity {
        self.index.affinity
    }

    pub fn text_range(&self) -> Range<usize> {
        self.text_start as usize..self.text_end as usize
    }

    #[must_use]
    fn refresh<B: Brush>(&self, layout: &Layout<B>) -> Self {
        Self::from_byte_index(layout, None, self.index.index as usize, self.index.affinity)
    }

    pub fn geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        self.index.geometry(layout, size)
    }

    pub fn weak_geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        self.index.weak_geometry(layout, size)
    }

    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>) -> Self {
        self.index
            .next_visual(layout)
            .map(|ix| Self::from_cursor_index(layout, ix))
            .unwrap_or(*self)
    }

    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>) -> Self {
        self.index
            .previous_visual(layout)
            .map(|ix| Self::from_cursor_index(layout, ix))
            .unwrap_or(*self)
    }
}

fn next_logical_range<B: Brush>(layout: &Layout<B>, path: ClusterPath) -> Option<Range<usize>> {
    Some(path.next_logical(layout)?.cluster(layout)?.text_range())
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub enum ClusterPartition {
    /// No partition.
    #[default]
    None,
    /// Text direction changes between two clusters.
    TextDirection,
    /// Soft line break between two clusters.
    SoftLine,
    /// Hard line break between two clusters.
    HardLine,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct CursorIndex {
    pub index: u32,
    pub affinity: Affinity,
    pub kind: CursorIndexKind,
}

impl Default for CursorIndex {
    fn default() -> Self {
        Self {
            index: 0,
            affinity: Default::default(),
            kind: CursorIndexKind::Start(Default::default()),
        }
    }
}

impl CursorIndex {
    pub fn new<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        let index = index.min(layout.data.text_len);
        let kind = CursorIndexKind::new(layout, index);
        Self {
            index: index as u32,
            affinity,
            kind,
        }
    }

    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>) -> Option<Self> {
        match self.kind {
            CursorIndexKind::Start(path) => {
                let next = path.next_visual(layout)?;
                let index = next.cluster(layout)?.text_range().start;
                Some(Self::new(layout, index, self.affinity))
            }
            CursorIndexKind::Between(prev, next, _partition) => {
                let cluster = prev.cluster(layout)?;
                let new = prev.next_visual(layout)?.next_visual(layout)?;
                let new_cluster = new.cluster(layout)?;
                let range = new_cluster.text_range();
                let index = if new_cluster.is_rtl() {
                    range.end
                } else {
                    range.start
                };
                let affinity = if cluster.is_rtl() != new_cluster.is_rtl() {
                    self.affinity.invert()
                } else {
                    self.affinity
                };
                Some(Self::new(layout, index, affinity))
            }
            CursorIndexKind::End(_) => None,
        }
    }

    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>) -> Option<Self> {
        match self.kind {
            CursorIndexKind::Start(_) => None,
            CursorIndexKind::Between(prev, _next, _partition) => {
                let cluster = prev.cluster(layout)?;
                let new = prev.previous_visual(layout)?;
                let new_cluster = new.cluster(layout)?;
                let range = new_cluster.text_range();
                let index = if new_cluster.is_rtl() {
                    range.end
                } else {
                    range.start
                };
                let affinity = if cluster.is_rtl() != new_cluster.is_rtl() {
                    self.affinity.invert()
                } else {
                    self.affinity
                };
                Some(Self::new(layout, index, affinity))
            }
            CursorIndexKind::End(path) => {
                let prev = path.previous_visual(layout)?;
                let index = prev.cluster(layout)?.text_range().end;
                Some(Self::new(layout, index, self.affinity))
            }
        }
    }

    pub fn line_index(&self) -> usize {
        match self.kind {
            CursorIndexKind::Start(path) | CursorIndexKind::End(path) => path.line_index(),
            CursorIndexKind::Between(prev, next, _partition) => {
                let path = match self.affinity {
                    Affinity::Upstream => prev,
                    Affinity::Downstream => next,
                };
                path.line_index()
            }
        }
    }

    pub fn text_range<B: Brush>(&self, layout: &Layout<B>) -> Range<usize> {
        match self.kind {
            CursorIndexKind::Start(path) => path
                .cluster(layout)
                .map(|c| c.text_range())
                .unwrap_or_default(),
            CursorIndexKind::Between(prev, next, _partition) => {
                let path = match self.affinity {
                    Affinity::Downstream => prev,
                    Affinity::Upstream => next,
                };
                path.cluster(layout)
                    .map(|c| c.text_range())
                    .unwrap_or_default()
            }
            CursorIndexKind::End(_path) => layout.data.text_len..layout.data.text_len,
        }
    }

    pub fn geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        let (line_index, offset) = match self.kind {
            CursorIndexKind::Start(path) => {
                let cluster = path.cluster(layout)?;
                let line_index = path.line_index();
                let mut offset = path.visual_offset(layout)?;
                if cluster.is_rtl() {
                    offset += cluster.advance();
                }
                (line_index, offset)
            }
            CursorIndexKind::Between(path, _next, _partition) => {
                let cluster = path.cluster(layout)?;
                let line_index = path.line_index();
                let mut offset = path.visual_offset(layout)?;
                if self.affinity.is_visually_leading(cluster.is_rtl()) {
                    offset += cluster.advance();
                }
                (line_index, offset)
            }
            CursorIndexKind::End(path) => {
                let cluster = path.cluster(layout)?;
                let line_index = path.line_index();
                let mut offset = path.visual_offset(layout)?;
                if !cluster.is_rtl() {
                    offset += cluster.advance();
                }
                (line_index, offset)
            }
        };
        let line = layout.get(line_index)?;
        let metrics = line.metrics();
        Some(Rect::new(
            offset as f64,
            metrics.min_coord as f64,
            offset as f64 + size as f64,
            metrics.max_coord as f64,
        ))
    }

    pub fn weak_geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        match self.kind {
            CursorIndexKind::Start(_)
            | CursorIndexKind::End(_)
            | CursorIndexKind::Between(
                _,
                _,
                ClusterPartition::None | ClusterPartition::SoftLine | ClusterPartition::HardLine,
            ) => None,
            CursorIndexKind::Between(prev, next, ClusterPartition::TextDirection) => {
                let path = match self.affinity {
                    Affinity::Downstream => next,
                    Affinity::Upstream => prev,
                };
                let cluster = path.cluster(layout)?;
                let line_index = path.line_index();
                let mut offset = path.visual_offset(layout)? + cluster.advance();
                // if cluster.is_rtl() {
                //     offset += cluster.advance();
                // }
                let line = layout.get(line_index)?;
                let metrics = line.metrics();
                Some(Rect::new(
                    offset as f64,
                    metrics.min_coord as f64,
                    offset as f64 + size as f64,
                    metrics.max_coord as f64,
                ))
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CursorIndexKind {
    /// Index is at the start of the text.
    Start(ClusterPath),
    /// Index is between two clusters in logical order.
    Between(ClusterPath, ClusterPath, ClusterPartition),
    /// Index is at the end of the text.
    End(ClusterPath),
}

impl CursorIndexKind {
    pub fn new<B: Brush>(layout: &Layout<B>, index: usize) -> Self {
        let path = ClusterPath::from_byte_index(layout, index);
        if index >= layout.data.text_len {
            Self::End(path)
        } else if let Some(prev_path) = path.previous_logical(layout) {
            let partition = if let Some((cluster, prev_cluster)) =
                path.cluster(layout).zip(prev_path.cluster(layout))
            {
                if prev_path.line_index() != path.line_index() {
                    if prev_cluster.is_hard_line_break() {
                        ClusterPartition::HardLine
                    } else {
                        ClusterPartition::SoftLine
                    }
                } else if cluster.is_rtl() != prev_cluster.is_rtl() {
                    ClusterPartition::TextDirection
                } else {
                    ClusterPartition::None
                }
            } else {
                ClusterPartition::None
            };
            Self::Between(prev_path, path, partition)
        } else {
            Self::Start(path)
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
    pub fn from_point<B: Brush>(
        layout: &Layout<B>,
        mode: Option<CursorMode>,
        x: f32,
        y: f32,
    ) -> Self {
        Cursor::from_point(layout, mode, x, y).into()
    }

    pub fn from_byte_index<B: Brush>(
        layout: &Layout<B>,
        mode: Option<CursorMode>,
        index: usize,
        affinity: Affinity,
    ) -> Self {
        Cursor::from_byte_index(layout, mode, index, affinity).into()
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
        let focus = Cursor::from_point(layout, None, x, y);
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
        // if let Some(line) = self.focus.path.line(layout) {
        //     self.maybe_extend(
        //         Cursor::from_byte_index(
        //             layout,
        //             Some(self.focus.mode),
        //             line.text_range().start,
        //             Affinity::Downstream,
        //         ),
        //         extend,
        //     )
        // } else {
        //     *self
        // }
        *self
    }

    #[must_use]
    pub fn line_end<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        // if let Some(line) = self.focus.path.line(layout) {
        //     self.maybe_extend(
        //         Cursor::from_byte_index(
        //             layout,
        //             Some(self.focus.mode),
        //             line.text_range().end.saturating_sub(1),
        //             Affinity::Upstream,
        //         ),
        //         extend,
        //     )
        // } else {
        //     *self
        // }
        *self
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

        // let line_index = self
        //     .focus
        //     .path
        //     .line_index()
        //     .saturating_add_signed(line_delta);
        // let line = layout.get(line_index)?;
        // let y = line.metrics().baseline - line.metrics().ascent * 0.5;
        // let h_pos = self.h_pos.unwrap_or(self.focus.visual_offset);
        // let new_focus = Cursor::from_point(layout, Some(self.focus.mode), h_pos, y);
        // let h_pos = Some(h_pos);
        // Some(if extend {
        //     Self {
        //         anchor: self.anchor,
        //         focus: new_focus,
        //         h_pos,
        //     }
        // } else {
        //     Self {
        //         anchor: new_focus,
        //         focus: new_focus,
        //         h_pos,
        //     }
        // })
        None
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
        let line_start_ix = start.index.line_index();
        let line_end_ix = end.index.line_index();
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
