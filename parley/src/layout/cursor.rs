// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text selection support.

#[cfg(feature = "accesskit")]
use super::LayoutAccessibility;
use super::{Affinity, BreakReason, Brush, Cluster, ClusterPath, Layout};
#[cfg(feature = "accesskit")]
use accesskit::TextPosition;
use alloc::vec::Vec;
use core::ops::Range;
use peniko::kurbo::Rect;

/// Defines how a cursor will bind to a text position when moving visually.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum VisualMode {
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

impl VisualMode {
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

/// A single position in a layout.
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
    /// Returns a new cursor for the given layout, byte index and affinity.
    pub fn from_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        let Some(cluster) = Cluster::from_index(layout, index) else {
            return Self::default();
        };
        Self::from_cluster(cluster, affinity)
    }

    /// Creates a new cursor for the given layout and point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        let Some((cluster, affinity)) = Cluster::from_point(layout, x, y) else {
            return Self::default();
        };
        Self::from_cluster(cluster, affinity)
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
        let (logical_index, affinity) = if pos.character_index == run.len() {
            (pos.character_index - 1, Affinity::Upstream)
        } else {
            (pos.character_index, Affinity::Downstream)
        };
        let cluster = run.get(logical_index)?;
        Some(Self::from_cluster(cluster, affinity))
    }

    fn from_cluster<B: Brush>(cluster: Cluster<B>, mut affinity: Affinity) -> Self {
        let mut range = cluster.text_range();
        let index = range.start as u32;
        let mut offset = cluster.visual_offset().unwrap_or_default();
        let is_rtl = cluster.is_rtl();
        if cluster.is_line_break() == Some(BreakReason::Explicit) {
            affinity = Affinity::Downstream;
        }
        let is_left_side = affinity.is_visually_leading(is_rtl);
        if !is_left_side {
            offset += cluster.advance();
            if !is_rtl {
                range = cluster
                    .next_logical()
                    .map(|cluster| cluster.text_range())
                    .unwrap_or(range.end..range.end);
            }
        } else if is_rtl {
            range = cluster
                .next_logical()
                .map(|cluster| cluster.text_range())
                .unwrap_or(range.end..range.end);
        }
        Self {
            path: cluster.path(),
            index,
            text_start: range.start as u32,
            text_end: range.end as u32,
            visual_offset: offset,
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
        let line_x = (metrics.offset + self.visual_offset) as f64;
        Some(Rect::new(
            line_x,
            metrics.min_coord as f64,
            line_x + size as f64,
            metrics.max_coord as f64,
        ))
    }

    fn bidi_link_geometry<B: Brush>(&self, layout: &Layout<B>, size: f32) -> Option<Rect> {
        let cluster = self.path.cluster(layout)?.bidi_link(self.affinity)?;
        let mut line_x = cluster.visual_offset()? as f64;
        let run = cluster.run();
        let path = cluster.path();
        if run.logical_to_visual(path.logical_index())? != 0 {
            line_x += cluster.advance() as f64;
        }
        let metrics = *path.line(layout)?.metrics();
        line_x += metrics.offset as f64;
        Some(Rect::new(
            line_x,
            metrics.min_coord as f64,
            line_x + size as f64,
            metrics.max_coord as f64,
        ))
    }

    pub fn next_visual<B: Brush>(&self, layout: &Layout<B>, mode: VisualMode) -> Self {
        let prefer_rtl = mode.prefer_rtl(layout);
        let Some(cluster) = self.path.cluster(layout) else {
            return *self;
        };
        if self.affinity.is_visually_leading(self.is_rtl) {
            if let Some(next_cluster) = cluster.next_visual() {
                // Handle hard line breaks
                if cluster.is_line_break() == Some(BreakReason::Explicit) {
                    // If we're at the front of a hard line break and moving
                    // right, skip directly to the leading edge of the next cluster
                    return Self::from_cluster(next_cluster, self.affinity);
                }
                // Handle text direction boundaries
                if next_cluster.is_rtl() != self.is_rtl {
                    if let Some(prefer_rtl) = prefer_rtl {
                        if self.is_rtl != prefer_rtl {
                            return Self::from_cluster(next_cluster, self.affinity.invert());
                        }
                    }
                }
            }
            // We're moving right so we want to track right-side affinity;
            // let's swap.
            Self::from_index(layout, self.index as usize, self.affinity.invert())
        } else if let Some(next) = cluster.next_visual() {
            // Handle soft line breaks
            if matches!(
                cluster.is_line_break(),
                Some(BreakReason::Regular) | Some(BreakReason::Emergency)
            ) {
                // Without this check, moving to the next line will
                // skip the first character which can be jarring
                return Self::from_cluster(next, self.affinity.invert());
            }
            // And hard line breaks
            if next.is_line_break() == Some(BreakReason::Explicit) {
                if let Some(next_next) = next.next_visual() {
                    return Self::from_cluster(next_next, self.affinity.invert());
                }
            }
            let next_rtl = next.is_rtl();
            if let Some(next_next) = next.next_visual() {
                // Check for directional boundary condition
                if next_next.is_rtl() != next_rtl {
                    if let Some(prefer_rtl) = prefer_rtl {
                        if next_rtl != prefer_rtl {
                            return Self::from_cluster(next_next, self.affinity);
                        } else {
                            return Self::from_cluster(next, self.affinity);
                        }
                    }
                }
            }
            let affinity = if self.is_rtl != next_rtl {
                self.affinity.invert()
            } else {
                self.affinity
            };
            Self::from_cluster(next, affinity)
        } else {
            *self
        }
    }

    pub fn previous_visual<B: Brush>(&self, layout: &Layout<B>, mode: VisualMode) -> Self {
        let prefer_rtl = mode.prefer_rtl(layout);
        let Some(cluster) = self.path.cluster(layout) else {
            return *self;
        };
        if !self.affinity.is_visually_leading(self.is_rtl) {
            // Handle hard line breaks
            if cluster.is_hard_line_break() {
                // If we're at the back of a hard line break and moving
                // left, skip directly to the trailing edge of the next cluster
                if let Some(next) = cluster.previous_logical() {
                    return Self::from_cluster(next, self.affinity);
                }
            }
            // Check for directional boundary condition
            if let Some(prev) = cluster.previous_visual() {
                if prev.is_rtl() != self.is_rtl {
                    if let Some(prefer_rtl) = prefer_rtl {
                        if self.is_rtl != prefer_rtl {
                            return Self::from_cluster(prev, self.affinity.invert());
                        }
                    }
                }
            }
            // We're moving left so we want to track left-side affinity;
            // let's swap
            Self::from_index(layout, self.index as usize, self.affinity.invert())
        } else if let Some(prev) = cluster.previous_visual() {
            // Handle soft line breaks
            if matches!(
                prev.is_line_break(),
                Some(BreakReason::Regular) | Some(BreakReason::Emergency)
            ) {
                // Match the behavior of next_visual: move to the end of the soft line
                // break
                return Self::from_cluster(prev, self.affinity.invert());
            }
            let prev_rtl = prev.is_rtl();
            // Check for directional boundary condition
            if let Some(prev_prev) = prev.previous_visual() {
                if prev_prev.is_rtl() != prev_rtl {
                    if let Some(prefer_rtl) = prefer_rtl {
                        if prev_rtl != prefer_rtl {
                            return Self::from_cluster(prev_prev, self.affinity);
                        } else {
                            return Self::from_cluster(prev, self.affinity);
                        }
                    }
                }
            }
            let affinity = if self.is_rtl != prev_rtl {
                self.affinity.invert()
            } else {
                self.affinity
            };
            Self::from_cluster(prev, affinity)
        } else {
            *self
        }
    }

    pub fn next_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let Some(mut next) = self.path.cluster(layout) else {
            return *self;
        };
        if self.affinity == Affinity::Upstream {
            next = next.next_logical().unwrap_or(next);
        }
        while let Some(cluster) = next.next_word() {
            next = cluster.clone();
            if !cluster.is_space_or_nbsp() {
                break;
            }
        }
        Self::from_cluster(next, Affinity::default())
    }

    pub fn previous_word<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let Some(mut next) = self.path.cluster(layout) else {
            return *self;
        };
        if self.affinity == Affinity::Upstream {
            next = next.next_logical().unwrap_or(next);
        }
        while let Some(cluster) = next.previous_word() {
            next = cluster.clone();
            if !cluster.is_space_or_nbsp() {
                break;
            }
        }
        Self::from_cluster(next, Affinity::default())
    }

    /// Used for determining visual order of two cursors.
    fn visual_order_key(&self) -> (usize, f32) {
        (self.path.line_index(), self.visual_offset)
    }

    #[cfg(feature = "accesskit")]
    pub fn to_access_position<B: Brush>(
        &self,
        layout: &Layout<B>,
        layout_access: &LayoutAccessibility,
    ) -> Option<TextPosition> {
        let run_path = (self.path.line_index(), self.path.run_index());
        let id = layout_access.access_ids_by_run_path.get(&run_path)?;
        let mut character_index = self.path.logical_index();
        // If the affinity is upstream, then that means that the cursor
        // logically follows the cluster specified in its cluster path,
        // so it's "on" the next logical cluster. AccessKit expects us to
        // specify the character that the cursor is "on", so we need to advance
        // to the next one in this case. As an example of when this happens
        // in LTR text: initially the cursor is on the first character of the
        // text with `Affinity::Downstream`. If the user presses Right Arrow,
        // the cursor stays on the same cluster but the affinity is flipped
        // to `Affinity::Upstream`, and now the cursor is between the first
        // and second characters; we interpret that here as being "on"
        // the second character.
        if self.affinity == Affinity::Upstream {
            let run = self.path.run(layout)?;
            if character_index < run.len() {
                character_index += 1;
            }
        }
        Some(TextPosition {
            node: *id,
            character_index,
        })
    }
}

