// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Greedy line breaking.

use alloc::vec::Vec;

#[cfg(feature = "libm")]
#[allow(unused_imports)]
use core_maths::CoreFloat;

use crate::analysis::cluster::Whitespace;
use crate::data::ClusterData;
use crate::layout::{
    BreakReason, Layout, LayoutData, LayoutItem, LayoutItemKind, LineData, LineItemData,
    LineMetrics, Run, RunMetrics,
};
use crate::style::{Brush, EndOfLineWhitespace, WhiteSpaceCollapse};
use crate::{InlineBoxKind, OverflowWrap, TextWrapMode};

use core::ops::Range;
use parley_core::Boundary;

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

#[derive(Clone)]
struct LineState {
    x: f32,
    items: Range<usize>,
    clusters: Range<usize>,
    num_spaces: usize,
    /// Of the text on the line currently being built, the maximum ascent (distance above the
    /// baseline) of any run seen so far. This is the raw font ascent and does *not* include any
    /// leading.
    text_ascent: f32,
    /// Of the text on the line currently being built, the maximum descent (distance below the
    /// baseline) of any run seen so far. This is the raw font descent and does *not* include any
    /// leading.
    text_descent: f32,
    /// Of the text on the line currently being built, the maximum intrinsic line height (i.e.
    /// including the full leading) of any run seen so far. This may be *smaller* than
    /// `text_ascent + text_descent` when the line height is smaller than the font metrics (negative
    /// leading), in which case lines are allowed to overlap.
    text_line_height: f32,
    /// Of the inline boxes on the line currently being built, the maximum ascent (distance above
    /// the baseline) of any box seen so far.
    ///
    /// Box extents are tracked separately from text so that an inline box only grows the line when
    /// it extends beyond the text (including any positive half-leading). Crucially, this means a
    /// box's descent never interacts with negative leading: a box that sits on or above the
    /// baseline cannot "fill in" the negative leading that allows text lines to overlap.
    box_ascent: f32,
    /// Of the inline boxes on the line currently being built, the maximum descent (distance below
    /// the baseline) of any box seen so far. See [`Self::box_ascent`].
    box_descent: f32,
    /// This is set to true if we encounter something on the line (either a glyph or an inline box)
    /// that is taller than the `line_max_height`. When in this state `break_next` should yield control
    /// flow to the caller to handle the constraint violation.
    ///
    /// This never happens when calling `break_all_lines` as it never sets `line_max_height`, and it defaults to `f32::MAX`.
    max_height_exceeded: bool,

    /// We lag the text-wrap-mode by one cluster due to line-breaking boundaries only
    /// being triggered on the cluster after the linebreak.
    text_wrap_mode: TextWrapMode,
}

impl Default for LineState {
    fn default() -> Self {
        Self {
            x: 0.0,
            items: 0..0,
            clusters: 0..0,
            num_spaces: 0,
            // Text extents default to zero, so a line with no text contributes no ascent, descent
            // or leading.
            text_ascent: 0.0,
            text_descent: 0.0,
            text_line_height: 0.0,
            // Box extents default to negative infinity so that a line with no inline boxes never
            // grows the line via the box-overflow terms in `line_height`.
            box_ascent: f32::NEG_INFINITY,
            box_descent: f32::NEG_INFINITY,
            max_height_exceeded: false,
            text_wrap_mode: TextWrapMode::default(),
        }
    }
}

impl LineState {
    /// Reset the per-line running state in preparation for building a new line.
    fn reset(&mut self) {
        self.x = 0.0;
        self.text_ascent = 0.0;
        self.text_descent = 0.0;
        self.text_line_height = 0.0;
        self.box_ascent = f32::NEG_INFINITY;
        self.box_descent = f32::NEG_INFINITY;
    }

    /// The line height seen so far.
    ///
    /// The baseline divides the line into an above-baseline and below-baseline part, each of which
    /// is the larger of the text's contribution and any inline box's contribution:
    ///
    /// - The text contributes its ascent/descent plus its (possibly negative) half-leading. This
    ///   is the text "strut", which may be smaller than the text's ink when the leading is negative
    ///   (i.e. lines are allowed to overlap).
    /// - An inline box contributes its raw ascent/descent. A box only enlarges a side when it
    ///   extends beyond the text strut *including* any positive half-leading (so a box that fits
    ///   within positive leading does not grow the line). Crucially, the box's extent is compared
    ///   against the text padded only by *positive* leading, so an inline box never interacts with
    ///   negative leading: it neither cancels the line overlap that negative leading produces, nor
    ///   is its own extent shrunk by it.
    #[inline(always)]
    fn line_height(&self) -> f32 {
        let leading = self.text_line_height - self.text_ascent - self.text_descent;
        let leading_above = leading * 0.5;
        // Use the exact complement so that `leading_above + leading_below == leading`.
        let leading_below = leading - leading_above;
        // A box only grows a side if it extends past the text plus its positive half-leading.
        let threshold_above = self.text_ascent + leading_above.max(0.0);
        let threshold_below = self.text_descent + leading_below.max(0.0);

        if self.box_ascent <= threshold_above && self.box_descent <= threshold_below {
            // No inline box extends beyond the text, so the line is exactly the text's intrinsic
            // line height (preserving negative leading and floating-point exactness).
            self.text_line_height
        } else {
            let above = if self.box_ascent > threshold_above {
                self.box_ascent
            } else {
                self.text_ascent + leading_above
            };
            let below = if self.box_descent > threshold_below {
                self.box_descent
            } else {
                self.text_descent + leading_below
            };
            above + below
        }
    }
}

