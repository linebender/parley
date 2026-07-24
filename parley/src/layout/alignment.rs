// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{BreakReason, data::LineItemData};
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
    layout.clear_justification();
    #[cfg(feature = "accesskit")]
    {
        layout.alignment = Some(alignment);
    }

    // Whether the text base direction is right-to-left.
    let is_rtl = layout.base_level & 1 == 1;
    let cluster_count = layout.shaped_text.clusters().len();
    let LayoutData {
        lines,
        line_items,
        shaped_text,
        justification_adjustments,
        ..
    } = layout;
    let clusters = shaped_text.clusters();

    // Apply alignment to line items
    for line in lines {
        let indent = line.indent;

        if is_rtl {
            // In RTL text, trailing whitespace is on the left. As we hang that whitespace, offset
            // the line to the left. Note: indent is not subtracted here because `free_space` below
            // already accounts for it.
            line.metrics.offset = -line.metrics.trailing_whitespace;
        } else {
            line.metrics.offset = indent;
        }

        // Compute free space.
        let line_width = line.metrics.inline_max_coord - line.metrics.inline_min_coord;
        let free_space =
            line_width - indent - line.metrics.advance + line.metrics.trailing_whitespace;

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
                if matches!(line.break_reason, BreakReason::None | BreakReason::Explicit)
                    || line.num_spaces == 0
                {
                    if is_rtl {
                        line.metrics.offset += free_space;
                    }
                    continue;
                }

                let adjustment = free_space / line.num_spaces as f32;
                let mut applied = 0;
                // Iterate over text runs in the line and clusters in the text run
                //   - Iterate forwards for even bidi levels (which represent LTR runs)
                //   - Iterate backwards for odd bidi levels (which represent RTL runs)
                let line_items: &mut dyn Iterator<Item = &mut LineItemData> = if is_rtl {
                    &mut line_items[line.item_range.clone()].iter_mut().rev()
                } else {
                    &mut line_items[line.item_range.clone()].iter_mut()
                };
                for line_item in line_items.filter(|item| item.is_text_run()) {
                    let cluster_range = line_item.cluster_range.clone();
                    let line_item_is_rtl = line_item.bidi_level & 1 != 0;
                    let cluster_indices: &mut dyn Iterator<Item = usize> = if line_item_is_rtl {
                        &mut cluster_range.rev()
                    } else {
                        &mut cluster_range.into_iter()
                    };
                    for cluster_index in cluster_indices {
                        if applied == line.num_spaces {
                            break;
                        }
                        if clusters[cluster_index].info.whitespace().is_space_or_nbsp() {
                            if justification_adjustments.is_empty() {
                                justification_adjustments.resize(cluster_count, 0.0);
                            }
                            justification_adjustments[cluster_index] = adjustment;
                            line_item.advance += adjustment;
                            applied += 1;
                        }
                    }
                    if applied == line.num_spaces {
                        break;
                    }
                }
                debug_assert_eq!(applied, line.num_spaces);
                line.metrics.advance += adjustment * applied as f32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{Alignment, AlignmentOptions};
    use crate::tests::test_builders::{FONT_FAMILY_LIST, create_font_context};
    use crate::{FontFamily, Layout, LayoutContext, PositionedLayoutItem};

    fn build_layout(text: &str) -> Layout<()> {
        let mut fcx = create_font_context();
        let mut lcx = LayoutContext::new();
        let mut builder = lcx.ranged_builder(&mut fcx, text, 1.0, true);
        builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(150.0));
        layout
    }

    #[test]
    fn justification_does_not_mutate_shaped_text() {
        let mut layout = build_layout("Lorem ipsum dolor sit amet, consectetur adipiscing elit.");
        let shaped_text = layout.data.shaped_text.clone();

        layout.align(Alignment::Justify, AlignmentOptions::default());
        assert_eq!(layout.data.shaped_text, shaped_text);

        layout.align(Alignment::Justify, AlignmentOptions::default());
        assert_eq!(layout.data.shaped_text, shaped_text);

        layout.break_all_lines(Some(150.0));
        assert_eq!(layout.data.shaped_text, shaped_text);
    }

    #[test]
    fn realignment_restores_empty_final_line_advance() {
        let mut layout = build_layout("Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n");
        let natural_advances = layout
            .lines()
            .map(|line| line.metrics().advance)
            .collect::<Vec<_>>();

        layout.align(Alignment::Justify, AlignmentOptions::default());
        layout.align(Alignment::Start, AlignmentOptions::default());

        assert_eq!(
            layout
                .lines()
                .map(|line| line.metrics().advance)
                .collect::<Vec<_>>(),
            natural_advances
        );
    }

    #[test]
    fn justified_advances_are_consistent() {
        for text in [
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
            "عند برمجة أجهزة الكمبيوتر، قد تجد نفسك فجأة في مواقف غريبة.",
        ] {
            let mut layout = build_layout(text);
            layout.align(Alignment::Justify, AlignmentOptions::default());

            for line in layout.lines() {
                let mut run_advance = 0.0;
                let mut cluster_advance = 0.0;
                for run in line.runs() {
                    run_advance += run.advance();
                    cluster_advance += run.clusters().map(|cluster| cluster.advance()).sum::<f32>();
                }
                let glyph_advance = line
                    .items()
                    .filter_map(|item| match item {
                        PositionedLayoutItem::GlyphRun(run) => Some(run.advance()),
                        PositionedLayoutItem::InlineBox(_) => None,
                    })
                    .sum::<f32>();

                for (name, advance) in [
                    ("runs", run_advance),
                    ("clusters", cluster_advance),
                    ("glyphs", glyph_advance),
                ] {
                    assert!(
                        (advance - line.metrics().advance).abs() < 0.001,
                        "{name} advance {advance} did not match line advance {}",
                        line.metrics().advance
                    );
                }
            }
        }
    }
}
