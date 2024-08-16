// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hit testing.

use peniko::kurbo::Rect;

use super::*;

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
    pub text_start: usize,
    /// End of the target cluster.
    pub text_end: usize,
    /// Insert point of the cursor (leading or trailing).
    pub insert_point: usize,
    /// `true` if the target cluster is in a right-to-left run.
    pub is_rtl: bool,
    /// `true` if the cursor was created from a point or position inside the layout
    pub is_inside: bool,
}

impl Cursor {
    /// Creates a new cursor from the specified layout and point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, mut x: f32, y: f32) -> Self {
        let mut result = Self {
            is_inside: x >= 0. && y >= 0.,
            ..Default::default()
        };
        let last_line = layout.data.lines.len().saturating_sub(1);
        for (line_index, line) in layout.lines().enumerate() {
            let line_metrics = line.metrics();
            if y > line_metrics.baseline + line_metrics.descent + line_metrics.leading * 0.5 {
                if line_index != last_line {
                    continue;
                }
                result.is_inside = false;
                x = f32::MAX;
            } else if y < 0. {
                x = 0.;
            }
            result.baseline = line_metrics.baseline;
            result.path.line_index = line_index;
            let mut last_edge = line_metrics.offset;
            for (run_index, run) in line.runs().enumerate() {
                result.path.run_index = run_index;
                for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                    let range = cluster.text_range();
                    result.text_start = range.start;
                    result.text_end = range.end;
                    result.is_rtl = run.is_rtl();
                    result.path.cluster_index = run.visual_to_logical(cluster_index).unwrap();
                    if x >= last_edge {
                        let advance = cluster.advance();
                        let next_edge = last_edge + advance;
                        result.offset = next_edge;
                        result.insert_point = range.end;
                        if x >= next_edge {
                            last_edge = next_edge;
                            continue;
                        }
                        result.advance = advance;
                        if x <= (last_edge + next_edge) * 0.5 {
                            result.insert_point = range.start;
                            result.offset = last_edge;
                        }
                    } else {
                        result.is_inside = false;
                        result.insert_point = range.start;
                        result.offset = line_metrics.offset;
                    }
                    return result;
                }
            }
            break;
        }
        result.is_inside = false;
        result
    }

    fn from_offset_on_line<B: Brush>(
        layout: &Layout<B>,
        mut x: f32,
        line_index: usize,
    ) -> Option<Self> {
        let mut result = Self {
            is_inside: x >= 0.0,
            ..Default::default()
        };
        let last_line = layout.data.lines.len().saturating_sub(1);
        let line = layout.get(line_index)?;
        let line_metrics = line.metrics();
        result.baseline = line_metrics.baseline;
        result.path.line_index = line_index;
        let mut last_edge = line_metrics.offset;
        for (run_index, run) in line.runs().enumerate() {
            result.path.run_index = run_index;
            for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                let range = cluster.text_range();
                result.text_start = range.start;
                result.text_end = range.end;
                result.is_rtl = run.is_rtl();
                result.path.cluster_index = run.visual_to_logical(cluster_index).unwrap();
                if x >= last_edge {
                    let advance = cluster.advance();
                    let next_edge = last_edge + advance;
                    result.offset = next_edge;
                    result.insert_point = range.end;
                    if x >= next_edge {
                        last_edge = next_edge;
                        continue;
                    }
                    result.advance = advance;
                    if x <= (last_edge + next_edge) * 0.5 {
                        result.insert_point = range.start;
                        result.offset = last_edge;
                    }
                } else {
                    result.is_inside = false;
                    result.insert_point = range.start;
                    result.offset = line_metrics.offset;
                }
                return Some(result);
            }
        }
        result.is_inside = false;
        Some(result)
    }

    /// Creates a new cursor for the specified layout and text position.
    pub fn from_position<B: Brush>(
        layout: &Layout<B>,
        mut position: usize,
        is_leading: bool,
    ) -> Self {
        let mut result = Self {
            is_inside: true,
            ..Default::default()
        };
        if position >= layout.data.text_len {
            result.is_inside = false;
            position = layout.data.text_len.saturating_sub(1);
        }
        let last_line = layout.data.lines.len().saturating_sub(1);
        for (line_index, line) in layout.lines().enumerate() {
            let line_metrics = line.metrics();
            result.baseline = line_metrics.baseline;
            result.path.line_index = line_index;
            if !line.text_range().contains(&position) && line_index != last_line {
                continue;
            }
            let mut last_edge = line_metrics.offset;
            result.offset = last_edge;
            for (run_index, run) in line.runs().enumerate() {
                result.path.run_index = run_index;
                if !run.text_range().contains(&position) {
                    last_edge += run.advance();
                    result.offset = last_edge;
                    continue;
                }
                for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                    let range = cluster.text_range();
                    result.text_start = range.start;
                    result.text_end = range.end;
                    result.offset = last_edge;
                    result.is_rtl = run.is_rtl();
                    result.path.cluster_index = run.visual_to_logical(cluster_index).unwrap();
                    let advance = cluster.advance();
                    if range.contains(&position) {
                        if !is_leading || !result.is_inside {
                            result.offset += advance;
                        }
                        result.insert_point = if is_leading { range.start } else { range.end };
                        result.advance = advance;
                        return result;
                    }
                    last_edge += advance;
                }
            }
            result.offset = last_edge;
            break;
        }
        result.insert_point = result.text_end;
        result.is_inside = false;
        result
    }

    /// Returns `true` if the cursor is on the leading edge of the target
    /// cluster.
    pub fn is_leading(&self) -> bool {
        self.text_start == self.insert_point
    }

    /// Returns `true` if the cursor is on the trailing edge of the target
    /// cluster.
    pub fn is_trailing(&self) -> bool {
        self.text_end == self.insert_point
    }

    fn insertion_index(&self) -> usize {
        if self.is_rtl {
            if self.is_leading() {
                self.text_end
            } else {
                self.text_start
            }
        } else {
            self.insert_point
        }
    }
}

