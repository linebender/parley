// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Greedy line breaking.

use alloc::vec::Vec;

#[cfg(feature = "libm")]
#[allow(unused_imports)]
use core_maths::CoreFloat;

use crate::OverflowWrap;
use crate::analysis::Boundary;
use crate::analysis::cluster::Whitespace;
use crate::layout::{
    BreakReason, Layout, LayoutData, LayoutItem, LayoutItemKind, LineData, LineItemData,
    LineMetrics, Run,
};
use crate::style::Brush;

use core::ops::Range;

#[derive(Default)]
struct LineLayout {
    lines: Vec<LineData>,
    line_items: Vec<LineItemData>,
}

impl LineLayout {
    fn swap<B: Brush>(&mut self, layout: &mut LayoutData<B>) {
        core::mem::swap(&mut self.lines, &mut layout.lines);
        core::mem::swap(&mut self.line_items, &mut layout.line_items);
    }
}

#[derive(Clone, Default)]
struct LineState {
    x: f32,
    items: Range<usize>,
    clusters: Range<usize>,
    num_spaces: usize,
    /// Of the line currently being built, the maximum line height seen so far.
    /// This represents a lower-bound on the eventual line height of the line.
    running_line_height: f32,
}

#[derive(Clone, Default)]
struct PrevBoundaryState {
    item_idx: usize,
    run_idx: usize,
    cluster_idx: usize,
    state: LineState,
}

#[derive(Clone, Default)]
struct BreakerState {
    /// The number of items that have been processed (used to revert state)
    items: usize,
    /// The number of lines that have been processed (used to revert state)
    lines: usize,

    /// Iteration state: the current item (within the layout)
    item_idx: usize,
    /// Iteration state: the current run (within the layout)
    run_idx: usize,
    /// Iteration state: the current cluster (within the layout)
    cluster_idx: usize,

    /// The y coordinate of the bottom of the last committed line (or else 0)
    /// Use of f64 here is important. f32 causes test failures due to accumulated error
    committed_y: f64,

    line: LineState,
    prev_boundary: Option<PrevBoundaryState>,
    emergency_boundary: Option<PrevBoundaryState>,
}

impl BreakerState {
    /// Add the cluster(s) currently being evaluated to the current line
    fn append_cluster_to_line(&mut self, next_x: f32, clusters_height: f32) {
        self.line.items.end = self.item_idx + 1;
        self.line.clusters.end = self.cluster_idx + 1;
        self.line.x = next_x;
        self.add_line_height(clusters_height);
        // Would like to add:
        // self.cluster_idx += 1;
    }

    /// Add inline box to line
    fn append_inline_box_to_line(&mut self, next_x: f32, box_height: f32) {
        // self.item_idx += 1;
        self.line.items.end += 1;
        self.line.x = next_x;
        self.add_line_height(box_height);
        // Would like to add:
        // self.item_idx += 1;
    }

    /// Store the current iteration state so that we can revert to it if we later want to take
    /// the line breaking opportunity at this point.
    fn mark_line_break_opportunity(&mut self) {
        self.prev_boundary = Some(PrevBoundaryState {
            item_idx: self.item_idx,
            run_idx: self.run_idx,
            cluster_idx: self.cluster_idx,
            state: self.line.clone(),
        });
    }

    /// Store the current iteration state so that we can revert to it if we later want to take
    /// an *emergency* line breaking opportunity at this point.
    fn mark_emergency_break_opportunity(&mut self) {
        self.emergency_boundary = Some(PrevBoundaryState {
            item_idx: self.item_idx,
            run_idx: self.run_idx,
            cluster_idx: self.cluster_idx,
            state: self.line.clone(),
        });
    }

    #[inline(always)]
    fn add_line_height(&mut self, height: f32) {
        self.line.running_line_height = self.line.running_line_height.max(height);
    }
}

