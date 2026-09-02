// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::inline_box::InlineBox;
use crate::layout::spacing::{EffectiveSpacing, Justification, Spacing};
use crate::layout::style_metrics::StyleMetrics;
use crate::layout::{ContentWidths, LineMetrics, Style};
use crate::resolve::ResolvedStyle;
use crate::style::Brush;
use crate::{IndentOptions, InlineBoxKind, LineHeight, OverflowWrap, TextWrapMode};
use core::ops::Range;

use alloc::vec::Vec;
use parlance::BidiLevel;
use parley_engine::shape::Whitespace;
use parley_engine::{Boundary, ShapedSlice, ShapedText};

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
    /// Number of justified clusters on the line.
    pub(crate) justification_opportunities: usize,
    pub(crate) justification: Justification,
    /// Text indent applied to this line.
    pub(crate) indent: f32,
    /// This line's entries in [`LayoutData::aligned_subtree_offsets`]. Empty for lines with only
    /// baseline-relative content.
    pub(crate) aligned_subtree_offsets: Range<u32>,
}

/// Position of an [aligned subtree] (rooted at a `vertical-align: top | bottom` style) on a line.
///
/// [aligned subtree]: crate::layout::style_metrics#aligned-subtrees
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AlignedSubtreeOffset {
    /// Style index of the subtree root.
    pub(crate) root: u16,
    /// Offset from the line's baseline to the subtree's baseline (positive upwards).
    pub(crate) baseline_offset: f32,
}

impl LineData {
    /// Offset from the line's baseline to the baseline of the aligned subtree rooted at style
    /// `root` (positive upwards). `offsets` is [`LayoutData::aligned_subtree_offsets`].
    pub(crate) fn aligned_subtree_offset(
        &self,
        offsets: &[AlignedSubtreeOffset],
        root: u16,
    ) -> f32 {
        if root == 0 {
            return 0.;
        }
        let range =
            self.aligned_subtree_offsets.start as usize..self.aligned_subtree_offsets.end as usize;
        offsets[range]
            .iter()
            .find(|subtree| subtree.root == root)
            .map_or(0., |subtree| subtree.baseline_offset)
    }

    /// Block-axis coordinate of the baseline of the given style's inline box. `offsets` is
    /// [`LayoutData::aligned_subtree_offsets`].
    pub(crate) fn style_baseline(
        &self,
        offsets: &[AlignedSubtreeOffset],
        metrics: &StyleMetrics,
    ) -> f32 {
        self.metrics.baseline
            - self.aligned_subtree_offset(offsets, metrics.aligned_subtree)
            - metrics.baseline_offset
    }

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
    /// Advance (size in direction of text flow) for the run.
    ///
    /// This includes the run's [`Spacing`], but not the justification. Spacing is a property of
    /// graphemes and shaped clusters, so can be known, whereas justification depends on a line's
    /// free space.
    pub(crate) advance: f32,

    // Fields that only apply to text runs (Ignored for boxes)
    // TODO: factor this out?
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// This run's shaped clusters on this line, as a range into [`ShapedText::shaped_clusters`].
    ///
    /// The bounds are atom-aligned.
    pub(crate) shaped_cluster_range: Range<u32>,
    /// This run's grapheme clusters on this line, as a range of grapheme indices relative to the
    /// owning [`parley_engine::ShapedRun`].
    pub(crate) grapheme_range: Range<usize>,
}

impl LineItemData {
    pub(crate) fn is_text_run(&self) -> bool {
        self.kind == LayoutItemKind::TextRun
    }

    #[inline(always)]
    pub(crate) fn is_rtl(&self) -> bool {
        self.bidi_level.is_rtl()
    }
}