/// Index based path to a cluster.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub struct CursorPath {
    /// Index of the containing line.
    pub line_index: usize,
    /// Index of the run within the containing line.
    pub run_index: usize,
    /// Index of the cluster within the containing run.
    pub cluster_index: usize,
}

impl CursorPath {
    /// Returns the line for this path and the specified layout.
    pub fn line<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Line<'a, B>> {
        layout.get(self.line_index)
    }

    /// Returns the run for this path and the specified layout.
    pub fn run<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Run<'a, B>> {
        self.line(layout)?.run(self.run_index)
    }

    /// Returns the cluster for this path and the specified layout.
    pub fn cluster<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Cluster<'a, B>> {
        self.run(layout)?.get(self.cluster_index)
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

    pub fn from_index<B: Brush>(layout: &Layout<B>, index: usize, is_leading: bool) -> Self {
        Cursor::from_position(layout, index, is_leading).into()
    }

    pub fn anchor(&self) -> &Cursor {
        &self.anchor
    }

    pub fn focus(&self) -> &Cursor {
        &self.focus
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor.insert_point == self.focus.insert_point
    }

    pub fn text_range(&self) -> Range<usize> {
        let start = self.anchor.insertion_index();
        let end = self.focus.insertion_index();
        if start < end {
            start..end
        } else {
            end..start
        }
    }

    /// Returns the index where text should be inserted based on this
    /// selection.
    pub fn insertion_index(&self) -> usize {
        self.focus.insert_point
    }

    pub fn collapse(&self) -> Self {
        Self {
            anchor: self.focus,
            focus: self.focus,
            h_pos: self.h_pos,
        }
    }

    pub fn extend_to_point<B: Brush>(&self, layout: &Layout<B>, x: f32, y: f32) -> Self {
        Self {
            anchor: self.anchor,
            focus: Cursor::from_point(layout, x, y),
            h_pos: None,
        }
    }

    pub fn next_logical<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        let new_focus = if self.focus.is_leading() && !self.focus.is_rtl {
            let mut new_focus = self.focus;
            new_focus.insert_point = new_focus.text_end;
            new_focus.offset += new_focus.advance;
            new_focus;
            Cursor::from_position(layout, self.focus.text_end, true)
        } else {
            Cursor::from_position(layout, self.focus.text_end + 1, true)
        };
        //let new_focus = Cursor::from_position(layout, self.focus.text_end + 1, true);
        if extend {
            Self {
                focus: new_focus,
                anchor: self.anchor,
                h_pos: None,
            }
        } else {
            new_focus.into()
        }
    }

    pub fn prev_logical<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        let new_focus = if self.focus.is_trailing() && !self.focus.is_rtl {
            let mut new_focus = self.focus;
            new_focus.insert_point = new_focus.text_start;
            new_focus.offset -= new_focus.advance;
            new_focus;
            Cursor::from_position(layout, self.focus.text_start, true)
        } else {
            Cursor::from_position(layout, self.focus.text_start.saturating_sub(1), true)
        };
        if extend {
            Self {
                focus: new_focus,
                anchor: self.anchor,
                h_pos: None,
            }
        } else {
            new_focus.into()
        }
    }

    pub fn next_line<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.move_line(layout, 1, extend).unwrap_or(*self)
    }

    pub fn prev_line<B: Brush>(&self, layout: &Layout<B>, extend: bool) -> Self {
        self.move_line(layout, -1, extend).unwrap_or(*self)
    }

    fn move_line<B: Brush>(
        &self,
        layout: &Layout<B>,
        line_delta: isize,
        extend: bool,
    ) -> Option<Self> {
        let line_index = self.focus.path.line_index.saturating_add_signed(line_delta);
        let new_focus = Cursor::from_offset_on_line(
            layout,
            self.h_pos.unwrap_or(self.focus.offset),
            line_index,
        )?;
        let h_pos = Some(self.h_pos.unwrap_or(new_focus.offset));
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

    pub fn visual_caret<B: Brush>(&self, layout: &Layout<B>) -> Option<peniko::kurbo::Line> {
        self.focus.path.line(layout).map(|line| {
            let metrics = line.metrics();
            let line_min = (metrics.baseline - metrics.ascent - metrics.leading * 0.5) as f64;
            let line_max = line_min + metrics.line_height as f64;
            let line_x = self.focus.offset as f64;
            peniko::kurbo::Line::new((line_x, line_min), (line_x, line_max))
        })
    }

    pub fn visual_anchor<B: Brush>(&self, layout: &Layout<B>) -> Option<peniko::kurbo::Line> {
        self.anchor.path.line(layout).map(|line| {
            let metrics = line.metrics();
            let line_min = (metrics.baseline - metrics.ascent - metrics.leading * 0.5) as f64;
            let line_max = line_min + metrics.line_height as f64;
            let line_x = self.anchor.offset as f64;
            peniko::kurbo::Line::new((line_x, line_min - 10.0), (line_x, line_max - 10.0))
        })
    }

    pub fn visual_regions<B: Brush>(&self, layout: &Layout<B>) -> Vec<Rect> {
        let mut rects = vec![];
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
        if start.insertion_index() > end.insertion_index() {
            core::mem::swap(&mut start, &mut end);
        }
        let text_range = start.insertion_index()..end.insertion_index();
        let line_start_ix = start.path.line_index;
        let line_end_ix = end.path.line_index;
        for line_ix in line_start_ix..=line_end_ix {
            let Some(line) = layout.get(line_ix) else {
                continue;
            };
            let metrics = line.metrics();
            let line_min = (metrics.baseline - metrics.ascent - metrics.leading * 0.5) as f64;
            let line_max = line_min + metrics.line_height as f64;
            if line_ix == line_start_ix || line_ix == line_end_ix {
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
            } else {
                let x = metrics.offset as f64;
                let width = (metrics.advance as f64).max(MIN_RECT_WIDTH);
                f(Rect::new(x, line_min, x + width, line_max));
            }
        }
    }
}
