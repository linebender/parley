// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping implementation using `harfrust`for shaping
//! and `icu` for text analysis.

use core::mem;
use core::ops::RangeInclusive;

use alloc::vec::Vec;

use super::layout::Layout;
use super::resolve::{RangedStyle, ResolveContext, Resolved};
use super::style::{Brush, FontFeature, FontVariation};
use crate::analysis::cluster::{Char, CharCluster, Status};
use crate::analysis::{AnalysisDataSources, CharInfo};
use crate::inline_box::InlineBox;
use crate::lru_cache::LruCache;
use crate::util::nearly_eq;
use crate::{FontData, icu_convert};
use icu_properties::props::Script;
use icu_provider::prelude::icu_locale_core::LanguageIdentifier;

use fontique::{self, Query, QueryFamily, QueryFont};

mod cache;

pub(crate) struct ShapeContext {
    shape_data_cache: LruCache<cache::ShapeDataKey, harfrust::ShaperData>,
    shape_instance_cache: LruCache<cache::ShapeInstanceId, harfrust::ShaperInstance>,
    shape_plan_cache: LruCache<cache::ShapePlanId, harfrust::ShapePlan>,
    unicode_buffer: Option<harfrust::UnicodeBuffer>,
    features: Vec<harfrust::Feature>,
    scratch_string: String,
}

impl Default for ShapeContext {
    fn default() -> Self {
        const MAX_ENTRIES: usize = 16;
        Self {
            shape_data_cache: LruCache::new(MAX_ENTRIES),
            shape_instance_cache: LruCache::new(MAX_ENTRIES),
            shape_plan_cache: LruCache::new(MAX_ENTRIES),
            unicode_buffer: Some(harfrust::UnicodeBuffer::new()),
            features: Vec::new(),
            scratch_string: String::new(),
        }
    }
}

struct Item {
    style_index: u16,
    size: f32,
    script: Script,
    level: u8,
    locale: Option<LanguageIdentifier>,
    variations: Resolved<FontVariation>,
    features: Resolved<FontFeature>,
    word_spacing: f32,
    letter_spacing: f32,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn shape_text<'a, B: Brush>(
    rcx: &'a ResolveContext,
    mut fq: Query<'a>,
    styles: &'a [RangedStyle<B>],
    inline_boxes: &[InlineBox],
    infos: &[(CharInfo, u16)],
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

    // Setup mutable state for iteration
    let mut style = &styles[0].style;
    let mut item = Item {
        style_index: 0,
        size: style.font_size,
        level: infos[0].0.bidi_embed_level,
        script: infos
            .iter()
            .map(|x| x.0.script)
            .find(|&script| real_script(script))
            .unwrap_or(Script::Latin),
        locale: style.locale.clone(),
        variations: style.font_variations,
        features: style.font_features,
        word_spacing: style.word_spacing,
        letter_spacing: style.letter_spacing,
    };

    let mut char_range = 0..0;
    let mut text_range = 0..0;

    let mut inline_box_iter = inline_boxes.iter().enumerate();
    let mut current_box = inline_box_iter.next();

