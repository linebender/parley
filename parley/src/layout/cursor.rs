// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text selection support.

use super::{Affinity, Brush, ClusterPath, Layout};
use core::ops::Range;
use peniko::kurbo::Rect;

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub enum VisualCursorMode {
    /// During cursor motion, affinity is adjusted to prioritize the dominant
    /// direction of the layout.
    ///
    /// That is, if the base direction of the layout is left-to-right, then
    /// the visual cursor will represent the position where the next
    /// left-to-right character would be inserted, and vice versa.
    ///
    /// This matches the behavior of Pango's strong cursor.
    #[default]
    Strong,
    /// During cursor motion, affinity is adjusted to prioritize the non-dominant
    /// direction of the layout.
    ///
    /// That is, if the base direction of the layout is left-to-right, then
    /// the visual cursor will represent the position where the next
    /// right-to-left character would be inserted, and vice versa.
    ///
    /// This matches the behavior of Pango's weak cursor.
    Weak,
    /// During cursor motion, affinity is adjusted based on the directionality
    /// of the incoming position.
    ///
    /// That is, if a directional boundary is entered from a left-to-right run
    /// of text, then the cursor will represent the position where the next
    /// left-to-right character would be inserted, and vice versa.
    ///
    /// This matches the behavior of Firefox.
    Adaptive,
}

impl VisualCursorMode {
    /// Returns the preferred RTL state for the given layout.
    ///
    /// This is used to handle cursor modes when moving visually
    /// by cluster.
    fn prefer_rtl<B: Brush>(self, layout: &Layout<B>) -> Option<bool> {
        match self {
            Self::Strong => Some(layout.is_rtl()),
            Self::Weak => Some(!layout.is_rtl()),
            Self::Adaptive => None,
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
    /// Creates a new cursor for the given layout and point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let (path, affinity) = ClusterPath::from_point(layout, x, y);
        Self::from_cluster_path(layout, path, affinity)
    }

    /// Returns a new cursor for the given layout, byte index and affinity.
    pub fn from_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        let path = ClusterPath::from_byte_index(layout, index);
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
                let is_left_side = affinity.is_visually_leading(is_rtl);
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

    /// Returns a new cursor with internal state recomputed to match the given
    /// layout.
    ///
    /// This should be called whenever the layout is rebuilt or resized.
    #[must_use]
    pub fn refresh<B: Brush>(&self, layout: &Layout<B>) -> Self {
        Self::from_index(layout, self.index as usize, self.affinity)
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

    /// Returns the associated affinity for this cursor.
    pub fn affinity(&self) -> Affinity {
        self.affinity
    }

    /// Returns the visual geometry of the cursor where the next character
    /// matching the base direction of the layout would be inserted.
    ///
    /// If the current cursor is not on a directional boundary, this is also
    /// the location where characters opposite the base direction would be
    /// inserted.
    pub fn strong_geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        if self.is_rtl == layout.is_rtl() {
            self.geometry(layout, size)
        } else {
            self.bidi_link_geometry(layout, size)
                .or_else(|| self.geometry(layout, size))
        }
    }

    /// Returns the visual geometry of the cursor where the next character
    /// that is opposite the base direction of the layout would be inserted.
    ///
    /// This returns `None` when the current cursor is not on a directional
    /// boundary.
    pub fn weak_geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        // Weak cursor only exists if we're on a directional boundary
        let bidi_link = self.bidi_link_geometry(layout, size)?;
        if self.is_rtl == layout.is_rtl() {
            Some(bidi_link)
        } else {
            self.geometry(layout, size)
        }
    }