/// Line breaking support for a paragraph.
pub struct BreakLines<'a, B: Brush> {
    layout: &'a mut Layout<B>,
    lines: LineLayout,
    state: BreakerState,
    prev_state: Option<BreakerState>,
    done: bool,
}

impl<'a, B: Brush> BreakLines<'a, B> {
    pub(crate) fn new(layout: &'a mut Layout<B>) -> Self {
        layout.data.width = 0.;
        layout.data.height = 0.;
        let mut lines = LineLayout::default();
        lines.swap(&mut layout.data);
        lines.lines.clear();
        lines.line_items.clear();
        Self {
            layout,
            lines,
            state: BreakerState::default(),
            prev_state: None,
            done: false,
        }
    }

    /// Reset state when a line has been committed
    fn start_new_line(&mut self) -> Option<(f32, f32)> {
        let line_height = self.state.line.running_line_height;

        self.state.items = self.lines.line_items.len();
        self.state.lines = self.lines.lines.len();
        self.state.line.x = 0.;
        self.state.line.running_line_height = 0.;
        self.state.prev_boundary = None; // Added by Nico
        self.state.emergency_boundary = None;

        self.finish_line(self.lines.lines.len() - 1, line_height);
        self.last_line_data()
    }

    fn last_line_data(&self) -> Option<(f32, f32)> {
        let line = self.lines.lines.last().unwrap();
        Some((line.metrics.advance, line.size()))
    }

    /// Returns the y-coordinate of the top of the current line
    pub fn committed_y(&self) -> f64 {
        self.state.committed_y
    }

