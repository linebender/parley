// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::BreakReason;
use crate::layout::data::{LayoutData, LayoutItemKind, LineData, LineItemData};
use crate::layout::spacing::is_word_separator;
use crate::style::Brush;
use parley_engine::ShapedText;

/// Alignment of a layout.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Alignment {
    /// This is [`Alignment::Left`] for LTR text and [`Alignment::Right`] for RTL text.
    #[default]
    Start,
    /// This is [`Alignment::Right`] for LTR text and [`Alignment::Left`] for RTL text.
    End,
    /// Align content to the left edge.
    ///
    /// For alignment that should be aware of text direction, use [`Alignment::Start`] or
    /// [`Alignment::End`] instead.
    Left,
    /// Align each line centered within the container.
    Center,
    /// Align content to the right edge.
    ///
    /// For alignment that should be aware of text direction, use [`Alignment::Start`] or
    /// [`Alignment::End`] instead.
    Right,
    /// Justify each line by spacing out content, except for the last line.
    Justify,
}

/// Additional options to fine tune alignment
#[derive(Debug, Clone, Copy)]
pub struct AlignmentOptions {
    /// If set to `true`, "end" and "center" alignment will apply even if the line contents are
    /// wider than the alignment width. If it is set to `false`, all overflowing lines will be
    /// [`Alignment::Start`] aligned.
    pub align_when_overflowing: bool,
}

#[expect(
    clippy::derivable_impls,
    reason = "Make default values explicit rather than relying on the implicit default value of bool"
)]
impl Default for AlignmentOptions {
    fn default() -> Self {
        Self {
            align_when_overflowing: false,
        }
    }
}

/// Align the layout.
pub(crate) fn align<B: Brush>(
    layout: &mut LayoutData<B>,
    alignment: Alignment,
    options: AlignmentOptions,
) {
    #[cfg(feature = "accesskit")]
    {
        layout.alignment = Some(alignment);
    }

    let is_rtl = layout.base_level.is_rtl();

    // Apply alignment to line items
    for line in &mut layout.lines {
        line.justification.amount_per_opportunity = 0.;

        let indent = line.indent;

        if is_rtl {
            // In RTL text, trailing whitespace is on the left. As we hang that whitespace, offset
            // the line to the left. Note: indent is not subtracted here because `free_space` below
            // already accounts for it.
            line.metrics.offset = -line.metrics.hanging_advance;
        } else {
            line.metrics.offset = indent;
        }

        // Compute free space.
        let line_width = line.metrics.inline_max_coord - line.metrics.inline_min_coord;
        let free_space = line_width - indent - line.metrics.advance + line.metrics.hanging_advance;

        if !options.align_when_overflowing && free_space <= 0.0 {
            if is_rtl {
                // In RTL text, right-align on overflow.
                line.metrics.offset += free_space;
            }
            continue;
        }

        match (alignment, is_rtl) {
            (Alignment::Left, _) | (Alignment::Start, false) | (Alignment::End, true) => {
                // Do nothing
            }
            (Alignment::Right, _) | (Alignment::Start, true) | (Alignment::End, false) => {
                line.metrics.offset += free_space;
            }
            (Alignment::Center, _) => {
                line.metrics.offset += free_space * 0.5;
            }
            (Alignment::Justify, _) => {
                // Justified alignment doesn't have any effect if free_space is negative or zero
                if free_space <= 0.0 {
                    continue;
                }

                // Justified alignment doesn't apply to the last line of a paragraph
                // (`BreakReason::None`), (`BreakReason::Explicit`) or if there are no whitespace
                // gaps to adjust. In that case, start-align, i.e., left-align for LTR text and
                // right-align for RTL text.
                if matches!(line.break_reason, BreakReason::None | BreakReason::Explicit) {
                    if is_rtl {
                        line.metrics.offset += free_space;
                    }
                    continue;
                }

                // Count the line's justification opportunities, and cache.
                let opportunities = match line.num_justification_opportunities {
                    Some(opportunities) => opportunities,
                    None => {
                        let opportunities = justification_opportunities(
                            line,
                            &layout.line_items,
                            &layout.shaped_text,
                        );
                        line.num_justification_opportunities = Some(opportunities);
                        opportunities
                    }
                };

                if opportunities == 0 {
                    if is_rtl {
                        line.metrics.offset += free_space;
                    }
                    continue;
                }

                line.justification.amount_per_opportunity = free_space / opportunities as f32;
            }
        }
    }
}

/// The number of justification opportunities on the line.
///
/// An opportunity is a [word separator](`is_word_separator`) that lies before
/// [`Justification::justification_end_cluster`](crate::layout::spacing::Justification).
fn justification_opportunities(
    line: &LineData,
    line_items: &[LineItemData],
    shaped_text: &ShapedText,
) -> u32 {
    let end_cluster = line.justification.justification_end_cluster;

    let mut opportunities = 0;
    for line_item in &line_items[line.item_range.clone()] {
        if line_item.kind != LayoutItemKind::TextRun {
            continue;
        }
        let slice = shaped_text
            .run_slice(line_item.index as u32)
            .narrow(line_item.shaped_cluster_range.clone());
        for atom in slice.atoms_start() {
            if atom.shaped_clusters_range().end > end_cluster {
                break;
            }
            if is_word_separator(atom.characters()[0].info.whitespace()) {
                opportunities += 1;
            }
        }
    }

    opportunities
}
