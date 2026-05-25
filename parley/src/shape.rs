// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shaping driver.
//!
//! Itemization and the shaping engine itself live in [`parley_core`].

use parley_core::{Analysis, ItemizeOptions, RunOrientation, ShapeContext, ShapeInput, ShapedText};

use super::layout::Layout;
use super::resolve::{ResolveContext, ResolvedStyle};
use super::style::Brush;
use crate::analysis::CharInfo;
use crate::inline_box::InlineBox;
use crate::util::nearly_eq;
use fontique::{Attributes, Query};

#[allow(clippy::too_many_arguments)]
pub(crate) fn shape_text<'a, B: Brush>(
    rcx: &'a ResolveContext,
    mut fq: Query<'a>,
    styles: &'a [ResolvedStyle<B>],
    inline_boxes: &[InlineBox],
    infos: &[(CharInfo, u16)],
    analysis: &Analysis,
    scx: &mut ShapeContext,
    shaped: &mut ShapedText,
    mut text: &str,
    layout: &mut Layout<B>,
) {
    // If we have both empty text and no inline boxes, shape with a fake space to generate metrics
    // that can be used to size a cursor.
    if text.is_empty() && inline_boxes.is_empty() {
        text = " ";
    }

    if text.is_empty() || styles.is_empty() {
        // Process inline boxes before exiting.
        for box_idx in 0..inline_boxes.len() {
            layout.data.push_inline_box(box_idx);
        }
        return;
    }

    // Itemize via `parley_core`, breaking runs on script, bidi level, language, and orientation
    // changes. We additionally split at shaping-relevant style changes and inline boxes.
    let split_before = {
        #[inline(always)]
        |char_index: usize, byte_index: usize| -> bool {
            // Break before an inline box so it can be emitted between the surrounding runs.
            if inline_boxes
                .binary_search_by_key(&byte_index, |b| b.index)
                .is_ok()
            {
                return true;
            }
            let cur_style = infos[char_index].1;
            let prev_style = infos[char_index - 1].1;
            if cur_style == prev_style {
                return false;
            }
            // Break when a property that affects shaping differs from the previous character.
            let cur = &styles[cur_style as usize];
            let prev = &styles[prev_style as usize];
            !nearly_eq(cur.font_size, prev.font_size)
                || cur.font_family != prev.font_family
                || cur.font_weight != prev.font_weight
                || cur.font_width != prev.font_width
                || cur.font_style != prev.font_style
                || cur.locale != prev.locale
                || cur.font_variations != prev.font_variations
                || cur.font_features != prev.font_features
                || !nearly_eq(cur.letter_spacing, prev.letter_spacing)
                || !nearly_eq(cur.word_spacing, prev.word_spacing)
        }
    };

    let options = ItemizeOptions::default();
    let mut box_iter = inline_boxes.iter().enumerate().peekable();
    // Track the last family stack we pushed to the query so we can skip `set_families` when
    // adjacent items share it.
    let mut last_family = None;
    for item in analysis.itemize_with(text, &options, split_before) {
        // Emit any inline boxes at or before this item's start. Because the predicate splits at
        // every box position, each box index coincides with an item boundary.
        while let Some(&(box_idx, inline_box)) = box_iter.peek() {
            if inline_box.index <= item.text_range.start {
                layout.data.push_inline_box(box_idx);
                box_iter.next();
            } else {
                break;
            }
        }

        // Shaping parameters are constant in the item, read it from the first character's style.
        let first_style_index = infos[item.char_range.start].1;
        let style = &styles[first_style_index as usize];

        if last_family != Some(style.font_family) {
            fq.set_families(rcx.stack(style.font_family).unwrap_or(&[]).iter().copied());
            last_family = Some(style.font_family);
        }

        let input = ShapeInput {
            text,
            analysis,
            text_range: item.text_range.clone(),
            char_range: item.char_range.clone(),
            script: item.script,
            language: style.locale,
            level: item.level,
            orientation: RunOrientation::Horizontal,
            attributes: Attributes {
                width: style.font_width,
                weight: style.font_weight,
                style: style.font_style,
            },
            font_size: style.font_size,
            features: rcx.features(style.font_features).unwrap_or(&[]),
            variations: rcx.variations(style.font_variations).unwrap_or(&[]),
            letter_spacing: style.letter_spacing,
            word_spacing: style.word_spacing,
        };

        shaped.clear();
        scx.shape_run(&input, &mut fq, shaped);

        // `shape_run` appends one or more constant-font runs that tile the item's text in logical
        // order; translate each into the layout.
        let mut char_cursor = item.char_range.start;
        for run in shaped.runs() {
            layout
                .data
                .push_shaped_run(&run, text, infos, char_cursor, first_style_index);
            char_cursor += run.len();
        }
    }

    // Process any remaining inline boxes at or beyond the end of the text.
    for (box_idx, _inline_box) in box_iter {
        layout.data.push_inline_box(box_idx);
    }
}
