// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{
    data::{ClusterData, LineItemData},
    Alignment, BreakReason, LayoutData,
};
use crate::style::Brush;

pub(crate) fn align<B: Brush>(
    layout: &mut LayoutData<B>,
    alignment_width: Option<f32>,
    alignment: Alignment,
    align_when_overflowing: bool,
) {
    // Whether the text base direction is right-to-left.
    let is_rtl = layout.base_level & 1 == 1;
    let alignment_width = alignment_width.unwrap_or_else(|| {
        let max_line_length = layout
            .lines
            .iter()
            .map(|line| line.metrics.advance)
            .max_by(f32::total_cmp)
            .unwrap_or(0.0);
        max_line_length.min(max_line_length)
    });

    // Apply alignment to line items
    for line in &mut layout.lines {
        // TODO: remove this field
        line.alignment = alignment;

        // Compute free space.
        let free_space = alignment_width - line.metrics.advance + line.metrics.trailing_whitespace;

        if !align_when_overflowing && free_space <= 0.0 {
            continue;
        }

        match (alignment, is_rtl) {
            (Alignment::Left, _) | (Alignment::Start, false) | (Alignment::End, true) => {
                // Do nothing
            }
            (Alignment::Right, _) | (Alignment::Start, true) | (Alignment::End, false) => {
                line.metrics.offset = free_space;
            }
            (Alignment::Middle, _) => {
                line.metrics.offset = free_space * 0.5;
            }
            (Alignment::Justified, _) => {
                // Justified alignment doesn't have any effect if free_space is negative or zero
                if free_space <= 0.0 {
                    continue;
                }

                // Justified alignment doesn't apply to the last line of a paragraph
                // (`BreakReason::None`) or if there are no whitespace gaps to adjust. In that
                // case, start-align, i.e., left-align for LTR text and right-align for RTL text.
                if line.break_reason == BreakReason::None || line.num_spaces == 0 {
                    if is_rtl {
                        line.metrics.offset = free_space;
                    }
                    continue;
                }

                let adjustment = free_space / line.num_spaces as f32;
                let mut applied = 0;
                // Iterate over text runs in the line and clusters in the text run
                //   - Iterate forwards for even bidi levels (which represent LTR runs)
                //   - Iterate backwards for odd bidi levels (which represent RTL runs)
                let line_items: &mut dyn Iterator<Item = &LineItemData> = if is_rtl {
                    &mut layout.line_items[line.item_range.clone()].iter().rev()
                } else {
                    &mut layout.line_items[line.item_range.clone()].iter()
                };
                line_items
                    .filter(|item| item.is_text_run())
                    .for_each(|line_item| {
                        let clusters = &mut layout.clusters[line_item.cluster_range.clone()];
                        let line_item_is_rtl = line_item.bidi_level & 1 != 0;
                        let clusters: &mut dyn Iterator<Item = &mut ClusterData> =
                            if line_item_is_rtl {
                                &mut clusters.iter_mut().rev()
                            } else {
                                &mut clusters.iter_mut()
                            };
                        clusters.for_each(|cluster| {
                            if applied == line.num_spaces {
                                return;
                            }
                            if cluster.info.whitespace().is_space_or_nbsp() {
                                cluster.advance += adjustment;
                                applied += 1;
                            }
                        });
                    });
            }
        }

        if is_rtl {
            // In RTL text, trailing whitespace is on the left. As we hang that whitespace, offset
            // the line to the left.
            line.metrics.offset -= line.metrics.trailing_whitespace;
        }
    }
}

/// Removes previous justification applied to clusters.
/// This is part of resetting state in preparation for re-linebreaking the same layout.
pub(crate) fn unjustify<B: Brush>(layout: &mut LayoutData<B>) {
    // Whether the text base direction is right-to-left.
    let is_rtl = layout.base_level & 1 == 1;

    for line in &layout.lines {
        if line.alignment == Alignment::Justified
            && line.max_advance.is_finite()
            && line.max_advance < f32::MAX
        {
            let extra = line.max_advance - line.metrics.advance + line.metrics.trailing_whitespace;
            if line.break_reason != BreakReason::None && line.num_spaces != 0 {
                let adjustment = extra / line.num_spaces as f32;
                let mut applied = 0;

                let line_items: &mut dyn Iterator<Item = &LineItemData> = if is_rtl {
                    &mut layout.line_items[line.item_range.clone()].iter().rev()
                } else {
                    &mut layout.line_items[line.item_range.clone()].iter()
                };
                for line_run in line_items.filter(|item| item.is_text_run()) {
                    if line_run.bidi_level & 1 != 0 {
                        for cluster in layout.clusters[line_run.cluster_range.clone()]
                            .iter_mut()
                            .rev()
                        {
                            if applied == line.num_spaces {
                                break;
                            }
                            if cluster.info.whitespace().is_space_or_nbsp() {
                                cluster.advance -= adjustment;
                                applied += 1;
                            }
                        }
                    } else {
                        for cluster in layout.clusters[line_run.cluster_range.clone()].iter_mut() {
                            if applied == line.num_spaces {
                                break;
                            }
                            if cluster.info.whitespace().is_space_or_nbsp() {
                                cluster.advance -= adjustment;
                                applied += 1;
                            }
                        }
                    }
                }
            }
        }
    }
}