    // Iterate over characters in the text
    for ((_, (byte_index, ch)), (info, style_index)) in text.char_indices().enumerate().zip(infos) {
        let mut break_run = false;
        let mut script = info.script;
        if !real_script(script) {
            script = item.script;
        }
        let level = info.bidi_embed_level;
        if item.style_index != *style_index {
            item.style_index = *style_index;
            style = &styles[*style_index as usize].style;
            if !nearly_eq(style.font_size, item.size)
                || style.locale != item.locale
                || style.font_variations != item.variations
                || style.font_features != item.features
                || !nearly_eq(style.letter_spacing, item.letter_spacing)
                || !nearly_eq(style.word_spacing, item.word_spacing)
            {
                break_run = true;
            }
        }

        if level != item.level || script != item.script {
            break_run = true;
        }

        // Check if there is an inline box at this index
        // Note:
        //   - We loop because there may be multiple boxes at this index
        //   - We do this *before* processing the text run because we need to know whether we should
        //     break the run due to the presence of an inline box.
        let mut deferred_boxes: Option<RangeInclusive<usize>> = None;
        while let Some((box_idx, inline_box)) = current_box {
            if inline_box.index == byte_index {
                break_run = true;
                if let Some(boxes) = &mut deferred_boxes {
                    deferred_boxes = Some((*boxes.start())..=box_idx);
                } else {
                    deferred_boxes = Some(box_idx..=box_idx);
                };
                // Update the current box to the next box
                current_box = inline_box_iter.next();
            } else {
                break;
            }
        }

        if break_run && !text_range.is_empty() {
            shape_item(
                &mut fq,
                rcx,
                styles,
                &item,
                scx,
                text,
                &text_range,
                &char_range,
                infos,
                layout,
                analysis_data_sources,
            );
            item.size = style.font_size;
            item.level = level;
            item.script = script;
            item.locale = style.locale.clone();
            item.variations = style.font_variations;
            item.features = style.font_features;
            text_range.start = text_range.end;
            char_range.start = char_range.end;
        }

        if let Some(deferred_boxes) = deferred_boxes {
            for box_idx in deferred_boxes {
                layout.data.push_inline_box(box_idx);
            }
        }

        text_range.end += ch.len_utf8();
        char_range.end += 1;
    }

    if !text_range.is_empty() {
        shape_item(
            &mut fq,
            rcx,
            styles,
            &item,
            scx,
            text,
            &text_range,
            &char_range,
            infos,
            layout,
            analysis_data_sources,
        );
    }

    // Process any remaining inline boxes whose index is greater than the length of the text
    if let Some((box_idx, _inline_box)) = current_box {
        layout.data.push_inline_box(box_idx);
    }
    for (box_idx, _inline_box) in inline_box_iter {
        layout.data.push_inline_box(box_idx);
    }
}

fn is_emoji_grapheme(analysis_data_sources: &AnalysisDataSources, grapheme: &str) -> bool {
    // TODO: Optimise and test this function
    return false;
    // // TODO: Optimise this since we have `is_emoji_or_pictograph` in the composite props
    // if analysis_data_sources.basic_emoji().contains_str(grapheme) {
    //     return true;
    // }

    // let mut chars_iter = grapheme.char_indices().peekable();
    // let mut first_and_previous_char = None; // Set only for the second iteration
    // let mut has_emoji = false;
    // let mut has_zwj = false;
    // while let Some((char_index, ch)) = chars_iter.next() {
    //     // Handle single-character graphemes
    //     if char_index == 0 && chars_iter.peek().is_none() {
    //         return analysis_data_sources.emoji().contains(ch) ||
    //             analysis_data_sources.extended_pictographic().contains(ch);
    //     }

    //     // Check if this character is an emoji
    //     let emoji_data_source_contains_char = analysis_data_sources.emoji().contains(ch);
    //     if emoji_data_source_contains_char {
    //         // Check if the next character is a variation selector
    //         if let Some((_, next_ch)) = chars_iter.peek() {
    //             if analysis_data_sources.variation_selector().contains(*next_ch) {
    //                 return true;
    //             }
    //         }
    //     }

    //     // Check for flag emoji (two regional indicators), must be a two-character grapheme.
    //     if let Some(first_char) = first_and_previous_char {
    //         if chars_iter.peek().is_none() &&
    //             analysis_data_sources.regional_indicator().contains(first_char) &&
    //             analysis_data_sources.regional_indicator().contains(ch) {
    //             return true;
    //         }
    //     }

    //     // Check for ZWJ-composed emoji graphemes (e.g. 👩‍👩‍👧‍👧)
    //     if ch as u32 == 0x200D {
    //         has_zwj = true;
    //     }
    //     has_emoji |= emoji_data_source_contains_char;

    //     first_and_previous_char = if char_index == 0 { Some(ch) } else { None };
    // }

    // // If the grapheme (already segmented by icu, so it is a valid grapheme) has both emoji
    // // characters and ZWJ, it's likely an emoji ZWJ sequence.
    // if has_emoji && has_zwj {
    // }
    // has_emoji && has_zwj
}