#[derive(Clone, Default)]
struct PrevBoundaryState {
    item_idx: usize,
    run_idx: usize,
    cluster_idx: usize,
    state: LineState,
}

/// Reason that the line breaker has yielded control flow
#[derive(Clone, Debug)]
pub enum YieldData {
    /// Control flow was yielded because a line break occurred.
    /// The `reason` field of the [`LineBreakData`] contains a specific reason about what caused a
    /// line break at this location.
    LineBreak(LineBreakData),
    /// Control flow was yielded because content on the line caused the line to exceed the max height
    ///
    /// The caller is responsible for finding a new location for the line with a greater available height
    /// adjusting the line geometry to the new position and resuming iteration.
    ///
    /// Note: that by default no max height is set (and one is not required for laying out text into
    /// rectangular regions), so you will only encounter this if you explicitly set a max height
    /// using `BreakLine::set_line_max_height`.
    MaxHeightExceeded(MaxHeightBreakData),
    /// Control flow was yielded because an inline box with kind [`InlineBoxKind::CustomOutOfFlow`]
    /// was encountered.
    ///
    /// Parley does not position these boxes itself. The caller is responsible for
    /// placing the box (e.g. via a caller-owned algorithm), adjusting the line geometry through
    /// [`BreakerState`], and then resuming iteration.
    InlineBoxBreak(BoxBreakData),
}

#[derive(Clone, Debug)]
/// Information about a line break
pub struct LineBreakData {
    /// The reason for the line break (see [`BreakReason`] for details)
    pub reason: BreakReason,
    /// The computed advance (width) of the line
    pub advance: f32,
    /// The computed height of the line
    pub line_height: f32,
    /// The position of the top of the line
    pub line_y_start: f64,
    /// The position of the bottom of the line
    pub line_y_end: f64,
}

#[derive(Clone, Debug)]
/// Information about a "max height break" (where control flow has been yielded due to the
/// line's configured max height being exceeded by content by laid out into the line).
pub struct MaxHeightBreakData {
    /// The current advance of the in-progress line
    pub advance: f32,
    /// The current line height of the in-progress line
    pub line_height: f32,
}

#[derive(Clone, Debug)]
/// Information about a "box break" (where control flow has been yielded due to an inline box
/// with kind [`InlineBoxKind::CustomOutOfFlow`] being encountered during layout.
pub struct BoxBreakData {
    /// The user-supplied ID for the inline box
    pub inline_box_id: u64,
    /// The index of the inline box within `Layout::inline_boxes()`
    pub inline_box_index: usize,
    /// The current advance of the line (up to but *not* including the `CustomOutOfFlow` box)
    pub advance: f32,
}

#[derive(Clone)]
/// The mutable state of the line breaker.
///
/// This is exposed so that callers using [`BreakLines`] directly can inspect and
/// adjust line geometry between calls to [`BreakLines::break_next`].
///
/// A `BreakerState` can be cloned and later passed to [`BreakLines::revert_to`]
/// to retry layout from a saved checkpoint.
pub struct BreakerState {
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

    /// The x coordinate of the left/start of the current line
    line_x: f32,
    /// The y coordinate of the top/start of the current line
    /// Use of f64 here is important. f32 causes test failures due to accumulated error
    line_y: f64,

    /// The max advance of the entire layout.
    layout_max_advance: f32,
    /// The max advance (max width) of the current line. This must be <= the `layout_max_advance`.
    line_max_advance: f32,
    /// The max height available to the current line.
    line_max_height: f32,

    /// The state of the current line
    line: LineState,

    // Saved breaker states for reverting to a previously encountered line-breaking opportunity
    /// Saved breaker state for the last non-emergency line-breaking opportunity
    prev_boundary: Option<PrevBoundaryState>,
    /// Saved breaker state for the last emergency line-breaking opportunity
    emergency_boundary: Option<PrevBoundaryState>,
}

impl Default for BreakerState {
    fn default() -> Self {
        Self {
            items: 0,
            lines: 0,
            item_idx: 0,
            run_idx: 0,
            cluster_idx: 0,
            line_x: 0.0,
            line_y: 0.0,
            layout_max_advance: 0.0,
            line_max_advance: 0.0,
            line_max_height: f32::MAX,
            line: LineState::default(),
            prev_boundary: None,
            emergency_boundary: None,
        }
    }
}

impl BreakerState {
    /// Add the cluster(s) currently being evaluated to the current line.
    ///
    /// `metrics` provides the raw font ascent and descent of the cluster(s) (i.e. the distances
    /// they extend above and below the baseline, *not* including leading) as well as the intrinsic
    /// line height of the cluster(s) (i.e. including the full leading), which may be smaller than
    /// `ascent + descent` when the leading is negative.
    pub fn append_cluster_to_line(&mut self, next_x: f32, metrics: &RunMetrics) {
        self.line.items.end = self.item_idx + 1;
        self.line.clusters.end = self.cluster_idx + 1;
        self.cluster_idx += 1;
        self.line.x = next_x;
        self.line.text_ascent = self.line.text_ascent.max(metrics.ascent);
        self.line.text_descent = self.line.text_descent.max(metrics.descent);
        self.line.text_line_height = self.line.text_line_height.max(metrics.line_height);
        self.update_max_height_exceeded();
    }