    /// Returns true if all the text has been placed into lines.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Computes the next line in the paragraph. Returns the advance and size
    /// (width and height for horizontal layouts) of the line.
    pub fn break_next(&mut self, max_advance: f32) -> Option<(f32, f32)> {
        // Maintain iterator state
        if self.done {
            return None;
        }
        self.prev_state = Some(self.state.clone());

        // HACK: ignore max_advance for empty layouts
        // Prevents crash when width is too small (https://github.com/linebender/parley/issues/186)
        let max_advance =
            if self.layout.data.text_len == 0 && self.layout.data.inline_boxes.is_empty() {
                f32::MAX
            } else {
                max_advance
            };

        // This macro simply calls the `commit_line` with the provided arguments and some parts of self.
        // It exists solely to cut down on the boilerplate for accessing the self variables while
        // keeping the borrow checker happy
        macro_rules! try_commit_line {
            ($break_reason:expr) => {
                try_commit_line(
                    self.layout,
                    &mut self.lines,
                    &mut self.state.line,
                    max_advance,
                    $break_reason,
                )
            };
        }

        // dbg!(&self.layout.items);

        // println!("\nBREAK NEXT");
        // dbg!(&self.state.line.items);

        // Iterate over remaining runs in the Layout
        let item_count = self.layout.data.items.len();
        while self.state.item_idx < item_count {
            let item = &self.layout.data.items[self.state.item_idx];

            // println!(
            //     "\nitem = {} {:?}. x: {}",
            //     self.state.item_idx, item.kind, self.state.line.x
            // );
            // dbg!(&self.state.line.items);

            match item.kind {
                LayoutItemKind::InlineBox => {
                    let inline_box = &self.layout.data.inline_boxes[item.index];

                    // Compute the x position of the content being currently processed
                    let next_x = self.state.line.x + inline_box.width;

                    // println!("BOX next_x: {}", next_x);

                    // If the box fits on the current line (or we are at the start of the current line)
                    // then simply move on to the next item
                    if next_x <= max_advance {
                        // println!("BOX FITS");

                        self.state.item_idx += 1;

                        self.state
                            .append_inline_box_to_line(next_x, inline_box.height);

                        // We can always line break after an inline box
                        self.state.mark_line_break_opportunity();
                    } else {
                        // If we're at the start of the line, this box will never fit, so consume it and accept the overflow.
                        if self.state.line.x == 0.0 {
                            // println!("BOX EMERGENCY BREAK");
                            self.state
                                .append_inline_box_to_line(next_x, inline_box.height);
                            if try_commit_line!(BreakReason::Emergency) {
                                self.state.item_idx += 1;
                                return self.start_new_line();
                            }
                        } else {
                            // println!("BOX BREAK");
                            if try_commit_line!(BreakReason::Regular) {
                                return self.start_new_line();
                            }
                        }
                    }
                }
                LayoutItemKind::TextRun => {
                    let run_idx = item.index;
                    let run_data = &self.layout.data.runs[run_idx];

                    let run = Run::new(self.layout, 0, 0, run_data, None);
                    let cluster_start = run_data.cluster_range.start;
                    let cluster_end = run_data.cluster_range.end;

                    // println!("TextRun ({:?})", &run_data.text_range);

                    // Iterate over remaining clusters in the Run
                    while self.state.cluster_idx < cluster_end {
                        let cluster = run.get(self.state.cluster_idx - cluster_start).unwrap();

                        // Retrieve metadata about the cluster
                        let is_ligature_continuation = cluster.is_ligature_continuation();
                        let whitespace = cluster.info().whitespace();
                        let is_newline = whitespace == Whitespace::Newline;
                        let is_space = whitespace.is_space_or_nbsp();
                        let boundary = cluster.info().boundary();
                        let style = &self.layout.data.styles[cluster.data.style_index as usize];

                        if boundary == Boundary::Line {
                            // We do not currently handle breaking within a ligature, so we ignore boundaries in such a position.
                            //
                            // We also don't record boundaries when the advance is 0. As we do not want overflowing content to cause extra consecutive
                            // line breaks. We should accept the overflowing fragment in that scenario.
                            if !is_ligature_continuation && self.state.line.x != 0.0 {
                                self.state.mark_line_break_opportunity();
                                // break_opportunity = true;
                            }
                        } else if is_newline {
                            self.state.append_cluster_to_line(
                                self.state.line.x,
                                run.metrics().line_height,
                            );
                            if try_commit_line!(BreakReason::Explicit) {
                                // TODO: can this be hoisted out of the conditional?
                                self.state.cluster_idx += 1;
                                return self.start_new_line();
                            }
                        } else if
                        // This text can contribute "emergency" line breaks.
                        style.overflow_wrap != OverflowWrap::Normal && !is_ligature_continuation
                        // If we're at the start of the line, this particular cluster will never fit, so it's not a valid emergency break opportunity.
                        && self.state.line.x != 0.0
                        {
                            self.state.mark_emergency_break_opportunity();
                        }

                        // If current cluster is the start of a ligature, then advance state to include
                        // the remaining clusters that make up the ligature
                        let mut advance = cluster.advance();
                        if cluster.is_ligature_start() {
                            while let Some(cluster) = run.get(self.state.cluster_idx + 1) {
                                if !cluster.is_ligature_continuation() {
                                    break;
                                } else {
                                    advance += cluster.advance();
                                    self.state.cluster_idx += 1;
                                }
                            }
                        }

                        // Compute the x position of the content being currently processed
                        let next_x = self.state.line.x + advance;

                        // println!("Cluster {} next_x: {}", self.state.cluster_idx, next_x);

                        // if break_opportunity {
                        //     println!("===");
                        // }

                        // If that x position does NOT exceed max_advance then we simply add the cluster(s) to the current line
                        if next_x <= max_advance {
                            let line_height = run.metrics().line_height;
                            self.state.append_cluster_to_line(next_x, line_height);
                            self.state.cluster_idx += 1;
                            if is_space {
                                self.state.line.num_spaces += 1;
                            }
                        }
                        // Else we line break:
                        else {
                            // Handle case where cluster is space character. Hang overflowing whitespace.
                            if is_space {
                                let line_height = run.metrics().line_height;
                                self.state.append_cluster_to_line(next_x, line_height);
                                if try_commit_line!(BreakReason::Regular) {
                                    // TODO: can this be hoisted out of the conditional?
                                    self.state.cluster_idx += 1;
                                    return self.start_new_line();
                                }
                            }
                            // Handle the (common) case where we have previously encountered a line-breaking opportunity in the current line
                            //
                            // We "take" the line-breaking opportunity by starting a new line and resetting our
                            // item/run/cluster iteration state back to how it was when the line-breaking opportunity was encountered
                            else if let Some(prev) = self.state.prev_boundary.take() {
                                // println!("REVERT");
                                // debug_assert!(prev.state.x != 0.0);

                                // Q: Why do we revert the line state here, but only revert the indexes if the commit succeeds?
                                self.state.line = prev.state;
                                if try_commit_line!(BreakReason::Regular) {
                                    // Revert boundary state to prev state
                                    self.state.item_idx = prev.item_idx;
                                    self.state.run_idx = prev.run_idx;
                                    self.state.cluster_idx = prev.cluster_idx;

                                    return self.start_new_line();
                                }
                            }
                            // Otherwise perform an emergency line break
                            else if let Some(prev_emergency) =
                                self.state.emergency_boundary.take()
                            {
                                self.state.line = prev_emergency.state;
                                if try_commit_line!(BreakReason::Emergency) {
                                    // Revert boundary state to prev state
                                    self.state.item_idx = prev_emergency.item_idx;
                                    self.state.run_idx = prev_emergency.run_idx;
                                    self.state.cluster_idx = prev_emergency.cluster_idx;

                                    return self.start_new_line();
                                }
                            } else {
                                let line_height = run.metrics().line_height;
                                self.state.append_cluster_to_line(next_x, line_height);
                                self.state.cluster_idx += 1;
                            }
                        }
                    }
                    self.state.run_idx += 1;
                    self.state.item_idx += 1;
                }
            }
        }

        if self.state.line.items.end == 0 {
            self.state.line.items.end = 1;
        }
        if try_commit_line!(BreakReason::None) {
            self.done = true;
            return self.start_new_line();
        }

        None
    }

