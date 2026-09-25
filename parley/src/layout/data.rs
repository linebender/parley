// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::inline_box::InlineBox;
use crate::layout::spacing::{Justification, Spacing};
use crate::layout::whitespace::whitespace_hangs;
use crate::layout::{ContentWidths, LineMetrics, Style};
use crate::resolve::ResolvedStyle;
use crate::style::Brush;
use crate::{
    IndentOptions, InlineBoxKind, LineHeight, OverflowWrap, TextWrapMode, WhiteSpaceCollapse,
};
use core::ops::Range;

use alloc::vec::Vec;
use parlance::BidiLevel;
use parley_engine::shape::Whitespace;
use parley_engine::{ShapedSlice, ShapedText};

/// `HarfRust`-based run data
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunData {
    /// Font attributes, needed for accessibility.
    pub(crate) font_attrs: fontique::Attributes,
    /// Synthesis for rendering (contains variation settings)
    pub(crate) synthesis: fontique::Synthesis,
    /// The line height
    pub line_height: f32,
    /// Additional spacing inserted between this run's atoms.
    ///
    /// TODO: Letter spacing in the form of gaps should not be applied between cursive scripts, see
    /// [CSS Text 4 § 8.2.1][css-spacing-cursive]. Currently we erroneously *do* apply it.
    ///
    /// [css-spacing-cursive]: https://www.w3.org/TR/css-text-4/#cursive-tracking
    pub(crate) spacing: Spacing,
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum BreakReason {
    #[default]
    None,
    Regular,
    Explicit,
    Emergency,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct LineData {
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of line items.
    pub(crate) item_range: Range<usize>,
    /// Metrics for the line.
    pub(crate) metrics: LineMetrics,
    /// The cause of the line break.
    pub(crate) break_reason: BreakReason,
    /// Maximum advance for the line.
    pub(crate) max_advance: f32,
    /// The number of justification opportunities on the line.
    pub(crate) num_justification_opportunities: u32,
    pub(crate) justification: Justification,
    /// Text indent applied to this line.
    pub(crate) indent: f32,
}

impl LineData {
    pub(crate) fn size(&self) -> f32 {
        self.metrics.line_height
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LineItemData {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: BidiLevel,

    // Fields that only apply to text runs (Ignored for boxes)
    // TODO: factor this out?
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// This run's shaped clusters on this line, as a range into [`ShapedText::shaped_clusters`].
    ///
    /// The bounds are atom-aligned.
    pub(crate) shaped_cluster_range: Range<u32>,
}

impl LineItemData {
    #[inline(always)]
    pub(crate) fn is_rtl(&self) -> bool {
        self.bidi_level.is_rtl()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutItemKind {
    TextRun,
    InlineBox,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutItem {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: BidiLevel,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutData<B: Brush> {
    // General settings (directly from the "builder")
    /// The display scale factor
    pub(crate) scale: f32,
    /// Whether metrics should be quantized to pixel boundaries
    pub(crate) quantize: bool,
    /// The `BiDi` base level
    pub(crate) base_level: BidiLevel,
    /// The length of the text in the layout
    pub(crate) text_len: usize,

    // Output of style resolution (input to line breaking)
    pub(crate) styles: Vec<Style<B>>,
    pub(crate) inline_boxes: Vec<InlineBox>,

    // Output of shaping (input to line breaking)
    pub(crate) shaped_text: ShapedText,
    pub(crate) runs: Vec<RunData>,
    pub(crate) items: Vec<LayoutItem>,

    // Output of line breaking
    /// The lines in the
    pub(crate) lines: Vec<LineData>,
    /// Items within each line
    pub(crate) line_items: Vec<LineItemData>,
    /// The width constraint that was used to line break the layout
    pub(crate) layout_max_advance: f32,
    /// The computed width of the layout excluding hanging whitespace
    pub(crate) width: f32,
    /// The computed width of the layout including hanging whitespace
    pub(crate) full_width: f32,
    /// The computed height of the layout
    pub(crate) height: f32,

    // Output of alignment
    /// The alignment that was applied to the layout, if any.
    pub(crate) alignment: Option<super::Alignment>,
    /// The text-indent amount in layout units.
    pub(crate) indent_amount: f32,
    /// Options controlling text-indent behavior (each-line, hanging).
    pub(crate) indent_options: IndentOptions,
}

impl<B: Brush> Default for LayoutData<B> {
    fn default() -> Self {
        Self {
            scale: 1.,
            quantize: true,
            base_level: BidiLevel::new(0),
            text_len: 0,
            width: 0.,
            full_width: 0.,
            height: 0.,
            styles: Vec::new(),
            inline_boxes: Vec::new(),
            shaped_text: ShapedText::new(),
            runs: Vec::new(),
            items: Vec::new(),
            lines: Vec::new(),
            line_items: Vec::new(),
            alignment: None,
            layout_max_advance: 0.0,
            indent_amount: 0.0,
            indent_options: IndentOptions::default(),
        }
    }
}

impl<B: Brush> LayoutData<B> {
    pub(crate) fn clear(&mut self) {
        self.scale = 1.;
        self.quantize = true;
        self.base_level = BidiLevel::new(0);
        self.text_len = 0;
        self.width = 0.;
        self.full_width = 0.;
        self.height = 0.;
        self.layout_max_advance = 0.0;
        self.indent_amount = 0.0;
        self.indent_options = IndentOptions::default();
        self.styles.clear();
        self.inline_boxes.clear();
        self.shaped_text.clear();
        self.runs.clear();
        self.items.clear();
        self.lines.clear();
        self.line_items.clear();
        self.alignment = None;
    }

    /// Push an inline box to the list of items
    pub(crate) fn push_inline_box(&mut self, index: usize, bidi_level: BidiLevel) {
        self.items.push(LayoutItem {
            kind: LayoutItemKind::InlineBox,
            index,
            bidi_level,
        });
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn process_shaped_run(
        &mut self,
        shaped_run_idx: usize,
        run_style: &ResolvedStyle<B>,
        spacing: Spacing,
    ) {
        let shaped_run = &self.shaped_text.runs()[shaped_run_idx];
        debug_assert!(
            !shaped_run.shaped_clusters_range.is_empty(),
            "Shaped runs returned by `parley_engine` must be non-empty"
        );
        let style_index =
            self.shaped_text.characters()[shaped_run.characters_range.start as usize].style_index;

        let line_height = {
            // Compute line height
            let style = &self.styles[style_index as usize];
            match style.line_height {
                LineHeight::Absolute(value) => value,
                LineHeight::FontSizeRelative(value) => value * shaped_run.font_size,
                LineHeight::MetricsRelative(value) => {
                    (shaped_run.font_metrics.ascent
                        + shaped_run.font_metrics.descent
                        + shaped_run.font_metrics.leading)
                        * value
                }
            }
        };

        let font = &self.shaped_text.fonts()[shaped_run.font_index];
        let run = RunData {
            font_attrs: fontique::Attributes {
                width: run_style.font_width,
                weight: run_style.font_weight,
                style: run_style.font_style,
            },
            synthesis: font.synthesis,
            line_height,
            spacing,
        };

        self.runs.push(run);
        self.items.push(LayoutItem {
            kind: LayoutItemKind::TextRun,
            index: self.runs.len() - 1,
            bidi_level: shaped_run.bidi_level,
        });
    }

    /// Calculates the min- and max-content widths of the layout.
    ///
    /// The max-content width is the widest line when breaking only at forced breaks. The
    /// min-content width is the widest line when breaking at every opportunity.
    ///
    /// This should mirror the line breaker in terms of layout decisions like hanging whitespace (if
    /// it doesn't, one of the calculations is buggy).
    ///
    /// Hanging whitespace is not considered when measuring the line's content for fit, but some
    /// whitespace hangs *conditionally*. See the module-level docs of [`crate::layout::whitespace`]
    /// for an explanation of how conditionally and unconditionally hanging whitespace are
    /// determined. For the content widths this means:
    ///
    /// - max-content width: if there's a conditionally-hanging suffix of whitespace, it fits, so
    ///   nothing hangs; without one, the unconditionally hanging whitespace hangs in full.
    /// - min-content width: any conditionally-hanging suffix hangs in full, so all hanging
    ///   whitespace hangs in full.
    #[expect(clippy::cast_possible_truncation, reason = "deferred")]
    pub(crate) fn calculate_content_widths(&self) -> ContentWidths {
        let mut state = ContentWidthsState {
            min_width: 0.,
            max_width: 0.,
            running_min_width: 0.,
            running_max_width: 0.,
            running_hanging_whitespace: 0.,
            hangs_conditionally: false,
            text_wrap_mode: TextWrapMode::Wrap,
        };

        for item in &self.items {
            match item.kind {
                LayoutItemKind::TextRun => {
                    let slice = self.shaped_text.run_slice(item.index as u32);
                    let run_spacing = self.runs[item.index].spacing;
                    // Trailing whitespace can only hang if it ends up at the line's end edge after
                    // bidi reordering. We don't currently apply UAX #9 L1 (resetting trailing
                    // whitespace to paragraph level), so only logically-last items that match the
                    // paragraph level are guaranteed to be at that edge.
                    let can_hang = item.bidi_level == self.base_level;
                    let is_rtl = item.bidi_level.is_rtl();

                    if run_spacing.is_zero() {
                        state.measure_text_run::<B, false>(
                            &self.styles,
                            slice,
                            run_spacing,
                            can_hang,
                            is_rtl,
                        );
                    } else {
                        state.measure_text_run::<B, true>(
                            &self.styles,
                            slice,
                            run_spacing,
                            can_hang,
                            is_rtl,
                        );
                    }
                }
                LayoutItemKind::InlineBox => {
                    let ibox = &self.inline_boxes[item.index];
                    if ibox.kind == InlineBoxKind::InFlow {
                        state.running_max_width += ibox.width;
                        if state.text_wrap_mode == TextWrapMode::Wrap {
                            state.min_width = state
                                .min_width
                                .max(state.running_min_width - state.running_hanging_whitespace);
                            state.min_width = state.min_width.max(ibox.width);
                            state.running_min_width = 0.0;
                        } else {
                            state.running_min_width += ibox.width;
                        }
                        // Inline boxes don't hang.
                        state.running_hanging_whitespace = 0.0;
                    }
                }
            }
        }

        // The end of a layout is considered to be a forced line break as per CSS Text 4 § 5, so
        // whitespace can hang conditionally.
        state.min_width = state
            .min_width
            .max(state.running_min_width - state.running_hanging_whitespace);
        if !state.hangs_conditionally {
            state.running_max_width -= state.running_hanging_whitespace;
        }
        state.max_width = state.max_width.max(state.running_max_width);

        // Negative-width inline boxes (e.g. negative margins) can make the max-content width
        // smaller than the min-content width. CSS Sizing 3 § 2.1 requires the max-content size to
        // be floored by the min-content size.
        state.max_width = state.max_width.max(state.min_width);

        ContentWidths {
            min: state.min_width,
            max: state.max_width,
        }
    }
}

struct ContentWidthsState {
    min_width: f32,
    max_width: f32,

    running_min_width: f32,
    running_max_width: f32,

    /// The running advance of whitespace that would hang if a line ended here. Can exceed
    /// `running_min_width` when the hanging whitespace started before the last break
    /// opportunity, in which case the line consists entirely of hanging whitespace.
    running_hanging_whitespace: f32,

    /// Whether that running whitespace hangs conditionally before a forced break (following CSS
    /// Text 4 § 4.3.2, that's the case for `WhiteSpaceCollapse::Preserve`). Conditionally hanging
    /// whitespace only hangs if it doesn't fit, which here means it counts towards the max-content
    /// width, but not the min-content width.
    hangs_conditionally: bool,

    text_wrap_mode: TextWrapMode,
}

impl ContentWidthsState {
    /// Measures a text run.
    ///
    /// The run is scanned one shaped cluster at a time. break opportunities are only considered at
    /// grapheme starts, and the hanging-whitespace accumulator is updated per cluster (a cluster
    /// that doesn't hang resets it, one that does extends it), which is equivalent to the per-atom
    /// "hangs entirely / hangs partially from the logical end" rule of
    /// [`atom_hanging_advance`](crate::layout::whitespace::atom_hanging_advance).
    ///
    /// With `SPACED`, each atom's spacing gap is added to the cluster on its visual end, i.e., its
    /// logically last cluster for LTR and its logically first cluster for RTL. The gap then hangs
    /// with that cluster, as in `atom_hanging_advance`. Without `SPACED`, `spacing` is ignored.
    #[inline(always)]
    fn measure_text_run<B: Brush, const SPACED: bool>(
        &mut self,
        styles: &[Style<B>],
        slice: ShapedSlice<'_>,
        spacing: Spacing,
        can_hang: bool,
        is_rtl: bool,
    ) {
        let clusters = slice.shaped_clusters();

        // The spacing gap of the current atom.
        let mut gap = 0.;
        let mut skip_atom = false;
        for (i, cluster) in clusters.iter().enumerate() {
            let whitespace = cluster.whitespace();
            let style = &styles[cluster.style_index as usize];
            let is_atom_start = i == 0 || cluster.is_grapheme_start();
            if is_atom_start {
                skip_atom = false;
                let prev_text_wrap_mode = self.text_wrap_mode;
                self.text_wrap_mode = style.text_wrap_mode;
                if prev_text_wrap_mode == TextWrapMode::Wrap
                    && (cluster.is_soft_wrap_opportunity_before()
                        || style.overflow_wrap == OverflowWrap::Anywhere)
                {
                    self.min_width = self
                        .min_width
                        .max(self.running_min_width - self.running_hanging_whitespace);
                    self.running_min_width = 0.0;
                }
                // `Whitespace::Newline` are forced breaks. Note newlines have no advance.
                if whitespace == Whitespace::Newline {
                    // Newlines hang, so whitespace before them keeps hanging.
                    self.min_width = self
                        .min_width
                        .max(self.running_min_width - self.running_hanging_whitespace);
                    if !self.hangs_conditionally {
                        self.running_max_width -= self.running_hanging_whitespace;
                    }
                    self.max_width = self.max_width.max(self.running_max_width);
                    self.running_min_width = 0.0;
                    self.running_max_width = 0.0;
                    self.running_hanging_whitespace = 0.0;
                    skip_atom = true;
                    continue;
                }
                if SPACED {
                    gap = spacing.atom_gap(whitespace);
                }
            } else if skip_atom {
                continue;
            }

            let mut advance = cluster.advance;
            if SPACED {
                let owns_gap = if is_rtl {
                    is_atom_start
                } else {
                    clusters
                        .get(i + 1)
                        .is_none_or(|next| next.is_grapheme_start())
                };
                if owns_gap {
                    advance += gap;
                }
            }
            self.running_min_width += advance;
            self.running_max_width += advance;

            // A cluster hangs if all of its characters hang. Its first character is checked via the
            // cached flags so the common case never touches `characters`.
            let hangs = can_hang
                && whitespace_hangs(whitespace, style)
                && (cluster.char_len() == 1
                    || slice
                        .characters_in(cluster.chars_range())
                        .iter()
                        .all(|character| {
                            whitespace_hangs(
                                character.whitespace,
                                &styles[character.style_index as usize],
                            )
                        }));
            if hangs {
                self.running_hanging_whitespace += advance;
                self.hangs_conditionally =
                    style.white_space_collapse == WhiteSpaceCollapse::Preserve;
            } else {
                self.running_hanging_whitespace = 0.0;
            }
        }
    }
}
