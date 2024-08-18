// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hit testing.

use super::*;
use alloc::vec::Vec;
use peniko::kurbo::Rect;

/// Represents a position within a layout.
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Cursor {
    /// Path to the target cluster.
    pub path: CursorPath,
    /// Offset to the baseline.
    pub baseline: f32,
    /// Offset to the target cluster along the baseline.
    pub offset: f32,
    /// Advance of the target cluster.
    pub advance: f32,
    /// Start of the target cluster.
    pub text_start: u32,
    /// End of the target cluster.
    pub text_end: u32,
    /// `true` if the target cluster is in a right-to-left run.
    pub is_rtl: bool,
    /// `true` if the cursor was created from a point or position inside the layout.
    pub is_inside: bool,
    /// Possible visual positions of the cursor.
    pub placement: CursorPlacement,
}

impl Cursor {
    /// Creates a new cursor from the specified layout and point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, mut x: f32, y: f32) -> Self {
        let mut result = Self {
            is_inside: x >= 0. && y >= 0.,
            ..Default::default()
        };
        let last_line_index = layout.data.lines.len().saturating_sub(1);
        if let Some((line_index, line)) = layout
            .line_for_offset(y)
            .or_else(|| Some((last_line_index, layout.get(last_line_index)?)))
        {
            let line_index = line_index as u32;
            let line_metrics = line.metrics();
            if y > line_metrics.max_coord {
                result.is_inside = false;
                x = f32::MAX;
            } else if y < 0.0 {
                x = 0.0;
            }
            result.baseline = line_metrics.baseline;
            result.path.line_index = line_index;
            result.path.visual_line_index = line_index;
            result.placement = CursorPlacement::Single(CursorPosition {
                line_index: line_index as u32,
                offset: 0.0,
            });
            let mut cur_edge = line_metrics.offset;
            let last_run_ix = line.data.item_range.len().saturating_sub(1);
            for (run_index, run) in line.runs().enumerate() {
                result.path.run_index = run_index as u32;
                let last_cluster_ix = run.cluster_range().len().saturating_sub(1);
                for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                    let range = cluster.text_range();
                    let advance = cluster.advance();
                    if x <= cur_edge + advance * 0.5 {
                        let index = if run.is_rtl() { range.end } else { range.start };
                        return Self::from_byte_index(layout, index);
                    } else if run.is_rtl() && x < cur_edge + advance {
                        return Self::from_byte_index(layout, range.start);
                    } else if cluster_index == last_cluster_ix && run_index == last_run_ix {
                        let mut cursor = Self::from_byte_index(layout, range.start + 1);
                        let offset = cursor.placement.primary_position().offset;
                        cursor.baseline = line_metrics.baseline;
                        cursor.path.visual_line_index = line_index;
                        cursor.offset = line_metrics.offset + line_metrics.advance;
                        cursor.placement = CursorPlacement::LineBoundary {
                            prefer_end: true,
                            end: CursorPosition {
                                line_index,
                                offset: line_metrics.offset + line_metrics.advance,
                            },
                            start: CursorPosition {
                                line_index: cursor.path.line_index,
                                offset,
                            },
                        };
                        return cursor;
                    } else if x < cur_edge + advance {
                        return Self::from_byte_index(layout, range.end);
                    }
                    cur_edge += advance;
                }
            }
        }
        result.is_inside = false;
        result
    }

    /// Creates a new cursor for the specified layout and text position.
    pub fn from_byte_index<B: Brush>(layout: &Layout<B>, mut index: usize) -> Self {
        let mut result = Self {
            is_inside: true,
            ..Default::default()
        };
        if index >= layout.data.text_len {
            result.is_inside = false;
            result.text_start = layout.data.text_len as u32;
            result.text_end = result.text_start;
            index = layout.data.text_len;
        }
        let last_line_index = layout.data.lines.len().saturating_sub(1);
        if let Some((line_index, line)) = layout
            .line_for_byte_index(index)
            .or_else(|| Some((last_line_index, layout.get(last_line_index)?)))
        {
            let line_index = line_index as u32;
            let line_metrics = line.metrics();
            result.baseline = line_metrics.baseline;
            result.path.line_index = line_index;
            result.path.visual_line_index = line_index;
            let mut last_edge = line_metrics.offset;
            result.offset = last_edge;
            let mut last_is_rtl = None;
            let mut last_run_end = 0.0;
            let mut last_run_start = 0.0;
            for (run_index, run) in line.runs().enumerate() {
                let is_rtl = run.is_rtl();
                result.path.run_index = run_index as u32;
                if !run.text_range().contains(&index) {
                    last_run_start = last_edge;
                    last_edge += run.advance();
                    result.offset = last_edge;
                    last_is_rtl = Some(is_rtl);
                    last_run_end = last_edge;
                    continue;
                }
                let last_cluster_ix = run.cluster_range().len().saturating_sub(1);
                for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                    let range = cluster.text_range();
                    result.text_start = range.start as u32;
                    result.text_end = range.end as u32;
                    result.offset = last_edge;
                    result.placement = CursorPlacement::Single(CursorPosition {
                        line_index: line_index as u32,
                        offset: last_edge,
                    });
                    result.is_rtl = run.is_rtl();
                    result.path.cluster_index =
                        run.visual_to_logical(cluster_index).unwrap() as u32;
                    let advance = cluster.advance();
                    result.advance = advance;
                    if range.contains(&index) {
                        if cluster_index == last_cluster_ix
                            && last_is_rtl.is_some()
                            && last_is_rtl != Some(is_rtl)
                        {
                            if is_rtl {
                                result.placement = CursorPlacement::DirectionalBoundary {
                                    primary: CursorPosition {
                                        line_index,
                                        offset: last_run_end,
                                    },
                                    secondary: CursorPosition {
                                        line_index,
                                        offset: last_edge + advance,
                                    },
                                };
                            } else {
                                result.placement = CursorPlacement::DirectionalBoundary {
                                    primary: CursorPosition {
                                        line_index,
                                        offset: last_edge,
                                    },
                                    secondary: CursorPosition {
                                        line_index,
                                        offset: last_run_start,
                                    },
                                };
                            }
                        } else if is_rtl || !result.is_inside {
                            result.placement = CursorPlacement::Single(CursorPosition {
                                line_index,
                                offset: last_edge + advance,
                            });
                        }
                        // if is_rtl && cluster_index == last_cluster_ix && !last_is_rtl {
                        //     result.offset = last_run_end;
                        // } else if is_rtl || !result.is_inside {
                        //     result.offset += advance;
                        //     result.advance = -advance;
                        // }
                        return result;
                    }
                    last_edge += advance;
                }
            }
            result.placement = CursorPlacement::Single(CursorPosition {
                line_index,
                offset: last_edge,
            });
        }
        result.is_inside = false;
        result
    }

    pub fn text_range(&self) -> Range<usize> {
        self.text_start as usize..self.text_end as usize
    }

    fn at_end_of_line(&self) -> bool {
        match self.placement {
            CursorPlacement::LineBoundary { prefer_end, .. } => prefer_end,
            _ => false,
        }
    }
}