    /// Reverts the last computed line, returning to the previous state.
    pub fn revert(&mut self) -> bool {
        if let Some(state) = self.prev_state.take() {
            self.state = state;
            self.lines.lines.truncate(self.state.lines);
            self.lines.line_items.truncate(self.state.items);
            self.done = false;
            true
        } else {
            false
        }
    }

    /// Breaks all remaining lines with the specified maximum advance. This
    /// consumes the line breaker.
    pub fn break_remaining(mut self, max_advance: f32) {
        // println!("\nDEBUG ITEMS");
        // for item in &self.layout.items {
        //     match item.kind {
        //         LayoutItemKind::InlineBox => println!("{:?}", item.kind),
        //         LayoutItemKind::TextRun => {
        //             let run_data = &self.layout.runs[item.index];
        //             println!("{:?} ({:?})", item.kind, &run_data.text_range);
        //         }
        //     }
        // }

        // println!("\nBREAK ALL");

        while self.break_next(max_advance).is_some() {}
        self.finish();
    }

    /// Consumes the line breaker and finalizes all line computations.
    pub fn finish(mut self) {
        if self.layout.data.text_len == 0 {
            if let Some(line) = self.lines.line_items.first_mut() {
                line.text_range = 0..0;
                line.cluster_range = 0..0;
            }
        }
    }