    fn geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        let metrics = *self.path.line(layout)?.metrics();
        let line_x = self.visual_offset as f64;
        Some(Rect::new(
            line_x,
            metrics.min_coord as f64,
            line_x + size as f64,
            metrics.max_coord as f64,
        ))
    }

    fn bidi_link_geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        let (path, cluster) = self.path.bidi_link_cluster(layout, self.affinity)?;
        let mut line_x = path.visual_offset(layout)? as f64;
        let run = path.run(layout)?;
        if run.logical_to_visual(path.logical_index())? != 0 {
            line_x += cluster.advance() as f64;
        }
        let metrics = *path.line(layout)?.metrics();
        Some(Rect::new(
            line_x,
            metrics.min_coord as f64,
            line_x + size as f64,
            metrics.max_coord as f64,
        ))
    }

    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>, mode: VisualCursorMode) -> Self {
        let prefer_rtl = mode.prefer_rtl(layout);
        if self.affinity.is_visually_leading(self.is_rtl) {
            // Check for directional boundary condition
            if let Some((next_path, next_cluster)) = self.path.next_visual_cluster(layout) {
                if next_cluster.is_rtl() != self.is_rtl {
                    println!("MOVING RIGHT INTO BIDI BOUNDARY");
                    if let Some(prefer_rtl) = prefer_rtl {
                        if self.is_rtl != prefer_rtl {
                            return Self::from_cluster_path(
                                layout,
                                next_path,
                                self.affinity.invert(),
                            );
                        }
                    }
                }
            }
            // We're moving right so we want to track right-side affinity;
            // let's swap.
            Self::from_index(layout, self.index as usize, self.affinity.invert())
        } else if let Some((next, next_cluster)) = self.path.next_visual_cluster(layout) {
            let next_rtl = next_cluster.is_rtl();
            // Check for directional boundary condition
            if let Some((next_next, next_next_cluster)) = next.next_visual_cluster(layout) {
                if next_next_cluster.is_rtl() != next_rtl {
                    println!("MOVING RIGHT INTO BIDI BOUNDARY 2");
                    if let Some(prefer_rtl) = prefer_rtl {
                        if next_rtl != prefer_rtl {
                            return Self::from_cluster_path(layout, next_next, self.affinity);
                        } else {
                            return Self::from_cluster_path(layout, next, self.affinity);
                        }
                    }
                }
            }
            let affinity = if self.is_rtl != next_rtl {
                // println!("MOVING INTO BIDI BOUNDARY");
                self.affinity.invert()
            } else {
                self.affinity
            };
            Self::from_cluster_path(layout, next, affinity)
        } else {
            *self
        }
    }

    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>, mode: VisualCursorMode) -> Self {
        let prefer_rtl = mode.prefer_rtl(layout);
        if !self.affinity.is_visually_leading(self.is_rtl) {
            // Check for directional boundary condition
            if let Some((prev_path, prev_cluster)) = self.path.previous_visual_cluster(layout) {
                if prev_cluster.is_rtl() != self.is_rtl {
                    println!("MOVING LEFT INTO BIDI BOUNDARY");
                    if let Some(prefer_rtl) = prefer_rtl {
                        if self.is_rtl != prefer_rtl {
                            return Self::from_cluster_path(
                                layout,
                                prev_path,
                                self.affinity.invert(),
                            );
                        }
                    }
                }
            }
            // We're moving left so we want to track left-side affinity;
            // let's swap
            Self::from_index(layout, self.index as usize, self.affinity.invert())
        } else if let Some((prev, prev_cluster)) = self.path.previous_visual_cluster(layout) {
            let prev_rtl = prev_cluster.is_rtl();
            // Check for directional boundary condition
            if let Some((prev_prev, prev_prev_cluster)) = prev.previous_visual_cluster(layout) {
                if prev_prev_cluster.is_rtl() != prev_rtl {
                    println!("MOVING LEFT INTO BIDI BOUNDARY 2");
                    if let Some(prefer_rtl) = prefer_rtl {
                        if prev_rtl != prefer_rtl {
                            return Self::from_cluster_path(layout, prev_prev, self.affinity);
                        } else {
                            return Self::from_cluster_path(layout, prev, self.affinity);
                        }
                    }
                }
            }
            let affinity = if self.is_rtl != prev_rtl {
                self.affinity.invert()
            } else {
                self.affinity
            };
            Self::from_cluster_path(layout, prev, affinity)
        } else {
            *self
        }
    }

    pub fn next_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let mut next_path = if self.affinity == Affinity::Upstream {
            self.path.next_logical(layout).unwrap_or(self.path)
        } else {
            self.path
        };
        while let Some((path, cluster)) = next_path.next_word_cluster(layout) {
            next_path = path;
            if !cluster.is_space_or_nbsp() {
                break;
            }
        }
        Self::from_cluster_path(layout, next_path, Affinity::default())
    }

    pub fn previous_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let mut next_path = if self.affinity == Affinity::Upstream {
            self.path.next_logical(layout).unwrap_or(self.path)
        } else {
            self.path
        };
        // let mut next_path = self.path;
        while let Some((path, cluster)) = next_path.previous_word_cluster(layout) {
            next_path = path;
            if !cluster.is_space_or_nbsp() {
                break;
            }
        }
        Self::from_cluster_path(layout, next_path, Affinity::default())
    }
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Selection {
    anchor: Cursor,
    focus: Cursor,
    /// Current horizontal position. Used for tracking line movement.
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
    /// Creates a collapsed selection with the anchor and focus set to the
    /// position associated with the given point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        Cursor::from_point(layout, x, y).into()
    }

    /// Creates a collapsed selection with the anchor and focus set to the
    /// position associated with the given byte index and affinity.
    pub fn from_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        Cursor::from_index(layout, index, affinity).into()
    }

    /// Creates a new selection bounding the word at the given coordinates.
    pub fn word_from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let mut anchor = Cursor::from_point(layout, x, y);
        if !(anchor.affinity == Affinity::Downstream
            && anchor
                .cluster_path()
                .cluster(layout)
                .map(|cluster| cluster.is_word_boundary())
                .unwrap_or_default())
        {
            anchor = anchor.previous_word(layout);
        }
        let mut focus = anchor.next_word(layout);
        if anchor.is_rtl {
            core::mem::swap(&mut anchor, &mut focus);
        }
        Self {
            anchor,
            focus,
            h_pos: None,
        }
    }

    /// Returns the anchor point of the selection.
    ///
    /// This represents the location where the selection was initiated.
    pub fn anchor(&self) -> &Cursor {
        &self.anchor
    }

    /// Returns the focus point of the selection.
    ///
    /// This represents the current location of the selection.
    pub fn focus(&self) -> &Cursor {
        &self.focus
    }

    /// Returns true when the anchor and focus are at the same position.
    pub fn is_collapsed(&self) -> bool {
        self.anchor.text_start == self.focus.text_start
    }

    /// Returns the range of text bounded by this selection.
    ///
    /// This is equivalent to the text that would be removed when pressing the
    /// delete key.
    pub fn text_range(&self) -> Range<usize> {
        if self.is_collapsed() {
            self.focus.text_range()
        } else if self.anchor.text_start < self.focus.text_start {
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

    /// Refreshes the internal cursor state to match the the given layout.
    ///
    /// This should be called whenever the layout is rebuilt or resized.
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

    /// Returns a new selection with the focus moved to the next cluster in
    /// visual order.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn next_visual<B: Brush>(
        &self,
        layout: &Layout<B>,
        mode: VisualCursorMode,
        extend: bool,
    ) -> Self {
        self.maybe_extend(self.focus.next_visual(layout, mode), extend)
    }

    /// Returns a new selection with the focus moved to the previous cluster in
    /// visual order.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.    
    #[must_use]
    pub fn previous_visual<B: Brush>(
        &self,
        layout: &Layout<B>,
        mode: VisualCursorMode,
        extend: bool,
    ) -> Self {
        self.maybe_extend(self.focus.previous_visual(layout, mode), extend)
    }

    /// Returns a new selection with the focus moved to the next word.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.    
    #[must_use]
    pub fn next_word<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(self.focus.next_word(layout), extend)
    }

    /// Returns a new selection with the focus moved to the previous word.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.     
    #[must_use]
    pub fn previous_word<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(self.focus.previous_word(layout), extend)
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

    /// Returns a new selection with the focus moved to the start of the
    /// current line.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.    
    #[must_use]
    pub fn line_start<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some(line) = self.focus.path.line(layout) {
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
        if let Some(line) = self.focus.path.line(layout) {
            self.maybe_extend(
                Cursor::from_index(
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

    /// Returns a new selection with the focus moved to the next line. The
    /// current horizontal position will be maintained.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.     
    #[must_use]
    pub fn next_line<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.move_line(layout, 1, extend).unwrap_or(*self)
    }

    /// Returns a new selection with the focus moved to the previous line. The
    /// current horizontal position will be maintained.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.     
    #[must_use]
    pub fn previous_line<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.move_line(layout, -1, extend).unwrap_or(*self)
    }

    fn move_line<B: Brush>(
        &self,
        layout: &Layout<B>,
        line_delta: isize,
        extend: bool,
    ) -> Option<Self> {
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
        if start.text_start > end.text_start {
            core::mem::swap(&mut start, &mut end);
        }
        let text_range = start.text_start..end.text_start;
        let line_start_ix = start.path.line_index();
        let line_end_ix = end.path.line_index();
        for line_ix in line_start_ix..=line_end_ix {
            let Some(line) = layout.get(line_ix) else {
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