/// Index based path to a cluster.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub struct CursorPath {
    /// Index of the containing line.
    pub line_index: u32,
    /// Index of the run within the containing line.
    pub run_index: u32,
    /// Index of the cluster within the containing run.
    pub cluster_index: u32,
    /// Index of the line containing the visual representation of the
    /// cursor.
    pub visual_line_index: u32,
}

impl CursorPath {
    /// Returns the line for this path and the specified layout.
    pub fn line<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Line<'a, B>> {
        layout.get(self.line_index as usize)
    }

    /// Returns the run for this path and the specified layout.
    pub fn run<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Run<'a, B>> {
        self.line(layout)?.run(self.run_index as usize)
    }

    /// Returns the cluster for this path and the specified layout.
    pub fn cluster<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Cluster<'a, B>> {
        self.run(layout)?.get(self.cluster_index as usize)
    }

    pub fn visual_line<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Line<'a, B>> {
        layout.get(self.visual_line_index as usize)
    }
}

/// Describes the possible visual positions where a [`Cursor`] may be rendered.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CursorPlacement {
    /// A cursor with a single visual position.
    Single(CursorPosition),
    /// A cursor that sits on a boundary where the text direction changes.
    DirectionalBoundary {
        /// Indicates the visual position where left-to-right text would be
        /// inserted.
        ///
        /// This is considered the primary position because it is the one
        /// used by all text editors.
        primary: CursorPosition,
        /// Indicates the visual position where right-to-left text would be
        /// inserted.
        secondary: CursorPosition,
    },
    /// A cursor that sits on a line boundary.
    LineBoundary {
        /// True when the `end` position is preferred. This is set when the
        /// cursor is generated as the result of a hit test that explicitly
        /// targeted the end of the line.
        prefer_end: bool,
        /// The visual position at the end of previous line.
        end: CursorPosition,
        /// The visual position at the start of the current line. This always
        /// represents the logical position of the cursor.
        start: CursorPosition,
    },
}