    fn finish_line(&mut self, line_idx: usize, line_height: f32) {
        let prev_line_metrics = match line_idx {
            0 => None,
            idx => Some(self.lines.lines[idx - 1].metrics),
        };
        let line = &mut self.lines.lines[line_idx];

        // Reset metrics for line
        line.metrics.ascent = 0.;
        line.metrics.descent = 0.;
        line.metrics.leading = 0.;
        line.metrics.offset = 0.;
        line.text_range.start = usize::MAX;

        line.metrics.line_height = line_height;

        if line.item_range.is_empty() {
            line.text_range = self.layout.data.text_len..self.layout.data.text_len;
        }
        // Compute metrics for the line, but ignore trailing whitespace.
        let mut have_metrics = false;
        let mut needs_reorder = false;
        for line_item in self.lines.line_items[line.item_range.clone()]
            .iter_mut()
            .rev()
        {
            match line_item.kind {
                LayoutItemKind::InlineBox => {
                    let item = &self.layout.data.inline_boxes[line_item.index];

                    // Advance is already computed in "commit line" for items

                    // Default vertical alignment is to align the bottom of boxes with the text baseline.
                    // This is equivalent to the entire height of the box being "ascent"
                    line.metrics.ascent = line.metrics.ascent.max(item.height);

                    // Mark us as having seen non-whitespace content on this line
                    have_metrics = true;
                }
                LayoutItemKind::TextRun => {
                    line_item.compute_whitespace_properties(&self.layout.data);

                    // Compute the text range for the line
                    // Q: Can we not simplify this computation by assuming that items are in order?
                    line.text_range.end = line.text_range.end.max(line_item.text_range.end);
                    line.text_range.start = line.text_range.start.min(line_item.text_range.start);

                    // Mark line as needing bidi re-ordering if it contains any runs with non-zero bidi level
                    // (zero is the default level, so this is equivalent to marking lines that have multiple levels)
                    if line_item.bidi_level != 0 {
                        needs_reorder = true;
                    }

                    // Compute the run's advance by summing the advances of its constituent clusters
                    line_item.advance = self.layout.data.clusters[line_item.cluster_range.clone()]
                        .iter()
                        .map(|c| c.advance)
                        .sum();

                    // Ignore trailing whitespace for metrics computation
                    // (we are iterating backwards so trailing whitespace comes first)
                    if !have_metrics && line_item.is_whitespace {
                        continue;
                    }

                    // Compute the run's vertical metrics
                    let run = &self.layout.data.runs[line_item.index];
                    line.metrics.ascent = line.metrics.ascent.max(run.metrics.ascent);
                    line.metrics.descent = line.metrics.descent.max(run.metrics.descent);

                    // Mark us as having seen non-whitespace content on this line
                    have_metrics = true;
                }
            }
        }

        // Reorder the items within the line (if required). Reordering is required if the line contains
        // a mix of bidi levels (a mix of LTR and RTL text)
        let item_count = line.item_range.end - line.item_range.start;
        if needs_reorder && item_count > 1 {
            reorder_line_items(&mut self.lines.line_items[line.item_range.clone()]);
        }

        // Compute size of line's trailing whitespace. "Trailing" is considered the right edge
        // for LTR text and the left edge for RTL text.
        let run = if self.layout.is_rtl() {
            self.lines.line_items[line.item_range.clone()].first()
        } else {
            self.lines.line_items[line.item_range.clone()].last()
        };
        line.metrics.trailing_whitespace = run
            .filter(|item| item.is_text_run())
            .and_then(|run| {
                let cluster = if self.layout.is_rtl() {
                    self.layout.data.clusters[run.cluster_range.clone()].first()
                } else {
                    self.layout.data.clusters[run.cluster_range.clone()].last()
                };
                cluster
                    .filter(|cluster| cluster.info.whitespace().is_space_or_nbsp())
                    .map(|cluster| cluster.advance)
            })
            .unwrap_or(0.0);

        if !have_metrics {
            // Line consisting entirely of whitespace?
            if !line.item_range.is_empty() {
                let line_item = &self.lines.line_items[line.item_range.start];
                if line_item.is_text_run() {
                    let run = &self.layout.data.runs[line_item.index];
                    line.metrics.ascent = run.metrics.ascent;
                    line.metrics.descent = run.metrics.descent;
                }
            } else if let Some(metrics) = prev_line_metrics {
                // HACK: copy metrics from previous line if we don't have
                // any; this should only occur for an empty line following
                // a newline at the end of a layout
                line.metrics = metrics;
                // If we have no items on this line, it must be the last (empty)
                // line in a layout following a newline. Commit an empty run so
                // that AccessKit has a node with which to identify the visual
                // cursor position
                if let Some((index, run)) = self
                    .layout
                    .data
                    .runs
                    .iter()
                    .enumerate()
                    .rfind(|(_, run)| !run.text_range.is_empty())
                {
                    let run_index = self.lines.line_items.len();
                    let cluster = run.cluster_range.end;
                    let text = run.text_range.end;
                    self.lines.line_items.push(LineItemData {
                        kind: LayoutItemKind::TextRun,
                        index,
                        bidi_level: 0,
                        advance: 0.,
                        is_whitespace: false,
                        has_trailing_whitespace: false,
                        cluster_range: cluster..cluster,
                        text_range: text..text,
                    });
                    line.item_range = run_index..run_index + 1;
                }
            }
        }

        line.metrics.leading =
            line.metrics.line_height - (line.metrics.ascent + line.metrics.descent);

        // Whether metrics should be quantized to pixel boundaries
        let quantize = self.layout.data.quantize;

        let (ascent, descent) = if quantize {
            // We mimic Chrome in rounding ascent and descent separately,
            // before calculating the rest.
            // See lines_integral_line_height_ascent_descent_rounding() for more details.
            (line.metrics.ascent.round(), line.metrics.descent.round())
        } else {
            (line.metrics.ascent, line.metrics.descent)
        };

        let (leading_above, leading_below) = if quantize {
            // Calculate leading using the rounded ascent and descent.
            let leading = line.metrics.line_height - (ascent + descent);
            // We mimic Chrome in giving 'below' the larger leading half.
            // Although the comment in Chromium's NGLineHeightMetrics::AddLeading function
            // in ng_line_height_metrics.cc claims it's for legacy test compatibility.
            // So we might want to think about giving 'above' the larger half instead.
            let above = (leading * 0.5).floor();
            let below = leading.round() - above;
            (above, below)
        } else {
            (line.metrics.leading * 0.5, line.metrics.leading * 0.5)
        };

        let y = self.state.committed_y;
        line.metrics.baseline =
            ascent + leading_above + if quantize { y.round() as f32 } else { y as f32 };

        // Small line heights will cause leading to be negative.
        // Negative leadings are correct for baseline calculation, but not for min/max coords.
        // We clamp leading to zero for the purposes of min/max coords,
        // which in turn clamps the selection box minimum height to ascent + descent.
        line.metrics.min_coord = line.metrics.baseline - ascent - leading_above.max(0.);
        line.metrics.max_coord = line.metrics.baseline + descent + leading_below.max(0.);

        self.state.committed_y += line.metrics.line_height as f64;
    }
}

