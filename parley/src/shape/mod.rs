// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping implementation using `harfrust`for shaping
//! and `icu` for text analysis.

use parley_core::shape::{CharCluster, Status};
use parley_core::{Analysis, AnalysisDataSources, FontInstance, ShapeContext, ShapeOptions};

use super::layout::Layout;
use super::resolve::{ResolveContext, ResolvedStyle};
use super::style::{Brush, FontFeature, FontVariation};
use crate::FontData;
use crate::inline_box::InlineBox;
use crate::util::nearly_eq;
use fontique::Language;

use fontique::{self, Query, QueryFamily, QueryFont};
use parlance::Script;

#[allow(clippy::too_many_arguments)]
pub(crate) fn shape_text<'a, B: Brush>(
    rcx: &'a ResolveContext,
    mut fq: Query<'a>,
    styles: &'a [ResolvedStyle<B>],
    inline_boxes: &[InlineBox],
    analysis: &Analysis,
    char_style_indices: &[u16],
    scx: &mut ShapeContext,
    mut text: &str,
    layout: &mut Layout<B>,
    analysis_data_sources: &AnalysisDataSources,
) {
    // If we have both empty text and no inline boxes, shape with a fake space
    // to generate metrics that can be used to size a cursor.
    if text.is_empty() && inline_boxes.is_empty() {
        text = " ";
    }
    // Do nothing if there is no text or styles (there should always be a default style)
    if text.is_empty() || styles.is_empty() {
        // Process any remaining inline boxes whose index is greater than the length of the text
        for box_idx in 0..inline_boxes.len() {
            // Push the box to the list of items
            layout.data.push_inline_box(box_idx);
        }
        return;
    }

    let mut inline_box_iter = inline_boxes.iter().peekable();
    let split_after = |item_range: parley_core::itemize::TextRange| {
        // Split at inlines boxes, so each box falls on a shaping boundary.
        {
            let mut split = false;
            // We loop because there may be multiple boxes at this index.
            while let Some(inline_box) = inline_box_iter.peek() {
                if inline_box.index < item_range.byte_range.end {
                    // Inline boxes *before* this index are popped (this occurs if the itemizer
                    // split a run and we were not called, such as at a bidi boundary).
                    inline_box_iter.next();
                } else if inline_box.index == item_range.byte_range.end {
                    inline_box_iter.next();
                    split = true;
                } else {
                    break;
                }
            }

            if split {
                return true;
            }
        }

        let item_style_index = char_style_indices[item_range.char_range.start];
        let style_index = char_style_indices[item_range.char_range.end];

        if style_index != item_style_index {
            let item_style = &styles[usize::from(item_style_index)];
            let style = &styles[usize::from(style_index)];
            !nearly_eq(style.font_size, item_style.font_size)
                || style.locale != item_style.locale
                || style.font_variations != item_style.font_variations
                || style.font_features != item_style.font_features
                || !nearly_eq(style.letter_spacing, item_style.letter_spacing)
                || !nearly_eq(style.word_spacing, item_style.word_spacing)
        } else {
            false
        }
    };

    let mut inline_box_iter = inline_boxes.iter().enumerate().peekable();
    for item in analysis.itemize(text, split_after) {
        // Push inline boxes positioned before the start of this item.
        while let Some((box_idx, inline_box)) = inline_box_iter.peek() {
            if inline_box.index <= item.range.byte_range.start {
                layout.data.push_inline_box(*box_idx);
                inline_box_iter.next();
            } else {
                break;
            }
        }

        let style_index = char_style_indices[item.range.char_range.start];
        let style = &styles[usize::from(style_index)];
        let mut font_selector =
            FontSelector::new(&mut fq, rcx, styles, style_index, item.script, style.locale);

        scx.shape_item(
            text,
            analysis,
            &item,
            &ShapeOptions {
                language: style.locale,
                font_size: style.font_size,
                features: rcx.features(style.font_features).unwrap_or(&[]),
                variations: rcx.variations(style.font_variations).unwrap_or(&[]),
                char_style_indices,
            },
            |char_cluster| {
                font_selector
                    .select_font(char_cluster, analysis_data_sources)
                    .map(|selected| FontInstance {
                        blob: selected.font.blob,
                        index: selected.font.index,
                        synthesis: selected.font.synthesis,
                    })
            },
            analysis_data_sources,
            |shaped_run| {
                let run_style_index = char_style_indices[shaped_run.range.char_range.start];
                let run_style = &styles[usize::from(run_style_index)];
                let segment_char_info = &analysis.char_info()[shaped_run.range.char_range.clone()];
                let segment_char_style_indices =
                    &char_style_indices[shaped_run.range.char_range.clone()];
                layout.data.push_run(
                    FontData::new(shaped_run.font.blob, shaped_run.font.index),
                    style.font_size,
                    fontique::Attributes {
                        width: run_style.font_width,
                        weight: run_style.font_weight,
                        style: run_style.font_style,
                    },
                    shaped_run.font.synthesis,
                    shaped_run.glyph_buffer,
                    item.bidi_level,
                    run_style_index,
                    style.word_spacing,
                    style.letter_spacing,
                    &text[shaped_run.range.byte_range.clone()],
                    segment_char_info,
                    segment_char_style_indices,
                    shaped_run.range.byte_range,
                    shaped_run.coords,
                );
            },
        );
    }

    // Process any remaining inline boxes whose index is greater than the length of the text
    for (box_idx, _inline_box) in inline_box_iter {
        layout.data.push_inline_box(box_idx);
    }
}