impl CursorPlacement {
    /// Returns the primary position for the cursor.
    ///
    /// This is the generally preferred position that matches what the majority
    /// of text editors would display.
    pub fn primary_position(&self) -> CursorPosition {
        match *self {
            Self::Single(pos) => pos,
            Self::DirectionalBoundary { primary, .. } => primary,
            Self::LineBoundary {
                prefer_end,
                end,
                start,
            } => {
                if prefer_end {
                    end
                } else {
                    start
                }
            }
        }
    }

    /// Returns the alternate position fo the cursor, if one is available.
    ///
    /// Some text editors will use this to display the two potential insertion
    /// points for mixed-direction text.
    pub fn alternate_position(&self) -> Option<CursorPosition> {
        match *self {
            Self::Single(_) => None,
            Self::DirectionalBoundary { secondary, .. } => Some(secondary),
            Self::LineBoundary {
                prefer_end,
                end,
                start,
            } => {
                if prefer_end {
                    Some(start)
                } else {
                    Some(end)
                }
            }
        }
    }
}

impl Default for CursorPlacement {
    fn default() -> Self {
        Self::Single(CursorPosition::default())
    }
}

/// Visual position of a [`Cursor`].
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct CursorPosition {
    /// Index of the line containing the cursor.
    pub line_index: u32,
    /// Offset of the cursor along the direction of the line.
    pub offset: f32,
}

impl CursorPosition {
    /// Returns the line for this position and the specified layout.
    pub fn line<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Line<'a, B>> {
        layout.get(self.line_index as usize)
    }
}

/// Returns a point that is falls within the vertical bounds of the given line.
fn line_y<B: Brush>(line: &Line<B>) -> f32 {
    line.metrics().baseline - line.metrics().ascent * 0.5
}