impl<B: Brush> Drop for BreakLines<'_, B> {
    fn drop(&mut self) {
        // Compute the overall width and height of the entire layout
        // The "width" excludes trailing whitespace. The "full_width" includes it.
        let mut width = 0_f32;
        let mut full_width = 0_f32;
        let mut height = 0_f64; // f32 causes test failures due to accumulated error
        for line in &self.lines.lines {
            width = width.max(line.metrics.advance - line.metrics.trailing_whitespace);
            full_width = full_width.max(line.metrics.advance);
            height += line.metrics.line_height as f64;
        }

        // Save the computed widths/height to the layout
        self.layout.data.width = width;
        self.layout.data.full_width = full_width;
        self.layout.data.height = height as f32;

        // for (i, line) in self.lines.lines.iter().enumerate() {
        //     println!("LINE {i}");
        //     for item_idx in line.item_range.clone() {
        //         let item = &self.lines.line_items[item_idx];
        //         println!("  ITEM {:?} ({})", item.kind, item.advance);
        //     }
        // }

        // Save the computed lines to the layout
        self.lines.swap(&mut self.layout.data);
    }
}

// fn cluster_range_is_valid(
//     mut cluster_range: Range<usize>,
//     state_cluster_range: Range<usize>,
//     is_first: bool,
//     is_last: bool,
//     is_empty: bool,
// ) -> bool {
//     // Compute cluster range
//     if is_first {
//         cluster_range.start = state_cluster_range.start;
//     }
//     if is_last {
//         cluster_range.end = state_cluster_range.end;
//     }