fn shape_item<'a, B: Brush>(
    fq: &mut Query<'a>,
    rcx: &'a ResolveContext,
    styles: &'a [RangedStyle<B>],
    item: &Item,
    scx: &mut ShapeContext,
    text: &str,
    text_range: &core::ops::Range<usize>,
    char_range: &core::ops::Range<usize>,
    infos: &[(CharInfo, u16)],
    layout: &mut Layout<B>,
    analysis_data_sources: &AnalysisDataSources,
) {
    let item_text = &text[text_range.clone()];
    let item_infos = &infos[char_range.start..char_range.end]; // Only process current item
    let first_style_index = item_infos[0].1;
    let mut font_selector = FontSelector::new(
        fq,
        rcx,
        styles,
        first_style_index,
        item.script,
        item.locale.clone(),
    );

    let grapheme_cluster_boundaries = analysis_data_sources
        .grapheme_segmenter()
        .segment_str(item_text);
    let mut item_infos_iter = item_infos.iter();
    let mut code_unit_offset_in_string = text_range.start;
    let mut clusters_iter = grapheme_cluster_boundaries
        .skip(1) // First boundary index is always zero
        .scan(
            (
                0usize,
                &mut item_infos_iter,
                &mut code_unit_offset_in_string,
            ),
            |(last, item_infos_iter, code_unit_offset), boundary| {
                let segment_text = &item_text[*last..boundary];

                let mut len = 0;
                let mut map_len = 0;
                let mut force_normalize = false;
                let start = **code_unit_offset;

                let mut is_emoji_or_pictograph = false;

                let chars = segment_text
                    .char_indices()
                    .zip(item_infos_iter.by_ref())
                    .map(|((_, ch), (info, style_index))| {
                        force_normalize |= info.force_normalize;
                        len += 1;
                        map_len.saturating_add(info.contributes_to_shaping as u8);
                        **code_unit_offset += ch.len_utf8();
                        is_emoji_or_pictograph |= info.is_emoji_or_pictograph;

                        Char {
                            ch,
                            contributes_to_shaping: info.contributes_to_shaping,
                            glyph_id: 0,
                            style_index: *style_index,
                            is_control_character: info.is_control,
                        }
                    })
                    .collect();

                let end = **code_unit_offset;

                let cluster = CharCluster::new(
                    chars,
                    is_emoji_or_pictograph
                        || if (segment_text.len() > 1) {
                            is_emoji_grapheme(analysis_data_sources, segment_text)
                        } else {
                            false
                        },
                    len,
                    map_len,
                    start as u32,
                    end as u32,
                    force_normalize,
                );

                *last = boundary;
                Some(cluster)
            },
        );

    let mut cluster = clusters_iter.next().expect("one cluster");

    let mut current_font =
        font_selector.select_font(&mut cluster, analysis_data_sources, &mut scx.scratch_string);

    // Main segmentation loop (based on swash shape_clusters) - only within current item
    while let Some(font) = current_font.take() {
        // Collect all clusters for this font segment
        let cluster_range = cluster.range();
        let segment_start_offset = cluster_range.start as usize - text_range.start;
        let mut segment_end_offset = cluster_range.end as usize - text_range.start;

        loop {
            cluster = match clusters_iter.next() {
                Some(c) => c,
                None => break, // End of current item - process final segment
            };

            if let Some(next_font) = font_selector.select_font(
                &mut cluster,
                analysis_data_sources,
                &mut scx.scratch_string,
            ) {
                if next_font != font {
                    current_font = Some(next_font);
                    break;
                } else {
                    // Same font - add to current segment
                    segment_end_offset = cluster.range().end as usize - text_range.start;
                }
            } else {
                cluster = match clusters_iter.next() {
                    Some(c) => c,
                    None => break, // End of current item - process final segment
                };
            }
        }

        // Shape this font segment with harfrust
        let segment_text = &item_text[segment_start_offset..segment_end_offset];
        // Shape the entire segment text including newlines
        // The line breaking algorithm will handle newlines automatically

        // TODO: How do we want to handle errors like this?
        let font_ref =
            harfrust::FontRef::from_index(font.font.blob.as_ref(), font.font.index).unwrap();

        // Create harfrust shaper
        let shaper_data = scx.shape_data_cache.entry(
            cache::ShapeDataKey::new(font.font.blob.id(), font.font.index),
            || harfrust::ShaperData::new(&font_ref),
        );
        let instance = scx.shape_instance_cache.entry(
            cache::ShapeInstanceKey::new(
                font.font.blob.id(),
                font.font.index,
                &font.font.synthesis,
                rcx.variations(item.variations),
            ),
            || {
                harfrust::ShaperInstance::from_variations(
                    &font_ref,
                    variations_iter(&font.font.synthesis, rcx.variations(item.variations)),
                )
            },
        );

        let direction = if item.level & 1 != 0 {
            harfrust::Direction::RightToLeft
        } else {
            harfrust::Direction::LeftToRight
        };
        let script = icu_convert::script_to_harfrust(item.script);
        let language = item
            .locale
            .as_ref()
            .and_then(|lang| lang.language.as_str().parse::<harfrust::Language>().ok());
        scx.features.clear();
        for feature in rcx.features(item.features).unwrap_or(&[]) {
            scx.features.push(harfrust::Feature::new(
                harfrust::Tag::from_u32(feature.tag),
                feature.value as u32,
                ..,
            ));
        }
        let harf_shaper = shaper_data
            .shaper(&font_ref)
            .instance(Some(instance))
            .point_size(Some(item.size))
            .build();
        let shaper_plan = scx.shape_plan_cache.entry(
            cache::ShapePlanKey::new(
                font.font.blob.id(),
                font.font.index,
                &font.font.synthesis,
                direction,
                script,
                language.clone(),
                &scx.features,
                rcx.variations(item.variations),
            ),
            || {
                harfrust::ShapePlan::new(
                    &harf_shaper,
                    direction,
                    Some(script),
                    language.as_ref(),
                    &scx.features,
                )
            },
        );

        // Prepare harfrust buffer
        let mut buffer = mem::take(&mut scx.unicode_buffer).unwrap();
        buffer.clear();

        // Use the entire segment text including newlines
        buffer.reserve(segment_text.len());
        for (i, ch) in segment_text.chars().enumerate() {
            // Ensure that each cluster's index matches the index into `infos`. This is required
            // for efficient cluster lookup within `data.rs`.
            //
            // In other words, instead of using `buffer.push_str`, which iterates `segment_text`
            // with `char_indices`, push each char individually via `.chars` with a cluster index
            // that matches its `infos` counterpart. This allows us to lookup `infos` via cluster
            // index in `data.rs`.
            buffer.add(ch, i as u32);
        }

        buffer.set_direction(direction);

        buffer.set_script(script);

        if let Some(lang) = language {
            buffer.set_language(lang);
        }

        let glyph_buffer = harf_shaper.shape_with_plan(shaper_plan, buffer, &scx.features);

        // Extract relevant CharInfo slice for this segment
        let char_start = char_range.start + item_text[..segment_start_offset].chars().count();
        let segment_char_start = char_start - char_range.start;
        let segment_char_count = segment_text.chars().count();
        let segment_infos =
            &item_infos[segment_char_start..(segment_char_start + segment_char_count)];

        // Push harfrust-shaped run for the entire segment
        layout.data.push_run(
            FontData::new(font.font.blob.clone(), font.font.index),
            item.size,
            font.font.synthesis,
            &glyph_buffer,
            item.level,
            item.word_spacing,
            item.letter_spacing,
            segment_text,
            segment_infos,
            (text_range.start + segment_start_offset)..(text_range.start + segment_end_offset),
            harf_shaper.coords(),
        );

        // Replace buffer to reuse allocation in next iteration.
        scx.unicode_buffer = Some(glyph_buffer.clear());
    }
}

