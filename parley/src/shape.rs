// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping implementation using `harfrust`for shaping
//! and `swash` for text analysis.

use core::mem;
use core::ops::RangeInclusive;

use alloc::vec::Vec;
use icu::locale::LanguageIdentifier;
use icu_properties::props::{GeneralCategory, GraphemeClusterBreak, Script};
use super::layout::Layout;
use super::resolve::{RangedStyle, ResolveContext, Resolved};
use super::style::{Brush, FontFeature, FontVariation};
use crate::inline_box::InlineBox;
use crate::util::nearly_eq;
use crate::{Font, swash_convert, layout, icu_working};

use fontique::{self, Query, QueryFamily, QueryFont};
use swash::text::cluster::CharInfo;
use crate::context::AnalysisDataSources;
use crate::replace_swash::UserData;

pub(crate) struct ShapeContext {
    unicode_buffer: Option<harfrust::UnicodeBuffer>,
    features: Vec<harfrust::Feature>,
}

impl Default for ShapeContext {
    fn default() -> Self {
        Self {
            unicode_buffer: Some(harfrust::UnicodeBuffer::new()),
            features: Vec::new(),
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
    infos_icu: &[(icu_working::CharInfo, u16)],
    levels: &[u8], // TODO(conor) - remove
    scx: &mut ShapeContext,
    mut text: &str,
    layout: &mut Layout<B>,
    analysis_data_sources: &AnalysisDataSources,
) {
    println!("[BIDI] [shape_text] levels: {:?}", levels);
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
        level: levels.first().copied().unwrap_or(0),
        script: infos_icu
            .iter()
            .map(|x| { println!("script swash: {:?}", x.0.script_icu); x.0.script_icu })
            .find(|&script| real_script(script))
            .unwrap_or(Script::Latin),
        locale: style.locale.clone(),
        variations: style.font_variations,
        features: style.font_features,
        word_spacing: style.word_spacing,
        letter_spacing: style.letter_spacing,
    };
    fn print_locale(lang: Option<LanguageIdentifier>) {
        if let Some(lang) = lang {
            println!("[icu::Language] Language:{}, Script:{:?}, Region:{:?}", lang.language, lang.script, lang.region);
        } else {
            println!("[icu::Language] Language: NONE");
        }
    }
    print_locale(style.locale.clone());

    let mut char_range = 0..0;
    let mut text_range = 0..0;
    println!("creating text_range: {:?}", text_range);

    let mut inline_box_iter = inline_boxes.iter().enumerate();
    let mut current_box = inline_box_iter.next();

    // Iterate over characters in the text
    for ((char_index, (byte_index, ch)), (info, style_index)) in
        text.char_indices().enumerate().zip(infos_icu)
    {
        let mut break_run = false;
        //let mut script = infos_icu.get(char_index).unwrap().0.script_icu;
        let mut script = info.script_icu;
        if !real_script(script) {
            script = item.script;
        }
        let level_swash = levels.get(char_index).copied().unwrap_or(0);
        //let level = infos_icu.get(char_index).unwrap().0.bidi_embed_level;
        let mut level = info.bidi_embed_level;
        if level_swash != level {
            //level = level_swash;
        }
        println!("[BIDI] [shape_text] level: {}, level_swash: {}", level, level_swash);
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
            println!("calling shape_item_icu, text_range: {:?}", text_range);
            shape_item(
                &mut fq,
                rcx,
                styles,
                &item,
                scx,
                text,
                &text_range,
                &char_range,
                infos_icu,
                layout,
                analysis_data_sources,
            );
            item.size = style.font_size;
            println!("[BIDI] [shape_text], set level level: {}, level_swash: {}", level, level_swash);
            item.level = level;
            item.script = script;
            item.locale = style.locale.clone();
            item.variations = style.font_variations;
            item.features = style.font_features;
            text_range.start = text_range.end;
            println!("text_range: {:?}", text_range);
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
            infos_icu,
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
    if analysis_data_sources.basic_emoji.contains_str(grapheme) {
        return true;
    }

    // TODO(conor) consuming iterator to get char count, not optimal
    if grapheme.chars().count() == 1 {
        let ch = grapheme.chars().next().unwrap();
        return analysis_data_sources.emoji.contains(ch) ||
            analysis_data_sources.extended_pictographic.contains(ch);
    }

    // For multi-character sequences not covered by BasicEmoji:

    // Handle emojis using variation selectors (e.g. ❤︎ vs ❤️)
    let mut chars = grapheme.chars().peekable();
    while let Some(ch) = chars.next() {
        // Check if this character is an emoji
        if analysis_data_sources.emoji.contains(ch) {
            // Check if the next character is a variation selector
            if let Some(&next_ch) = chars.peek() {
                if analysis_data_sources.variation_selector.contains(next_ch) {
                    return true;
                }
            }
        }
    }

    // TODO(conor) Swash doesn't cluster these correctly in select_font, and Harfrust doesn't seem
    //  to either (rendering is incorrect), should check the latter more thoroughly though.
    // Check for flag emoji (two regional indicators)
    // TODO(conor) use iterator for this
    let chars: Vec<char> = grapheme.chars().collect();
    if chars.len() == 2 &&
        analysis_data_sources.regional_indicator.contains(chars[0]) &&
        analysis_data_sources.regional_indicator.contains(chars[1]) {
        return true;
    }

    // Check for ZWJ-composed emoji graphemes (e.g. 👩‍👩‍👧‍👧)
    let mut has_emoji = false;
    let mut has_zwj = false;

    for ch in grapheme.chars() {
        if ch as u32 == 0x200D {
            has_zwj = true;
        }
        if analysis_data_sources.emoji.contains(ch) {
            has_emoji = true;
        }
    }

    // If the grapheme (already segmented by icu, so it is a valid grapheme) has both emoji
    // characters and ZWJ, it's likely an emoji ZWJ sequence.
    has_emoji && has_zwj
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
    infos: &[(icu_working::CharInfo, u16)],
    layout: &mut Layout<B>,
    analysis_data_sources: &AnalysisDataSources,
) {
    let item_text = &text[text_range.clone()];
    println!("[shape_item ENTRY] item_text: '{}', text: '{}', text_range: {:?}, char_range: {:?}", item_text, text, text_range, char_range);
    let item_infos = &infos[char_range.start..char_range.end]; // Only process current item
    let first_style_index = item_infos[0].1;
    let mut font_selector =
        FontSelector::new(fq, rcx, styles, first_style_index, item.script, item.locale.clone());

    // ICU
    let item_infos_icu = &infos[char_range.start..char_range.end]; // Only process current item

    // TODO(conor) parse during analysis, just provide iterator
    //println!("segmenting item_text: '{}'", item_text);
    //let clusters_2 = analysis_data_sources.grapheme_segmenter.segment_str(item_text);
    // TODO(conor) as iterator
    let mut clusters = vec![];
    let mut last = 0;
    let mut code_unit_offset_in_string = text_range.start;
    let mut item_infos_icu_iter = item_infos_icu.iter();
    let grapheme_cluster_boundaries = analysis_data_sources.grapheme_segmenter.segment_str(item_text);
    //println!("Clusters for item text: {}: {:?}", item_text, clusters_2.map(|c| c.to_string()).collect::<Vec<String>>());
    for boundary in grapheme_cluster_boundaries.skip(1) { // First boundary index is always zero
        println!("boundary: {}", boundary);
        let segment_text = &item_text[last..boundary];

        // For simple single-character emojis
        //let mut chars = segment_text.chars();
        let is_emoji = {
            is_emoji_grapheme(analysis_data_sources, segment_text)
            // TODO(conor) more performant for single-chars, adopt mix of this and `is_emoji_grapheme`
            /*if chars.next().is_some() && chars.next().is_none() {
                // Exactly one character
                let ch = segment_text.chars().next().unwrap();
                basic_emoji.contains(ch)
            } else {
                // For emoji sequences, check if the string itself is an emoji
                is_emoji_grapheme(segment_text)
            }*/
        };
        println!("[icu] '{}' is_emoji: {:?}", segment_text, is_emoji);

        let mut len = 0;
        let mut map_len = 0;
        let mut force_normalize = false;
        let start = code_unit_offset_in_string;
        let chars = segment_text.char_indices().zip(item_infos_icu_iter.by_ref()).map(|((index, ch), (info, style_index))| {
            println!("[ICU CHAR->INFO] char:'{ch}' info: {info:?}, style_index:: {style_index}");

            // "Extend" break chars should be normalized first, with two exceptions
            force_normalize |= matches!(info.grapheme_cluster_break, GraphemeClusterBreak::Extend) &&
                !analysis_data_sources.variation_selector.contains(ch) &&
                ch as u32 != 0x200C; // Is not a Zero Width Non-Joiner
            // All spacing mark break chars should be normalized first.
            force_normalize |= matches!(info.grapheme_cluster_break, GraphemeClusterBreak::SpacingMark);

            // TODO(conor) compute this in analysis?
            let contributes_to_shaping = !matches!(info.general_category, GeneralCategory::Control) || (matches!(info.general_category, GeneralCategory::Format) &&
                !matches!(info.script_icu, Script::Inherited));
            map_len += contributes_to_shaping as u8;
            len += 1;

            let ch_len = ch.len_utf8();
            code_unit_offset_in_string += ch_len;

            let char = layout::replace_swash::Char {
                ch,
                len: ch_len as u8,
                offset: (start + index) as u32,
                contributes_to_shaping,
                glyph_id: 0, // TODO(conor) - correct to default to zero?
                data: *style_index as UserData, // TODO(conor) - needed?
                is_control_character: matches!(info.general_category, GeneralCategory::Control),
            };
            println!("[icu - CharCluster] Made char: {:?}", char);
            char
        }).collect();
        let end = code_unit_offset_in_string;

        let cluster_icu = layout::replace_swash::CharCluster::new(
            segment_text.to_string(),
            chars,
            is_emoji,
            len,
            map_len,
            start as u32,
            end as u32,
            force_normalize
        );

        clusters.push(cluster_icu);
        last = boundary;
    }

    let mut clusters_iter = clusters.iter_mut();
    let mut cluster = clusters_iter.next().expect("one cluster");

    println!("Calling select_font, site A");
    let mut current_font = font_selector.select_font_icu(cluster);
    println!("END site A");

    // Main segmentation loop (based on swash shape_clusters) - only within current item
    while let Some(font) = current_font.take() {
        // Collect all clusters for this font segment
        println!("[CURRENT_FONT] range: {}..{}, text_range.start: {}", cluster.range().start, cluster.range().end, text_range.start);
        let segment_start_offset = cluster.range().start as usize - text_range.start;
        println!("[CURRENT_FONT] segment_start_offset: {}", segment_start_offset);
        let mut segment_end_offset = cluster.range().end as usize - text_range.start;
        println!("[CURRENT_FONT] segment_end_offset: {}", segment_end_offset);

        loop {
            //println!("calling parser.next [2]");
            let maybe_cluster = clusters_iter.next();
            if maybe_cluster.is_none()  {
                // End of current item - process final segment
                //println!("cluster parser break [2]");
                break;
            } else {
                cluster = maybe_cluster.unwrap();
            }

            println!("Calling select_font, site B");
            if let Some(next_font) = font_selector.select_font_icu(cluster) {
                if next_font != font {
                    println!("[CURRENT_FONT] next_font");
                    current_font = Some(next_font);
                    break;
                } else {
                    // Same font - add to current segment
                    segment_end_offset = cluster.range().end as usize - text_range.start;
                    println!("[CURRENT_FONT] segment_end_offset: {}", segment_end_offset);
                }
            } else {
                let maybe_cluster = clusters_iter.next();
                if maybe_cluster.is_none()  {
                    // End of current item - process final segment
                    //println!("cluster parser break [2]");
                    break;
                } else {
                    cluster = maybe_cluster.unwrap();
                }
            }
        }

        // Shape this font segment with harfrust
        let segment_text = &item_text[segment_start_offset..segment_end_offset];
        //println!("shape_item: segment_text: '{}'", segment_text);
        // Shape the entire segment text including newlines
        // The line breaking algorithm will handle newlines automatically

        // TODO: How do we want to handle errors like this?
        let font_ref =
            harfrust::FontRef::from_index(font.font.blob.as_ref(), font.font.index).unwrap();

        // Create harfrust shaper
        // TODO: cache this upstream?
        let shaper_data = harfrust::ShaperData::new(&font_ref);
        let instance = harfrust::ShaperInstance::from_variations(
            &font_ref,
            variations_iter(&font.font.synthesis, rcx.variations(item.variations)),
        );
        // TODO: Don't create a new shaper for each segment.
        let harf_shaper = shaper_data
            .shaper(&font_ref)
            .instance(Some(&instance))
            .point_size(Some(item.size))
            .build();

        // Prepare harfrust buffer
        let mut buffer = mem::take(&mut scx.unicode_buffer).unwrap();
        buffer.clear();

        // Use the entire segment text including newlines
        buffer.reserve(segment_text.len());
        for (i, ch) in segment_text.chars().enumerate() {
            //println!("shape_item: ch: '{}'", ch);
            // Ensure that each cluster's index matches the index into `infos`. This is required
            // for efficient cluster lookup within `data.rs`.
            //
            // In other words, instead of using `buffer.push_str`, which iterates `segment_text`
            // with `char_indices`, push each char individually via `.chars` with a cluster index
            // that matches its `infos` counterpart. This allows us to lookup `infos` via cluster
            // index in `data.rs`.
            buffer.add(ch, i as u32);
        }

        let direction = if item.level & 1 != 0 {
            harfrust::Direction::RightToLeft
        } else {
            harfrust::Direction::LeftToRight
        };
        buffer.set_direction(direction);

        let script = swash_convert::script_icu_to_harfrust(item.script);
        buffer.set_script(script);

        if let Some(lang) = item.locale.clone() {
            if let Ok(harf_lang) = lang.language.as_str().parse::<harfrust::Language>() {
                println!("[LANGUAGE]: {:?}", harf_lang);
                buffer.set_language(harf_lang);
            }
        }

        scx.features.clear();
        for feature in rcx.features(item.features).unwrap_or(&[]) {
            scx.features.push(harfrust::Feature::new(
                harfrust::Tag::from_u32(feature.tag),
                feature.value as u32,
                0..buffer.len(),
            ));
        }

        let glyph_buffer = harf_shaper.shape(buffer, &scx.features);

        // Extract relevant CharInfo slice for this segment
        let char_start = char_range.start + item_text[..segment_start_offset].chars().count();
        let segment_char_start = char_start - char_range.start;
        let segment_char_count = segment_text.chars().count();
        let segment_infos =
            &item_infos[segment_char_start..(segment_char_start + segment_char_count)];

        // Push harfrust-shaped run for the entire segment
        println!("[FONT INDEX] {}", font.font.index);
        layout.data.push_run(
            Font::new(font.font.blob.clone(), font.font.index),
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

        let fb_script = swash_convert::script_to_fontique(script);
        let fb_language = locale.and_then(swash_convert::locale_icu_to_fontique);
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

    fn select_font_icu(&mut self, cluster: &mut crate::replace_swash::CharCluster) -> Option<SelectedFont> {
        println!("[select_font_icu] self.style index: {}, cluster.style_index: {}", self.style_index, cluster.style_index);
        let style_index = cluster.user_data() as u16;
        println!("[select_font_icu] cluster: '{}'", cluster.chars.iter().map(|ch| ch.ch).collect::<String>());
        let is_emoji = cluster.is_emoji;
        println!("[select_font_icu] is_emoji: '{}'", is_emoji);
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
            use skrifa::MetadataProvider;
            // use Status as MapStatus; // TODO(conor)

            let Ok(font_ref) = skrifa::FontRef::from_index(font.blob.as_ref(), font.index) else {
                return fontique::QueryStatus::Continue;
            };

            let charmap = font_ref.charmap();
            let map_status: crate::replace_swash::Status = cluster.map(|ch| {
                let glyph_id = charmap
                    .map(ch)
                    .map(|g| {
                        g.to_u32()
                            .try_into()
                            // TODO(conor) understand this error message, and is it irrelevant now?
                            .expect("Swash requires u16 glyph, so we hope that the glyph fits")
                    })
                    .unwrap_or_default();
                println!("[ICU GLYPH MAPPING] mapped char '{}' ({}) to gid:, {}", ch, ch.escape_unicode(), glyph_id);
                glyph_id
            });

            match map_status {
                crate::replace_swash::Status::Complete => {
                    selected_font = Some(font.into());
                    fontique::QueryStatus::Stop
                }
                crate::replace_swash::Status::Keep => {
                    selected_font = Some(font.into());
                    fontique::QueryStatus::Continue
                }
                crate::replace_swash::Status::Discard => {
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