//     // Return true if cluster is valid. Else false.
//     cluster_range.start < cluster_range.end
//         || (cluster_range.start == cluster_range.end && is_empty)
// }

// fn should_commit_line<B: Brush>(
//     layout: &LayoutData<B>,
//     state: &mut LineState,
//     is_last: bool,
// ) -> bool {
//     // Compute end cluster
//     state.clusters.end = state.clusters.end.min(layout.clusters.len());
//     if state.runs.end == 0 && is_last {
//         state.runs.end = 1;
//     }

//     let last_run = state.runs.len() - 1;
//     let is_empty = layout.text_len == 0;

//     // Iterate over runs. Checking if any have a valid cluster range.
//     let runs = &layout.runs[state.runs.clone()];
//     runs.iter().enumerate().any(|(i, run_data)| {
//         cluster_range_is_valid(
//             run_data.cluster_range.clone(),
//             state.clusters.clone(),
//             i == 0,
//             i == last_run,
//             is_empty,
//         )
//     })
// }

fn try_commit_line<B: Brush>(
    layout: &Layout<B>,
    lines: &mut LineLayout,
    state: &mut LineState,
    max_advance: f32,
    break_reason: BreakReason,
) -> bool {
    // Ensure that the cluster and item endpoints are within range
    state.clusters.end = state.clusters.end.min(layout.data.clusters.len());
    state.items.end = state.items.end.min(layout.data.items.len());

    let start_item_idx = lines.line_items.len();
    // let start_run_idx = lines.line_items.last().map(|item| item.index).unwrap_or(0);

    let items_to_commit = &layout.data.items[state.items.clone()];

    // Compute first and last run index
    let is_text_run = |item: &LayoutItem| item.kind == LayoutItemKind::TextRun;
    let first_run_pos = items_to_commit.iter().position(is_text_run).unwrap_or(0);
    let last_run_pos = items_to_commit.iter().rposition(is_text_run).unwrap_or(0);

    // // Return if line contains no runs
    // let (Some(first_run_pos), Some(last_run_pos)) = (first_run_pos, last_run_pos) else {
    //     return false;
    // };

    //let runs = &layout.runs[state.runs.clone()];
    // let start_run_idx = items_to_commit[first_run_pos].index;
    // let end_run_idx = items_to_commit[last_run_pos].index;

    // Iterate over the items to commit
    // println!("\nCOMMIT LINE");
    let mut last_item_kind = LayoutItemKind::TextRun;
    let mut committed_text_run = false;
    for (i, item) in items_to_commit.iter().enumerate() {
        // println!("i = {} index = {} {:?}", i, item.index, item.kind);

        match item.kind {
            LayoutItemKind::InlineBox => {
                let inline_box = &layout.data.inline_boxes[item.index];

                lines.line_items.push(LineItemData {
                    kind: LayoutItemKind::InlineBox,
                    index: item.index,
                    bidi_level: item.bidi_level,
                    advance: inline_box.width,

                    // These properties are ignored for inline boxes. So we just put a dummy value.
                    is_whitespace: false,
                    has_trailing_whitespace: false,
                    cluster_range: 0..0,
                    text_range: 0..0,
                });

                last_item_kind = item.kind;
            }
            LayoutItemKind::TextRun => {
                let run_data = &layout.data.runs[item.index];

                // Compute cluster range
                // The first and last ranges have overrides to account for line-breaks within runs
                let mut cluster_range = run_data.cluster_range.clone();
                if i == first_run_pos {
                    cluster_range.start = state.clusters.start;
                }
                if i == last_run_pos {
                    cluster_range.end = state.clusters.end;
                }

                if cluster_range.start >= run_data.cluster_range.end {
                    // println!("INVALID CLUSTER");
                    // dbg!(&run_data.text_range);
                    // dbg!(cluster_range);
                    continue;
                }

                last_item_kind = item.kind;
                committed_text_run = true;

                // Push run to line
                let run = Run::new(layout, 0, 0, run_data, None);
                let text_range = if run_data.cluster_range.is_empty() {
                    0..0
                } else {
                    let first_cluster = run
                        .get(cluster_range.start - run_data.cluster_range.start)
                        .unwrap();
                    let last_cluster = run
                        .get((cluster_range.end - run_data.cluster_range.start).saturating_sub(1))
                        .unwrap();
                    first_cluster.text_range().start..last_cluster.text_range().end
                };

                lines.line_items.push(LineItemData {
                    kind: LayoutItemKind::TextRun,
                    index: item.index,
                    bidi_level: run_data.bidi_level,
                    advance: 0.,
                    is_whitespace: false,
                    has_trailing_whitespace: false,
                    cluster_range,
                    text_range,
                });
            }
        }
    }
    // let end_run_idx = lines.line_items.last().map(|item| item.index).unwrap_or(0);
    let end_item_idx = lines.line_items.len();

    // Return false and don't commit line if there were no items to process
    // FIXME: support lines with only inlines boxes
    // if start_item_idx == end_item_idx {
    //     // } || first_run_pos == last_run_pos {
    //     return false;
    // }

    // Q: why this special case?
    let mut num_spaces = state.num_spaces;
    if break_reason == BreakReason::Regular {
        num_spaces = num_spaces.saturating_sub(1);
    }

    lines.lines.push(LineData {
        item_range: start_item_idx..end_item_idx,
        max_advance,
        break_reason,
        num_spaces,
        metrics: LineMetrics {
            advance: state.x,
            ..Default::default()
        },
        ..Default::default()
    });

    // Reset state for the new line
    state.num_spaces = 0;
    if committed_text_run {
        state.clusters.start = state.clusters.end;
    }

    state.items.start = match last_item_kind {
        // For text runs, the first item of line N+1 needs to be the SAME as
        // the last item for line N. This is because the item (if it a text run
        // may be split across the two lines with some clusters in line N and some
        // in line N+1). The item is later filtered out (see `continue` in loop above)
        // if there are not actually any clusters in line N+1.
        LayoutItemKind::TextRun => state.items.end.saturating_sub(1),
        // Inline boxes cannot be spread across multiple lines, so we should set
        // the first item of line N+1 to be the item AFTER the last item in line N.
        LayoutItemKind::InlineBox => state.items.end,
    };

    true
}

/// Reorder items within line according to the bidi levels of the items
fn reorder_line_items(runs: &mut [LineItemData]) {
    let run_count = runs.len();

    // Find the max level and the min *odd* level
    let mut max_level = 0;
    let mut lowest_odd_level = 255;
    for run in runs.iter() {
        let level = run.bidi_level;
        let is_odd = level & 1 != 0;

        // Update max level
        if level > max_level {
            max_level = level;
        }

        // Update min odd level
        if is_odd && level < lowest_odd_level {
            lowest_odd_level = level;
        }
    }

    // Iterate over bidi levels
    for level in (lowest_odd_level..=max_level).rev() {
        // Iterate over text runs
        let mut i = 0;
        while i < run_count {
            if runs[i].bidi_level >= level {
                let mut end = i + 1;
                while end < run_count && runs[end].bidi_level >= level {
                    end += 1;
                }

                let mut j = i;
                let mut k = end - 1;
                while j < k {
                    runs.swap(j, k);
                    j += 1;
                    k -= 1;
                }

                i = end;
            }
            i += 1;
        }
    }
}