struct FontSelector<'a, 'b, B: Brush> {
    query: &'b mut Query<'a>,
    fonts_id: Option<usize>,
    rcx: &'a ResolveContext,
    styles: &'a [ResolvedStyle<B>],
    style_index: u16,
    attrs: fontique::Attributes,
    variations: &'a [FontVariation],
    features: &'a [FontFeature],
}

impl<'a, 'b, B: Brush> FontSelector<'a, 'b, B> {
    fn new(
        query: &'b mut Query<'a>,
        rcx: &'a ResolveContext,
        styles: &'a [ResolvedStyle<B>],
        style_index: u16,
        script: Script,
        locale: Option<Language>,
    ) -> Self {
        let style = &styles[style_index as usize];
        let fonts_id = style.font_family.id();
        let fonts = rcx.stack(style.font_family).unwrap_or(&[]);
        let attrs = fontique::Attributes {
            width: style.font_width,
            weight: style.font_weight,
            style: style.font_style,
        };
        let variations = rcx.variations(style.font_variations).unwrap_or(&[]);
        let features = rcx.features(style.font_features).unwrap_or(&[]);
        query.set_families(fonts.iter().copied());

        query.set_fallbacks(fontique::FallbackKey::new(script, locale.as_ref()));
        query.set_attributes(attrs);

        Self {
            query,
            fonts_id: Some(fonts_id),
            rcx,
            styles,
            style_index,
            attrs,
            variations,
            features,
        }
    }

    fn select_font(
        &mut self,
        cluster: &mut CharCluster,
        analysis_data_sources: &AnalysisDataSources,
    ) -> Option<SelectedFont> {
        let style_index = cluster.style_index();
        let is_emoji = cluster.is_emoji;
        if style_index != self.style_index || is_emoji || self.fonts_id.is_none() {
            self.style_index = style_index;
            let style = &self.styles[style_index as usize];

            let fonts_id = style.font_family.id();
            let fonts = self.rcx.stack(style.font_family).unwrap_or(&[]);
            let fonts = fonts.iter().copied().map(QueryFamily::Id);
            if is_emoji {
                use core::iter::once;
                let emoji_family = QueryFamily::Generic(fontique::GenericFamily::Emoji);
                self.query.set_families(fonts.chain(once(emoji_family)));
                self.fonts_id = None;
            } else if self.fonts_id != Some(fonts_id) {
                self.query.set_families(fonts);
                self.fonts_id = Some(fonts_id);
            }

            let attrs = fontique::Attributes {
                width: style.font_width,
                weight: style.font_weight,
                style: style.font_style,
            };
            if self.attrs != attrs {
                self.query.set_attributes(attrs);
                self.attrs = attrs;
            }
            self.variations = self.rcx.variations(style.font_variations).unwrap_or(&[]);
            self.features = self.rcx.features(style.font_features).unwrap_or(&[]);
        }
        let mut selected_font = None;
        self.query.matches_with(|font| {
            let Some(charmap) = font.charmap() else {
                return fontique::QueryStatus::Continue;
            };

            let map_status = cluster.map(
                |ch| {
                    charmap
                        .map(ch)
                        .map(|g| {
                            // HACK: in reality, we're only computing coverage, so
                            // we only care about whether the font  has a mapping
                            // for a particular glyph. Any non-zero value indicates
                            // the existence of a glyph so we can simplify this
                            // without a fallible conversion from u32 to u16.
                            (g != 0) as u16
                        })
                        .unwrap_or_default()
                },
                analysis_data_sources,
            );

            match map_status {
                Status::Complete => {
                    selected_font = Some(SelectedFont { font: font.clone() });
                    fontique::QueryStatus::Stop
                }
                Status::Keep => {
                    selected_font = Some(SelectedFont { font: font.clone() });
                    fontique::QueryStatus::Continue
                }
                Status::Discard => {
                    if selected_font.is_none() {
                        selected_font = Some(SelectedFont { font: font.clone() });
                    }
                    fontique::QueryStatus::Continue
                }
            }
        });
        selected_font
    }
}

struct SelectedFont {
    font: QueryFont,
}

impl PartialEq for SelectedFont {
    fn eq(&self, other: &Self) -> bool {
        self.font.family == other.font.family && self.font.synthesis == other.font.synthesis
    }
}