fn real_script(script: Script) -> bool {
    script != Script::Common && script != Script::Unknown && script != Script::Inherited
}

fn variations_iter<'a>(
    synthesis: &'a fontique::Synthesis,
    item: Option<&'a [FontVariation]>,
) -> impl Iterator<Item = harfrust::Variation> + 'a {
    synthesis
        .variation_settings()
        .iter()
        .map(|(tag, value)| harfrust::Variation {
            tag: *tag,
            value: *value,
        })
        .chain(
            item.unwrap_or(&[])
                .iter()
                .map(|variation| harfrust::Variation {
                    tag: harfrust::Tag::from_u32(variation.tag),
                    value: variation.value,
                }),
        )
}

struct FontSelector<'a, 'b, B: Brush> {
    query: &'b mut Query<'a>,
    fonts_id: Option<usize>,
    rcx: &'a ResolveContext,
    styles: &'a [RangedStyle<B>],
    style_index: u16,
    attrs: fontique::Attributes,
    variations: &'a [FontVariation],
    features: &'a [FontFeature],
}

impl<'a, 'b, B: Brush> FontSelector<'a, 'b, B> {
    fn new(
        query: &'b mut Query<'a>,
        rcx: &'a ResolveContext,
        styles: &'a [RangedStyle<B>],
        style_index: u16,
        script: Script,
        locale: Option<LanguageIdentifier>,
    ) -> Self {
        let style = &styles[style_index as usize].style;
        let fonts_id = style.font_stack.id();
        let fonts = rcx.stack(style.font_stack).unwrap_or(&[]);
        let attrs = fontique::Attributes {
            width: style.font_width,
            weight: style.font_weight,
            style: style.font_style,
        };
        let variations = rcx.variations(style.font_variations).unwrap_or(&[]);
        let features = rcx.features(style.font_features).unwrap_or(&[]);
        query.set_families(fonts.iter().copied());

        let fb_script = icu_convert::script_to_fontique(script);
        let fb_language = locale.and_then(icu_convert::locale_to_fontique);
        query.set_fallbacks(fontique::FallbackKey::new(fb_script, fb_language.as_ref()));
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
        scratch_string: &mut String,
    ) -> Option<SelectedFont> {
        let style_index = cluster.style_index();
        let is_emoji = cluster.is_emoji;
        if style_index != self.style_index || is_emoji || self.fonts_id.is_none() {
            self.style_index = style_index;
            let style = &self.styles[style_index as usize].style;

            let fonts_id = style.font_stack.id();
            let fonts = self.rcx.stack(style.font_stack).unwrap_or(&[]);
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
                scratch_string,
            );

            match map_status {
                Status::Complete => {
                    selected_font = Some(font.into());
                    fontique::QueryStatus::Stop
                }
                Status::Keep => {
                    selected_font = Some(font.into());
                    fontique::QueryStatus::Continue
                }
                Status::Discard => {
                    if selected_font.is_none() {
                        selected_font = Some(font.into());
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

impl From<&QueryFont> for SelectedFont {
    fn from(font: &QueryFont) -> Self {
        Self { font: font.clone() }
    }
}

impl PartialEq for SelectedFont {
    fn eq(&self, other: &Self) -> bool {
        self.font.family == other.font.family && self.font.synthesis == other.font.synthesis
    }
}