    /// Add an inline box to the line.
    ///
    /// `ascent` and `descent` are the distances the box extends above and below the text baseline
    /// respectively. A box with its bottom aligned to the baseline is simply one with a zero
    /// descent. The box grows the line only insofar as it extends beyond the text.
    pub fn append_inline_box_to_line(&mut self, next_x: f32, ascent: f32, descent: f32) {
        self.item_idx += 1;
        self.line.items.end += 1;
        self.line.x = next_x;
        self.line.box_ascent = self.line.box_ascent.max(ascent);
        self.line.box_descent = self.line.box_descent.max(descent);
        self.update_max_height_exceeded();
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

    /// Revert boundary state to prev state
    fn reset_to(&mut self, prev_state: PrevBoundaryState) {
        self.item_idx = prev_state.item_idx;
        self.run_idx = prev_state.run_idx;
        self.cluster_idx = prev_state.cluster_idx;
        self.line = prev_state.state;
    }

    #[inline(always)]
    fn update_max_height_exceeded(&mut self) {
        self.line.max_height_exceeded = self.line.line_height() > self.line_max_height;
    }

    /// Get the max-advance of the entire layout
    #[inline(always)]
    pub fn layout_max_advance(&self) -> f32 {
        self.layout_max_advance
    }
    /// Set the max-advance of the entire layout
    #[inline(always)]
    pub fn set_layout_max_advance(&mut self, advance: f32) {
        self.layout_max_advance = advance;
    }

    /// Get the max-advance of the current line
    #[inline(always)]
    pub fn line_max_advance(&self) -> f32 {
        self.line_max_advance
    }
    /// Set the max-advance of the current line
    #[inline(always)]
    pub fn set_line_max_advance(&mut self, advance: f32) {
        self.line_max_advance = advance;
    }

    /// Get the max-height of the current line
    #[inline(always)]
    pub fn line_max_height(&self) -> f32 {
        self.line_max_height
    }
    /// Set the max-height of the current line.
    #[inline(always)]
    pub fn set_line_max_height(&mut self, height: f32) {
        self.line_max_height = height;
    }

    /// Get the x-offset of the current line
    #[inline(always)]
    pub fn line_x(&self) -> f32 {
        self.line_x
    }
    /// Set the x-offset for the current line.
    #[inline(always)]
    pub fn set_line_x(&mut self, x: f32) {
        self.line_x = x;
    }

    /// Get the y-offset of the current line
    #[inline(always)]
    pub fn line_y(&self) -> f64 {
        self.line_y
    }
    /// Set the y-offset for the current line.
    #[inline(always)]
    pub fn set_line_y(&mut self, y: f64) {
        self.line_y = y;
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
    fn start_new_line(
        &mut self,
        reason: BreakReason,
        max_advance: f32,
        line_indent: f32,
    ) -> Option<YieldData> {
        commit_line(
            self.layout,
            &mut self.lines,
            &mut self.state.line,
            max_advance,
            reason,
            line_indent,
        );

        let line_height = self.state.line.line_height();
        let line_y_start = self.state.line_y;

        self.state.items = self.lines.line_items.len();
        self.state.lines = self.lines.lines.len();
        self.state.prev_boundary = None;
        self.state.emergency_boundary = None;

        // `finish_line` reads the line's accumulated vertical metrics from `self.state.line`, so
        // it must run before we reset the per-line running state.
        self.finish_line(self.lines.lines.len() - 1, line_height);
        self.state.line.reset();

        self.state.line_y += line_height as f64;

        Some(YieldData::LineBreak(
            self.last_line_data(reason, line_y_start),
        ))
    }

    #[inline(always)]
    fn last_line_data(&self, reason: BreakReason, line_y_start: f64) -> LineBreakData {
        let line = self.lines.lines.last().unwrap();
        LineBreakData {
            reason,
            advance: line.metrics.advance,
            line_height: line.size(),
            line_y_start,
            line_y_end: self.state.line_y,
        }
    }

    #[inline(always)]
    fn max_height_break_data(&self, line_height: f32) -> Option<YieldData> {
        Some(YieldData::MaxHeightExceeded(MaxHeightBreakData {
            advance: self.state.line.x,
            line_height,
        }))
    }

    #[inline(always)]
    pub fn state(&self) -> &BreakerState {
        &self.state
    }

    #[inline(always)]
    pub fn state_mut(&mut self) -> &mut BreakerState {
        &mut self.state
    }

    /// Set the max-advance of the previous line.
    ///
    /// This is an escape-hatch for allowing a custom width for
    /// alignment on each line, which is different to the breaking width.
    ///
    /// Should be used in combination with [`AlignmentOptions`](crate::AlignmentOptions::align_when_overflowing).
    ///
    /// This method changes a line's reported [`inline_max_coord`](LineMetrics::inline_max_coord), so
    /// if you use this method and read that value, you should be cautious.
    // This escape hatch has not been carefully evaluated for unexpected consequences.
    // It's current motivation is cases where Parley gives different line breaking results
    // than blink, for reasons which haven't been fully understood.
    #[doc(hidden)]
    pub fn set_prior_line_width(&mut self, advance: f32) {
        if let Some(line) = self.lines.lines.last_mut() {
            line.metrics.inline_max_coord = line.metrics.inline_min_coord + advance;
        }
    }

    /// Reverts the to an externally saved state.
    pub fn revert_to(&mut self, state: BreakerState) {
        self.state = state;
        self.lines.lines.truncate(self.state.lines);
        self.lines.line_items.truncate(self.state.items);
        self.done = false;
    }

    /// Reverts the last computed line, returning to the previous state.
    #[inline(always)]
    pub fn revert(&mut self) -> bool {
        if let Some(state) = self.prev_state.take() {
            self.revert_to(state);
            true
        } else {
            false
        }
    }

    /// Returns the y-coordinate of the top of the current line
    #[inline(always)]
    pub fn committed_y(&self) -> f64 {
        self.state.line_y
    }

    /// Returns true if all the text has been placed into lines.
    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Computes the next line in the paragraph. Returns the advance and size
    /// (width and height for horizontal layouts) of the line.
    #[inline(always)]
    pub fn break_next(&mut self) -> Option<YieldData> {
        self.break_next_line_or_box()
    }

    /// Computes the next line in the paragraph. Returns the advance and size
    /// (width and height for horizontal layouts) of the line.
    fn break_next_line_or_box(&mut self) -> Option<YieldData> {
        assert!(
            self.state.layout_max_advance == f32::INFINITY
                || self.state.line_max_advance - self.state.layout_max_advance < 1.0
        );

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
                self.state.line_max_advance
            };

        let line_indent = self.resolve_indent();

        let max_advance = max_advance - line_indent;

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

                    // The portion of the box above the baseline contributes to the line's ascent
                    // and the portion below to its descent. By default (no explicit baseline) the
                    // bottom of the box is aligned with the text baseline, i.e. the box is all
                    // ascent and zero descent. Out-of-flow boxes contribute nothing.
                    let (width_contribution, height_contribution, resolved_baseline) =
                        match inline_box.kind {
                            InlineBoxKind::InFlow => {
                                let baseline = inline_box.baseline.unwrap_or(inline_box.height);
                                (inline_box.width, inline_box.height, baseline)
                            }
                            InlineBoxKind::OutOfFlow => (0.0, 0.0, 0.0),
                            // If the box is a `CustomOutOfFlow` box then we yield control flow back to the caller.
                            // It is then the caller's responsibility to handle placement of the box.
                            InlineBoxKind::CustomOutOfFlow => {
                                return Some(YieldData::InlineBoxBreak(BoxBreakData {
                                    inline_box_id: inline_box.id,
                                    inline_box_index: item.index,
                                    advance: self.state.line.x,
                                }));
                            }
                        };

                    let ascent_contribution = resolved_baseline;
                    let descent_contribution = height_contribution - resolved_baseline;

                    // Compute the x position of the content being currently processed
                    let next_x = self.state.line.x + width_contribution;

                    // println!("BOX next_x: {}", next_x);

                    let box_will_be_appended = next_x <= max_advance || self.state.line.x == 0.0;
                    if height_contribution > self.state.line_max_height && box_will_be_appended {
                        return self.max_height_break_data(height_contribution);
                    }

                    // If the box fits on the current line (or we are at the start of the current line)
                    // then simply move on to the next item
                    if next_x <= max_advance || self.state.line.text_wrap_mode != TextWrapMode::Wrap
                    {
                        // println!("BOX FITS");

                        self.state.append_inline_box_to_line(
                            next_x,
                            ascent_contribution,
                            descent_contribution,
                        );

                        // We can always line break after an inline box
                        self.state.mark_line_break_opportunity();
                    } else {
                        // If we're at the start of the line, this box will never fit, so consume it and accept the overflow.
                        let reason = if self.state.line.x == 0.0 {
                            // println!("BOX EMERGENCY BREAK");
                            self.state.append_inline_box_to_line(
                                next_x,
                                ascent_contribution,
                                descent_contribution,
                            );
                            BreakReason::Emergency
                        } else {
                            // println!("BOX BREAK");
                            BreakReason::Regular
                        };
                        return self.start_new_line(reason, max_advance, line_indent);
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
                        let metrics = run.metrics();
                        let line_height = metrics.line_height;
                        let max_height_exceeded = self.state.line.max_height_exceeded;
                        let style = &self.layout.data.styles[cluster.data.style_index as usize];

                        // In `break-spaces` mode, preserved white space gets a soft-wrap
                        // opportunity after each character and does not "hang" at the end of a
                        // line (see the handling below).
                        let is_break_spaces =
                            style.white_space_collapse == WhiteSpaceCollapse::BreakSpaces;

                        // Lag text_wrap_mode style by one cluster
                        let text_wrap_mode = self.state.line.text_wrap_mode;
                        self.state.line.text_wrap_mode = style.text_wrap_mode;

                        if boundary == Boundary::Line && text_wrap_mode == TextWrapMode::Wrap {
                            // We do not currently handle breaking within a ligature, so we ignore boundaries in such a position.
                            //
                            // We also don't record boundaries when the advance is 0. As we do not want overflowing content to cause extra consecutive
                            // line breaks. We should accept the overflowing fragment in that scenario.
                            if !is_ligature_continuation && self.state.line.x != 0.0 {
                                self.state.mark_line_break_opportunity();
                                // break_opportunity = true;
                            }
                        } else if is_newline {
                            if max_height_exceeded {
                                return self.max_height_break_data(line_height);
                            }

                            // A CRLF sequence is a single grapheme cluster and must produce
                            // exactly one hard line break (UAX#14: CR × LF, do not break
                            // between). When this newline is a CR immediately followed by an
                            // LF, append the CR to the current line but suppress the break
                            // here and let the LF emit the single break, so CR and LF share
                            // one line. The lookahead reads the global cluster list so a CRLF
                            // that shaping split across two runs (e.g. a style boundary at the
                            // LF) is still coalesced. The LF must be item-adjacent to the CR:
                            // if it lands in a later run it only coalesces when the next item
                            // is that run (not an inline box sitting between the two), so an
                            // inline box at the LF offset keeps the CR's break. Lone CR, lone
                            // LF, LS, and PS are unaffected.
                            let lf_is_item_adjacent = self.state.cluster_idx + 1 < cluster_end
                                || self
                                    .layout
                                    .data
                                    .items
                                    .get(self.state.item_idx + 1)
                                    .is_some_and(|item| item.kind == LayoutItemKind::TextRun);
                            let is_cr_before_lf = cluster.info().source_char() == '\r'
                                && lf_is_item_adjacent
                                && self
                                    .layout
                                    .data
                                    .clusters
                                    .get(self.state.cluster_idx + 1)
                                    .is_some_and(|next| {
                                        next.info.whitespace() == Whitespace::Newline
                                            && next.info.source_char() == '\n'
                                    });

                            self.state
                                .append_cluster_to_line(self.state.line.x, metrics);

                            if is_cr_before_lf {
                                continue;
                            }

                            return self.start_new_line(
                                BreakReason::Explicit,
                                max_advance,
                                line_indent,
                            );
                        } else if
                        // This text can contribute "emergency" line breaks.
                        style.overflow_wrap != OverflowWrap::Normal && !is_ligature_continuation
                        && text_wrap_mode == TextWrapMode::Wrap
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

                        // If the content fits (the x position does NOT exceed max_advance)
                        //
                        // We simply append the cluster(s) to the current line
                        if next_x <= max_advance {
                            if max_height_exceeded {
                                return self.max_height_break_data(line_height);
                            }
                            self.state.append_cluster_to_line(next_x, metrics);
                            if is_space {
                                self.state.line.num_spaces += 1;
                            }
                            // `break-spaces`: a soft-wrap opportunity exists after every preserved
                            // white space character (including between consecutive spaces). Record
                            // it here, *after* appending, so the white space stays on the current
                            // line and the break happens before the following content.
                            if is_break_spaces
                                && whitespace != Whitespace::None
                                && text_wrap_mode == TextWrapMode::Wrap
                            {
                                self.state.mark_line_break_opportunity();
                            }
                        }
                        // Else we attempt to line break:
                        //
                        // This will only succeed if there is an available line-break opportunity that has been marked earlier
                        // in the line. If there is no such line-breaking opportunity (such as if wrapping is disabled), then
                        // we fall back to appending the content to the line anyway.
                        else {
                            // Case: cluster is a space character (and wrapping is enabled)
                            //
                            // Hanging white space is not considered when measuring the line's
                            // contents for fit, so an overflowing space must not cause a break by
                            // itself. We append it to the line (where it will hang) and keep
                            // consuming the rest of the white space run: the line then breaks at
                            // the next soft-wrap opportunity (just after the run, before the
                            // following content), at a forced break — where the white space
                            // *conditionally* hangs — or at the end of the text.
                            //
                            // In `break-spaces` mode, preserved white space does not hang; instead
                            // it takes up space and wraps like other content, so we fall through to
                            // the regular break-opportunity handling below (a soft-wrap opportunity
                            // was recorded after the preceding preserved white space character).
                            //
                            // A no-break space is not hangable white space: it is treated like any
                            // other visible character (and provides no soft-wrap opportunity), so
                            // it also falls through to the regular handling below.
                            if whitespace == Whitespace::Space
                                && !is_break_spaces
                                && text_wrap_mode == TextWrapMode::Wrap
                            {
                                if max_height_exceeded {
                                    return self.max_height_break_data(line_height);
                                }
                                self.state.append_cluster_to_line(next_x, metrics);
                                continue;
                            }
                            // Case: we have previously encountered a REGULAR line-breaking opportunity in the current line
                            //
                            // We "take" the line-breaking opportunity by starting a new line and resetting our
                            // item/run/cluster iteration state back to how it was when the line-breaking opportunity was encountered
                            else if let Some(prev) = self.state.prev_boundary.take() {
                                self.state.reset_to(prev);
                                return self.start_new_line(
                                    BreakReason::Regular,
                                    max_advance,
                                    line_indent,
                                );
                            }
                            // Case: we have previously encountered an EMERGENCY line-breaking opportunity in the current line
                            //
                            // We "take" the line-breaking opportunity by starting a new line and resetting our
                            // item/run/cluster iteration state back to how it was when the line-breaking opportunity was encountered
                            else if let Some(prev_emergency) =
                                self.state.emergency_boundary.take()
                            {
                                self.state.reset_to(prev_emergency);
                                return self.start_new_line(
                                    BreakReason::Emergency,
                                    max_advance,
                                    line_indent,
                                );
                            }
                            // Case: no line-breaking opportunities available
                            //
                            // This can happen when wrapping is disabled (TextWrapMode::NoWrap) or when no wrapping opportunities
                            // (according to our `OverflowWrap` and `WordBreak` styles) have yet been encountered.
                            //
                            // We fall back to appending the content to the line.
                            else {
                                if max_height_exceeded {
                                    return self.max_height_break_data(line_height);
                                }
                                self.state.append_cluster_to_line(next_x, metrics);
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
        self.done = true;
        self.start_new_line(BreakReason::None, max_advance, line_indent)
    }

    /// Computes the next line in the paragraph by character count.
    ///
    /// This method breaks lines based on the number of characters rather than advance width.
    /// Each text cluster (including whitespace and newlines) counts as 1 character.
    /// Each inline box also counts as 1 character.
    /// Ligature components each count separately (matching character count).
    ///
    /// Unlike `break_next`, this method does not respect normal line break opportunities and
    /// will break exactly when the character limit is reached. It does not break on newlines, for example.
    ///
    /// Inline boxes are supported and each contributes as 1 character.
    pub fn break_next_with_length(&mut self, max_chars: u32) -> Option<()> {
        if self.done {
            return None;
        }

        let line_indent = self.resolve_indent();

        // Track cluster count for this line
        let mut char_count: u32 = 0;

        let item_count = self.layout.data.items.len();
        while self.state.item_idx < item_count {
            let item = &self.layout.data.items[self.state.item_idx];

            match item.kind {
                LayoutItemKind::InlineBox => {
                    let inline_box = &self.layout.data.inline_boxes[item.index];

                    if inline_box.kind != InlineBoxKind::InFlow {
                        self.state
                            .append_inline_box_to_line(self.state.line.x, 0.0, 0.0);
                        continue;
                    }

                    // Check if adding this box would exceed the limit
                    if char_count >= max_chars && max_chars != 0 {
                        // Break before this box
                        self.start_new_line(BreakReason::Regular, f32::MAX, line_indent);
                        return Some(());
                    }

                    // Compute the x position for the line width tracking
                    let next_x = self.state.line.x + inline_box.width;
                    // The portion above the baseline is ascent, the rest descent. A box without an
                    // explicit baseline is bottom-aligned, i.e. all ascent and zero descent.
                    let baseline = inline_box.baseline.unwrap_or(inline_box.height);
                    self.state.append_inline_box_to_line(
                        next_x,
                        baseline,
                        inline_box.height - baseline,
                    );
                    char_count += 1;

                    // Check if we've reached the limit after adding this box
                    if char_count >= max_chars {
                        // Check if we've consumed all content (this is the last line).
                        let is_last_item = self.state.item_idx >= self.layout.data.items.len();
                        let break_reason = if is_last_item {
                            BreakReason::None
                        } else {
                            BreakReason::Regular
                        };

                        if break_reason == BreakReason::None {
                            self.done = true;
                        }
                        self.start_new_line(break_reason, f32::MAX, line_indent);
                        return Some(());
                    }
                }
                LayoutItemKind::TextRun => {
                    let run_idx = item.index;
                    let run_data = &self.layout.data.runs[run_idx];
                    let run = Run::new(self.layout, 0, 0, run_data, None);
                    let cluster_start = run_data.cluster_range.start;
                    let cluster_end = run_data.cluster_range.end;

                    while self.state.cluster_idx < cluster_end {
                        let cluster = run.get(self.state.cluster_idx - cluster_start).unwrap();

                        // Check if we should break before this cluster
                        if char_count >= max_chars && max_chars != 0 {
                            self.start_new_line(BreakReason::Regular, f32::MAX, line_indent);
                            return Some(());
                        }

                        let whitespace = cluster.info().whitespace();
                        let is_newline = whitespace == Whitespace::Newline;
                        let is_space = whitespace.is_space_or_nbsp();
                        let advance = cluster.advance();

                        // Compute the x position.
                        // Newlines don't contribute to line width (matching break_next behavior).
                        let next_x = if is_newline {
                            self.state.line.x
                        } else {
                            self.state.line.x + advance
                        };
                        let metrics = run.metrics();
                        self.state.append_cluster_to_line(next_x, metrics);
                        char_count += 1;

                        if is_space {
                            self.state.line.num_spaces += 1;
                        }

                        // Check if we've reached the limit after adding this cluster
                        if char_count >= max_chars {
                            // Determine the break reason:
                            // - BreakReason::None for the last line (end of content)
                            // - BreakReason::Explicit if this line ends with a newline
                            // - BreakReason::Regular for soft wraps
                            let is_last_cluster_of_run = self.state.cluster_idx >= cluster_end;
                            let is_last_item =
                                self.state.item_idx + 1 >= self.layout.data.items.len();
                            let break_reason = if is_last_cluster_of_run && is_last_item {
                                BreakReason::None
                            } else if is_newline {
                                BreakReason::Explicit
                            } else {
                                BreakReason::Regular
                            };

                            if break_reason == BreakReason::None {
                                self.done = true;
                            }
                            self.start_new_line(break_reason, f32::MAX, line_indent);
                            return Some(());
                        }
                    }
                    self.state.run_idx += 1;
                    self.state.item_idx += 1;
                }
            }
        }

        // Commit the final line (only reached if content remains after all break_next_with_length calls)
        if self.state.line.items.end == 0 {
            self.state.line.items.end = 1;
        }
        self.done = true;
        self.start_new_line(BreakReason::None, f32::MAX, line_indent);
        Some(())
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
        self.state.layout_max_advance = max_advance;
        self.state.line_max_advance = max_advance;
        while self.break_next().is_some() {}
        self.finish();
    }

    /// Consumes the line breaker and finalizes all line computations.
    pub fn finish(mut self) {
        if self.layout.data.text_len == 0
            && let Some(line) = self.lines.line_items.first_mut()
        {
            line.text_range = 0..0;
            line.cluster_range = 0..0;
        }
    }

    #[inline]
    fn resolve_indent(&self) -> f32 {
        let should_indent = {
            let is_scope_line = if self.layout.data.indent_options.each_line {
                self.lines.lines.is_empty()
                    || self.lines.lines.last().map(|l| l.break_reason)
                        == Some(BreakReason::Explicit)
            } else {
                self.lines.lines.is_empty()
            };
            is_scope_line ^ self.layout.data.indent_options.hanging
        };

        if should_indent {
            self.layout.data.indent_amount
        } else {
            0.0
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
        // Walk the line's items to compute text ranges, per-run advances and bidi ordering. The
        // vertical metrics (ascent/descent/line-height and inline box extents) are *not* computed
        // here: they were already accumulated into `self.state.line` as the line was built and are
        // read from there below. `have_metrics` records whether the line has any non-whitespace
        // content (text or an in-flow box), which distinguishes a genuinely empty line from a
        // whitespace-only one.
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
                    if item.kind == InlineBoxKind::InFlow {
                        // Mark us as having seen non-whitespace content on this line
                        have_metrics = true;
                    }
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

                    // Ignore trailing whitespace when deciding whether the line has content
                    // (we are iterating backwards so trailing whitespace comes first)
                    if !have_metrics && line_item.is_whitespace {
                        continue;
                    }

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
        //
        // How much of the trailing white space "hangs" (i.e. is excluded from the line's used
        // width and alignment) depends on the white-space-collapse mode (see
        // `EndOfLineWhitespace`). Removed and (unconditionally) hanging white space is fully
        // excluded; white space that only *conditionally* hangs (preserved white space at a forced
        // break or the last line) is excluded only insofar as it overflows the line.
        let run = if self.layout.is_rtl() {
            self.lines.line_items[line.item_range.clone()].first()
        } else {
            self.lines.line_items[line.item_range.clone()].last()
        };
        let break_reason = line.break_reason;
        let line_max_advance = line.max_advance;
        let line_advance = line.metrics.advance;
        line.metrics.trailing_whitespace = run
            .filter(|item| item.is_text_run() && item.has_trailing_whitespace)
            .map(|run| {
                let styles = &self.layout.data.styles;
                let clusters = &self.layout.data.clusters[run.cluster_range.clone()];
                let (ws_advance, end_of_line) = if run.is_rtl() {
                    trailing_whitespace_advance(clusters.iter(), styles)
                } else {
                    trailing_whitespace_advance(clusters.iter().rev(), styles)
                };
                match end_of_line {
                    // White space that takes up space (`break-spaces`, or preserved white space
                    // under `nowrap`) never hangs. (Normally unreachable, as such clusters are not
                    // reported as trailing white space.)
                    EndOfLineWhitespace::TakesUpSpace => 0.0,
                    // Removed white space always hangs (it is conceptually removed entirely).
                    EndOfLineWhitespace::Remove => ws_advance,
                    EndOfLineWhitespace::Hang => match break_reason {
                        // Soft wrap: the preserved white space hangs unconditionally.
                        BreakReason::Regular | BreakReason::Emergency => ws_advance,
                        // Forced break or last line (end of block): the white space only
                        // *conditionally* hangs, i.e. only the part that overflows the line hangs.
                        BreakReason::Explicit | BreakReason::None => {
                            (line_advance - line_max_advance).clamp(0.0, ws_advance)
                        }
                    },
                }
            })
            .unwrap_or(0.0);

        // Read the line's vertical extents from the running line state, which accumulated the
        // maximum text ascent/descent/line-height and inline box ascent/descent as content was
        // added (see `append_cluster_to_line` / `append_inline_box_to_line`). This runs before the
        // caller resets the state, and keeps `finish_line` consistent with the line height
        // computed by `LineState::line_height`. Whitespace-only lines still get their metrics from
        // the (whitespace) clusters that were appended to the state.
        line.metrics.ascent = self.state.line.text_ascent;
        line.metrics.descent = self.state.line.text_descent;
        // The text's intrinsic line height (without any growth caused by inline boxes); the text
        // strut and its leading are derived from this.
        let mut text_line_height = self.state.line.text_line_height;
        // How far the inline boxes on the line extend above/below the text baseline. Kept separate
        // from the text extents so that a box only shifts the baseline / grows the line when it
        // reaches beyond the text strut. `NEG_INFINITY` (the state's default) means "no box".
        let mut box_ascent = self.state.line.box_ascent;
        let mut box_descent = self.state.line.box_descent;

        if !have_metrics
            && line.item_range.is_empty()
            && let Some(metrics) = prev_line_metrics
        {
            // HACK: copy metrics from previous line if we don't have
            // any; this should only occur for an empty line following
            // a newline at the end of a layout
            line.metrics = metrics;
            text_line_height = metrics.line_height;
            box_ascent = f32::NEG_INFINITY;
            box_descent = f32::NEG_INFINITY;
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

        // Like `ascent` and `descent`, the reported leading ignores inline boxes: it is derived
        // from the text's *intrinsic* line height, not the (possibly larger) `line_height` that an
        // inline box may have grown the line to.
        line.metrics.leading = text_line_height - (line.metrics.ascent + line.metrics.descent);

        // Whether metrics should be quantized to pixel boundaries
        let quantize = self.layout.data.quantize;

        // We mimic Chrome in rounding ascent and descent separately,
        // before calculating the rest.
        // See lines_integral_line_height_ascent_descent_rounding() for more details.
        let (ascent, descent) = if quantize {
            box_ascent = box_ascent.round();
            box_descent = box_descent.round();
            (line.metrics.ascent.round(), line.metrics.descent.round())
        } else {
            (line.metrics.ascent, line.metrics.descent)
        };

        // The leading is distributed around the baseline using the text's *intrinsic* line height,
        // never the (possibly larger) line height caused by inline boxes. This is what keeps
        // consecutive baselines on a consistent grid: an inline box that fits within the line does
        // not perturb the text strut, and therefore does not move the baseline.
        let (leading_above, leading_below) = if quantize {
            // Calculate leading using the rounded ascent and descent.
            let leading = text_line_height - (ascent + descent);
            // We mimic Chrome in giving 'below' the larger leading half.
            // Although the comment in Chromium's NGLineHeightMetrics::AddLeading function
            // in ng_line_height_metrics.cc claims it's for legacy test compatibility.
            // So we might want to think about giving 'above' the larger half instead.
            let above = (leading * 0.5).floor();
            let below = leading.round() - above;
            (above, below)
        } else {
            let leading = text_line_height - (ascent + descent);
            (leading * 0.5, leading - leading * 0.5)
        };

        // The text strut extends `ascent + positive-half-leading` above the baseline and
        // `descent + positive-half-leading` below it. An inline box only grows a side of the line
        // (and, for the ascent side, pushes the baseline down) when it extends beyond this strut.
        // A box sitting below the baseline therefore grows the line downwards without disturbing
        // the baseline. This mirrors `LineState::line_height`.
        let threshold_above = ascent + leading_above.max(0.);
        let threshold_below = descent + leading_below.max(0.);

        // Distance from the top of the line down to the baseline. Note the unclamped
        // `leading_above` here (as opposed to the clamped value used for the min coord below):
        // negative leading is correct for baseline placement but must not shrink the min/max
        // coords.
        let above = if box_ascent > threshold_above {
            box_ascent
        } else {
            ascent + leading_above
        };

        let y = self.state.line_y;
        line.metrics.baseline = above + if quantize { y.round() as f32 } else { y as f32 };

        // Small line heights will cause leading to be negative.
        // Negative leadings are correct for baseline calculation, but not for min/max coords.
        // We clamp leading to zero for the purposes of min/max coords,
        // which in turn clamps the selection box minimum height to ascent + descent.
        // Inline boxes extend the coords whenever they reach beyond the (leading-padded) text.
        let block_above = threshold_above.max(box_ascent);
        let block_below = threshold_below.max(box_descent);
        line.metrics.block_min_coord = line.metrics.baseline - block_above;
        line.metrics.block_max_coord = line.metrics.baseline + block_below;

        // let max_advance = if self.state.line_max_advance < f32::MAX {
        //     self.state.line_max_advance
        // } else {
        //     line.metrics.advance - line.metrics.trailing_whitespace
        // };

        line.metrics.inline_min_coord = self.state.line_x;
        line.metrics.inline_max_coord = self.state.line_x + self.state.line_max_advance;
    }
}

impl<B: Brush> Drop for BreakLines<'_, B> {
    fn drop(&mut self) {
        // Compute the overall width and height of the entire layout
        // The "width" excludes trailing whitespace. The "full_width" includes it.
        let mut layout_width = 0_f32;
        let mut layout_full_width = 0_f32;
        let mut height = 0_f64; // f32 causes test failures due to accumulated error
        for line in &mut self.lines.lines {
            let indent_extra = line.indent.max(0.0);
            let line_max = line.metrics.inline_min_coord + line.metrics.advance + indent_extra;
            layout_full_width = layout_full_width.max(line_max);
            layout_width = layout_width.max(line_max - line.metrics.trailing_whitespace);
            height += line.metrics.line_height as f64;
        }

        // If laying out with infinite width constraint, then set all lines' "max_width"
        // to the measured width of the longest line.
        if self.state.layout_max_advance >= f32::MAX {
            for line in &mut self.lines.lines {
                if line.metrics.inline_max_coord >= f32::MAX {
                    line.metrics.inline_max_coord = layout_width;
                }
            }
        }

        // Don't include the last line's line_height in the layout's height if the last line is empty
        if let Some(last_line) = self.lines.lines.last()
            && last_line.item_range.is_empty()
        {
            height -= last_line.metrics.line_height as f64;
        }

        // Save the computed widths/height to the layout
        self.layout.data.width = layout_width;
        self.layout.data.full_width = layout_full_width;
        self.layout.data.height = height as f32;
        self.layout.data.layout_max_advance = self.state.layout_max_advance;

        // for (i, line) in self.lines.lines.iter().enumerate() {
        //     println!("LINE {i} (h:{})", line.metrics.line_height);
        //     for item_idx in line.item_range.clone() {
        //         let item = &self.lines.line_items[item_idx];
        //         println!("  ITEM {:?} ({})", item.kind, item.advance);
        //     }
        // }

        // Save the computed lines to the layout
        self.lines.swap(&mut self.layout.data);
    }
}

/// Sums the advance of the run of trailing white space yielded by `clusters` (which must iterate
/// from the trailing edge of the line inward), and returns it along with the
/// [`EndOfLineWhitespace`] behavior of the cluster at the very trailing edge.
///
/// A no-break space is not hangable white space (it is treated like any other visible character),
/// so it terminates the trailing white space run.
fn trailing_whitespace_advance<'c, B: Brush>(
    clusters: impl Iterator<Item = &'c ClusterData>,
    styles: &[crate::layout::Style<B>],
) -> (f32, EndOfLineWhitespace) {
    let mut advance = 0.0;
    // Overwritten by the trailing-edge (first-yielded) white space cluster below. The default is
    // only used if there is no trailing white space, in which case the advance is zero anyway.
    let mut end_of_line = EndOfLineWhitespace::Remove;
    for (i, cluster) in clusters
        .take_while(|cluster| {
            !matches!(
                cluster.info.whitespace(),
                Whitespace::None | Whitespace::NoBreakSpace
            )
        })
        .enumerate()
    {
        advance += cluster.advance;
        if i == 0 {
            let style = &styles[cluster.style_index as usize];
            end_of_line = style
                .white_space_collapse
                .end_of_line_whitespace(style.text_wrap_mode);
        }
    }
    (advance, end_of_line)
}

fn commit_line<B: Brush>(
    layout: &Layout<B>,
    lines: &mut LineLayout,
    state: &mut LineState,
    max_advance: f32,
    break_reason: BreakReason,
    line_indent: f32,
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

    // Exclude the trailing space from justification space count.
    // Only subtract if the line actually ends with a space — with
    // WordBreak::BreakAll, regular breaks can land between non-space
    // characters, in which case there is no trailing space to exclude.
    let mut num_spaces = state.num_spaces;
    if break_reason == BreakReason::Regular
        && state.clusters.start < state.clusters.end
        && layout.data.clusters[state.clusters.end - 1]
            .info
            .whitespace()
            .is_space_or_nbsp()
    {
        num_spaces = num_spaces.saturating_sub(1);
    }

    lines.lines.push(LineData {
        item_range: start_item_idx..end_item_idx,
        max_advance,
        break_reason,
        num_spaces,
        indent: line_indent,
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
