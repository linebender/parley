// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::BreakReason;
use super::justification::Opportunities;
use super::spacing::push_cluster_gaps;
use crate::data::LayoutData;
use crate::style::Brush;

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
    /// Justify each line by spacing out content.
    ///
    /// The last line of a paragraph is start-aligned instead, unless
    /// [`AlignmentOptions::last_line_alignment`] is set.
    Justify,
}

/// Additional options to fine tune alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignmentOptions {
    /// If set to `true`, "end" and "center" alignment will apply even if the line contents are
    /// wider than the alignment width. If it is set to `false`, all overflowing lines will be
    /// [`Alignment::Start`] aligned.
    pub align_when_overflowing: bool,
    /// The alignment of the last line of each paragraph: the last line of the layout and every
    /// line that ends in an explicit line break. This corresponds to the CSS `text-align-last`
    /// property.
    ///
    /// If set to `None`, those lines use the alignment passed to [`Layout::set_alignment`], except that
    ///  [`Alignment::Justify`] falls back to [`Alignment::Start`]. This is the behavior of
    /// `text-align-last: auto`.
    ///
    /// [`Layout::set_alignment`]: crate::Layout::set_alignment
    pub last_line_alignment: Option<Alignment>,
}

#[expect(
    clippy::derivable_impls,
    reason = "Make default values explicit rather than relying on the implicit default value of bool"
)]
impl Default for AlignmentOptions {
    fn default() -> Self {
        Self {
            align_when_overflowing: false,
            last_line_alignment: None,
        }
    }
}

/// Align the layout's lines according to [`Layout::set_alignment`].
///
/// Sets each line's offset and populates [`LayoutData::cluster_gaps`] with word/letter spacing
/// and justification. Runs over the whole layout at once because alignment falls back to the
/// widest line when no max advance was provided.
///
/// [`Layout::set_alignment`]: crate::Layout::set_alignment
pub(crate) fn align<B: Brush>(layout: &mut LayoutData<B>) {
    let alignment = layout.alignment_setting;
    let options = layout.alignment_options;
    layout.alignment = Some(alignment);
    layout.cluster_gaps.clear();
    let mut opportunities = Opportunities::default();

    let is_rtl = layout.base_level.is_rtl();

    // Apply alignment to line items
    for line in &mut layout.lines {
        let indent = line.metrics.indent;

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

        // Free space to distribute if this line is justified.
        let justify = 'justify: {
            if !options.align_when_overflowing && free_space <= 0.0 {
                if is_rtl {
                    // In RTL text, right-align on overflow.
                    line.metrics.offset += free_space;
                }
                break 'justify None;
            }

            let is_last_line =
                matches!(line.break_reason, BreakReason::None | BreakReason::Explicit);
            let line_alignment = if !is_last_line {
                alignment
            } else if let Some(last_line_alignment) = options.last_line_alignment {
                last_line_alignment
            } else if alignment == Alignment::Justify {
                Alignment::Start
            } else {
                alignment
            };

            match (line_alignment, is_rtl) {
                (Alignment::Left, _) | (Alignment::Start, false) | (Alignment::End, true) => None,
                (Alignment::Right, _) | (Alignment::Start, true) | (Alignment::End, false) => {
                    line.metrics.offset += free_space;
                    None
                }
                (Alignment::Center, _) => {
                    line.metrics.offset += free_space * 0.5;
                    None
                }
                (Alignment::Justify, _) => {
                    // Overflowing lines cannot be justified; start-align them instead.
                    if free_space <= 0.0 {
                        if is_rtl {
                            line.metrics.offset += free_space;
                        }
                        None
                    } else {
                        Some(free_space)
                    }
                }
            }
        };

        if justify.is_none() && !line.has_spacing {
            continue;
        }
        let items = &mut layout.line_items[line.item_range];
        let line_gaps_start = layout.cluster_gaps.len();
        for item in items.iter_mut() {
            item.gaps_start = None;
            let spacing = layout.runs[item.index].spacing;
            // On a justified line, every item needs gap entries because `Opportunities::justify`
            // indexes `cluster_gaps` contiguously across the line's items.
            if justify.is_none() && spacing.is_zero() {
                continue;
            }
            let slice = layout
                .shaped_text
                .run_slice(item.index as u32)
                .narrow(item.shaped_cluster_range.into());
            item.gaps_start = Some(layout.cluster_gaps.len() as u32);
            push_cluster_gaps(&mut layout.cluster_gaps, slice, spacing, item.is_rtl());
        }
        if let Some(free_space) = justify {
            opportunities.find(&layout.shaped_text, line, items);
            let justified =
                opportunities.justify(&mut layout.cluster_gaps[line_gaps_start..], free_space);
            // Without justification opportunities, fall back to start alignment.
            if !justified && is_rtl {
                line.metrics.offset += free_space;
            }
        }
    }
}