/// A range within a layout.
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
    /// position associated with the given byte index and affinity.
    pub fn from_index<B: Brush>(layout: &Layout<B>, index: usize, affinity: Affinity) -> Self {
        Cursor::from_index(layout, index, affinity).into()
    }

    /// Creates a collapsed selection with the anchor and focus set to the
    /// position associated with the given point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        Cursor::from_point(layout, x, y).into()
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

    /// Creates a new selection bounding the word at the given point.
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
        mode: VisualMode,
        extend: bool,
    ) -> Self {
        if !extend && !self.is_collapsed() {
            if self.focus.visual_order_key() > self.anchor.visual_order_key() {
                return self.focus.into();
            } else {
                return self.anchor.into();
            }
        }
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
        mode: VisualMode,
        extend: bool,
    ) -> Self {
        if !extend && !self.is_collapsed() {
            if self.focus.visual_order_key() < self.anchor.visual_order_key() {
                return self.focus.into();
            } else {
                return self.anchor.into();
            }
        }
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

    /// Returns a new selection with the focus moved to a `Cursor`.
    ///
    /// If `extend` is `true` then the current anchor will be retained,
    /// otherwise the new selection will be collapsed.
    #[must_use]
    pub fn maybe_extend(&self, focus: Cursor, extend: bool) -> Self {
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
        let line_index = self.focus.path.line_index();
        let new_line_index = line_index.saturating_add_signed(delta);
        if delta < 0 && line_index.checked_add_signed(delta).is_none() {
            return self
                .move_lines(layout, -(line_index as isize), extend)
                .line_start(layout, extend);
        } else if delta > 0 && new_line_index > line_limit {
            return self
                .move_lines(layout, (line_limit - line_index) as isize, extend)
                .line_end(layout, extend);
        }
        let Some(line) = layout.get(new_line_index) else {
            return *self;
        };
        let y = line.metrics().baseline - line.metrics().ascent * 0.5;
        let h_pos = self.h_pos.unwrap_or(self.focus.visual_offset);
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