/// The number of graphemes in `slice`.
///
/// This is `O(n)` in the slice's characters.
pub(crate) fn count_graphemes(slice: ShapedSlice<'_>) -> usize {
    slice
        .characters_in(slice.char_range())
        .iter()
        .filter(|character| character.grapheme_start)
        .count()
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
    /// Inline box metrics of each entry of `styles`.
    pub(crate) style_metrics: Vec<StyleMetrics>,
    pub(crate) inline_boxes: Vec<InlineBox>,
    /// Style index of the inline box (span) containing each entry of `inline_boxes`. This is the
    /// parent against which the box's `vertical_align` is resolved.
    pub(crate) inline_box_styles: Vec<u16>,

    // Output of shaping (input to line breaking)
    pub(crate) shaped_text: ShapedText,
    pub(crate) runs: Vec<RunData>,
    pub(crate) items: Vec<LayoutItem>,

    // Output of line breaking
    /// The lines in the
    pub(crate) lines: Vec<LineData>,
    /// Items within each line
    pub(crate) line_items: Vec<LineItemData>,
    /// Position of each aligned subtree rooted at a `vertical-align: top | bottom` style on each
    /// line. Each line owns a contiguous range ([`LineData::aligned_subtree_offsets`]), in line
    /// order.
    pub(crate) aligned_subtree_offsets: Vec<AlignedSubtreeOffset>,
    /// The width constraint that was used to line break the layout
    pub(crate) layout_max_advance: f32,
    /// The computed width of the layout excluding hanging whitespace
    pub(crate) width: f32,
    /// The computed width of the layout including hanging whitespace
    pub(crate) full_width: f32,
    /// The computed height of the layout
    pub(crate) height: f32,

    // Output of alignment
    #[cfg(feature = "accesskit")]
    /// Directly store the alignment if accessibility is enabled so we can
    /// set the corresponding AccessKit property.
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
            style_metrics: Vec::new(),
            inline_boxes: Vec::new(),
            inline_box_styles: Vec::new(),
            shaped_text: ShapedText::new(),
            runs: Vec::new(),
            items: Vec::new(),
            lines: Vec::new(),
            line_items: Vec::new(),
            aligned_subtree_offsets: Vec::new(),
            #[cfg(feature = "accesskit")]
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
        self.styles.clear();
        self.style_metrics.clear();
        self.inline_boxes.clear();
        self.inline_box_styles.clear();
        self.shaped_text.clear();
        self.runs.clear();
        self.items.clear();
        self.lines.clear();
        self.line_items.clear();
        self.aligned_subtree_offsets.clear();
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

    // TODO: this method does not handle mixed direction text at all.
    #[expect(clippy::cast_possible_truncation, reason = "deferred")]
    pub(crate) fn calculate_content_widths(&self) -> ContentWidths {
        fn hanging_whitespace_advance(atom: Option<(Whitespace, f32)>) -> f32 {
            atom.filter(|(whitespace, _)| {
                matches!(
                    whitespace,
                    Whitespace::Space | Whitespace::Tab | Whitespace::Newline
                )
            })
            .map_or(0.0, |(_, advance)| advance)
        }

        let mut min_width = 0.0_f32;
        let mut max_width = 0.0_f32;

        let mut running_min_width = 0.0;
        let mut running_max_width = 0.0;
        let mut text_wrap_mode = TextWrapMode::Wrap;
        // The whitespace class of the previous atom's first character, and the atom's advance.
        let mut prev_atom: Option<(Whitespace, f32)> = None;
        let is_rtl = self.base_level.is_rtl();
        for item in &self.items {
            match item.kind {
                LayoutItemKind::TextRun => {
                    let slice = self.shaped_text.run_slice(item.index as u32);
                    let spacing =
                        EffectiveSpacing::new(self.runs[item.index].spacing, Justification::NONE);
                    if is_rtl {
                        prev_atom = slice.atoms_start().next().map(|atom| {
                            let character = &atom.characters()[0];
                            (character.info.whitespace(), spacing.atom_advance(&atom))
                        });
                    }
                    for atom in slice.atoms_start() {
                        let character = &atom.characters()[0];
                        let boundary = character.info.boundary();
                        let style = &self.styles[character.style_index as usize];
                        let prev_text_wrap_mode = text_wrap_mode;
                        text_wrap_mode = style.text_wrap_mode;
                        if boundary == Boundary::Mandatory
                            || (prev_text_wrap_mode == TextWrapMode::Wrap
                                && (boundary == Boundary::Line
                                    || style.overflow_wrap == OverflowWrap::Anywhere))
                        {
                            let hanging_whitespace = hanging_whitespace_advance(prev_atom);
                            min_width = min_width.max(running_min_width - hanging_whitespace);
                            running_min_width = 0.0;
                            if boundary == Boundary::Mandatory {
                                max_width = max_width.max(running_max_width - hanging_whitespace);
                                running_max_width = 0.0;
                            }
                        }
                        let advance = spacing.atom_advance(&atom);
                        running_min_width += advance;
                        running_max_width += advance;
                        if !is_rtl {
                            prev_atom = Some((character.info.whitespace(), advance));
                        }
                    }
                    let hanging_whitespace = hanging_whitespace_advance(prev_atom);
                    min_width = min_width.max(running_min_width - hanging_whitespace);
                }
                LayoutItemKind::InlineBox => {
                    let ibox = &self.inline_boxes[item.index];
                    if ibox.kind == InlineBoxKind::InFlow {
                        running_max_width += ibox.width;
                        if text_wrap_mode == TextWrapMode::Wrap {
                            let hanging_whitespace = hanging_whitespace_advance(prev_atom);
                            min_width = min_width.max(running_min_width - hanging_whitespace);
                            min_width = min_width.max(ibox.width);
                            running_min_width = 0.0;
                        } else {
                            running_min_width += ibox.width;
                        }
                    }
                    prev_atom = None;
                }
            }
            let hanging_whitespace = hanging_whitespace_advance(prev_atom);
            max_width = max_width.max(running_max_width - hanging_whitespace);
        }

        let hanging_whitespace = hanging_whitespace_advance(prev_atom);
        min_width = min_width.max(running_min_width - hanging_whitespace);

        ContentWidths {
            min: min_width,
            max: max_width,
        }
    }
}