fn visual_for_cursor<B: Brush>(
    layout: &Layout<B>,
    cursor_pos: Option<CursorPosition>,
) -> Option<peniko::kurbo::Line> {
    let pos = cursor_pos?;
    pos.line(layout).map(|line| {
        let metrics = line.metrics();
        let line_x = pos.offset as f64;
        peniko::kurbo::Line::new(
            (line_x, metrics.min_coord as f64),
            (line_x, metrics.max_coord as f64),
        )
    })
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Selection {
    anchor: Cursor,
    focus: Cursor,
    focus_at_end: bool,
    h_pos: Option<f32>,
}

impl From<Cursor> for Selection {
    fn from(value: Cursor) -> Self {
        Self {
            anchor: value,
            focus: value,
            focus_at_end: value.at_end_of_line(),
            h_pos: None,
        }
    }
}

impl Selection {
    pub fn from_point<B: Brush>(layout: &Layout<B>, x: f32, y: f32) -> Self {
        Cursor::from_point(layout, x, y).into()
    }

    pub fn from_byte_index<B: Brush>(layout: &Layout<B>, index: usize) -> Self {
        Cursor::from_byte_index(layout, index).into()
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
            focus_at_end: self.focus_at_end,
            h_pos: self.h_pos,
        }
    }

    #[must_use]
    pub fn refresh<B: Brush>(&self, layout: &Layout<B>) -> Self {
        let anchor = Cursor::from_byte_index(layout, self.anchor.text_start as usize);
        let focus = Cursor::from_byte_index(layout, self.focus.text_start as usize);
        let focus =
            if self.focus_at_end && focus.path.run_index == 0 && focus.path.cluster_index == 0 {
                // Hack!
                // On resize, keep track of cursor positions that were set at end
                // of the line.
                if let Some(prev_line) = focus
                    .path
                    .line_index
                    .checked_sub(1)
                    .and_then(|line_ix| layout.get(line_ix as usize))
                {
                    let y = prev_line.metrics().baseline;
                    Cursor::from_point(layout, f32::MAX, y)
                } else {
                    focus
                }
            } else {
                focus
            };
        Self {
            anchor,
            focus,
            focus_at_end: self.focus_at_end,
            h_pos: None,
        }
    }

    #[must_use]
    pub fn extend_to_point<B: Brush>(&self, layout: &Layout<B>, x: f32, y: f32) -> Self {
        let focus = Cursor::from_point(layout, x, y);
        Self {
            anchor: self.anchor,
            focus,
            focus_at_end: focus.at_end_of_line(),
            h_pos: None,
        }
    }

    #[must_use]
    pub fn next_logical<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(
            Cursor::from_byte_index(layout, self.focus.text_end as usize),
            extend,
        )
    }

    #[must_use]
    pub fn prev_logical<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.maybe_extend(
            Cursor::from_byte_index(layout, self.focus.text_start.saturating_sub(1) as usize),
            extend,
        )
    }

    fn maybe_extend(&self, focus: Cursor, extend: bool) -> Self {
        if extend {
            Self {
                anchor: self.anchor,
                focus,
                focus_at_end: focus.at_end_of_line(),
                h_pos: None,
            }
        } else {
            focus.into()
        }
    }

    #[must_use]
    pub fn line_start<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some(y) = self
            .focus
            .placement
            .primary_position()
            .line(layout)
            .map(|line| line_y(&line))
        {
            self.maybe_extend(Cursor::from_point(layout, 0.0, y), extend)
        } else {
            *self
        }
    }

    #[must_use]
    pub fn line_end<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        if let Some(y) = self
            .focus
            .placement
            .primary_position()
            .line(layout)
            .map(|line| line_y(&line))
        {
            self.maybe_extend(Cursor::from_point(layout, f32::MAX, y), extend)
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
        line_delta: i32,
        extend: bool,
    ) -> Option<Self> {
        let line_index = self
            .focus
            .placement
            .primary_position()
            .line_index
            .saturating_add_signed(line_delta);
        let line = layout.get(line_index as usize)?;
        let y = line.metrics().baseline - line.metrics().ascent * 0.5;
        let h_pos = self.h_pos.unwrap_or(self.focus.offset);
        let new_focus = Cursor::from_point(layout, h_pos, y);
        let h_pos = Some(h_pos);
        Some(if extend {
            Self {
                anchor: self.anchor,
                focus: new_focus,
                focus_at_end: new_focus.at_end_of_line(),
                h_pos,
            }
        } else {
            Self {
                anchor: new_focus,
                focus: new_focus,
                focus_at_end: new_focus.at_end_of_line(),
                h_pos,
            }
        })
    }

    pub fn visual_focus<B: Brush>(&self, layout: &Layout<B>) -> Option<peniko::kurbo::Line> {
        visual_for_cursor(layout, Some(self.focus.placement.primary_position()))
    }

    pub fn visual_alternate_focus<B: Brush>(
        &self,
        layout: &Layout<B>,
    ) -> Option<peniko::kurbo::Line> {
        visual_for_cursor(layout, self.focus.placement.alternate_position())
    }

    pub fn visual_anchor<B: Brush>(&self, layout: &Layout<B>) -> Option<peniko::kurbo::Line> {
        self.anchor.path.visual_line(layout).map(|line| {
            let metrics = line.metrics();
            let line_min = (metrics.baseline - metrics.ascent - metrics.leading * 0.5) as f64;
            let line_max = line_min + metrics.line_height as f64;
            let line_x = self.anchor.offset as f64;
            peniko::kurbo::Line::new((line_x, line_min - 10.0), (line_x, line_max - 10.0))
        })
    }

    pub fn visual_regions<B: Brush>(&self, layout: &Layout<B>) -> Vec<Rect> {
        let mut rects = Vec::new();
        self.visual_regions_with(layout, |rect| rects.push(rect));
        rects
    }

    pub fn visual_regions_with<B: Brush>(&self, layout: &Layout<B>, mut f: impl FnMut(Rect)) {
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
        let line_start_ix = start.path.visual_line_index;
        let line_end_ix = end.path.visual_line_index;
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
