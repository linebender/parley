// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shaping of text.

use alloc::vec::Vec;
use core::mem;
use harfrust::ShapeOptions as HarfShapeOptions;
use linebender_resource_handle::FontData;
use parlance::{FontFeature, FontVariation, Language};

use crate::{
    Analysis, CharInfo, ShapedText,
    itemize::{Item, Item_, TextRange},
    lru_cache::LruCache,
    shape::{CharCluster, cache},
};

/// Shaping options for one item.
///
/// These are styling options relevant for shaping. They're styling, in that they're not derived
/// from the underlying text. When you [itemize][`Analysis::itemize`] the text, you should split the
/// text at points where these options change.
#[derive(Debug)]
pub struct ShapeOptions<'a> {
    /// The font size to shape the item with.
    pub font_size: f32,
    /// The language to shape the item with.
    pub language: Option<Language>,
    /// The font features to shape the item with.
    pub features: &'a [FontFeature],
    /// The font variations that are constant over an item.
    pub variations: &'a [FontVariation],
    /// The per-character style indices.
    // TODO: rename to something like `user_data` (s.t. we don't assume it's a style per se).
    pub char_style_indices: &'a [u16],
}

/// The font instance to shape an item with.
#[derive(Clone, Debug, PartialEq)]
pub struct FontInstance {
    /// The font.
    pub font: FontData,
    /// Font synthesis suggestions.
    // TODO: Synthesis carries more than we need, and ties us to `fontique`. We can likely change
    // this to opaque user data.
    pub synthesis: fontique::Synthesis,
}

/// Reusable scratch to shape text using [`Self::shape_text`].
pub struct Shaper {
    shape_data_cache: LruCache<cache::ShapeDataKey, harfrust::ShaperData>,
    shape_instance_cache: LruCache<cache::ShapeInstanceId, harfrust::ShaperInstance>,
    shape_plan_cache: LruCache<cache::ShapePlanId, harfrust::ShapePlan>,
    unicode_buffer: Option<harfrust::UnicodeBuffer>,
    features: Vec<harfrust::Feature>,
    char_cluster: CharCluster,
}

impl Default for Shaper {
    fn default() -> Self {
        const MAX_ENTRIES: usize = 16;
        Self {
            shape_data_cache: LruCache::new(MAX_ENTRIES),
            shape_instance_cache: LruCache::new(MAX_ENTRIES),
            shape_plan_cache: LruCache::new(MAX_ENTRIES),
            unicode_buffer: Some(harfrust::UnicodeBuffer::new()),
            features: Vec::new(),
            char_cluster: CharCluster::default(),
        }
    }
}

impl core::fmt::Debug for Shaper {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Shaper").finish_non_exhaustive()
    }
}

impl Shaper {
    /// Shape text into glyphs, overwriting `shaped_text`.
    ///
    /// The `text` passed in must be the same as used for producing `analysis`.
    ///
    /// This uses the items returned by `items`, and further itemizes the text into
    /// individually-shapeable runs of constant bidi level and script. `items` should be used to
    /// split on properties like shaping-relevant style changes (e.g., font size) or properties like
    /// language.
    ///
    /// Characters that don't have a particular script have their script resolved based on
    /// surrounding context (see [`crate::itemizer::Item::script`]).
    ///
    /// Each item is then broken into runs of maximal sequences of character clusters for which
    /// `select_font` returns the same font.
    ///
    /// # Panics
    ///
    /// Panics if `items` does not cover the entire source text, or the font returned by
    /// `select_font` is malformed.
    pub fn shape_text<'options>(
        &mut self,
        text: &str,
        analysis: &Analysis,
        items: impl IntoIterator<Item = Item_<'options>>,
        mut select_font: impl FontSelector,
        shaped_text: &mut ShapedText,
    ) {
        shaped_text.clear();
        shaped_text.reserve(text.len());

        let mut items = items.into_iter();

        let Some(mut current_item) = items.next() else {
            assert!(
                text.is_empty(),
                "`items` does not cover the entire source text"
            );
            return;
        };

        let mut itemizer = analysis.itemize(text);

        while let Some(item) = itemizer.next(
            #[inline(always)]
            |text_range| text_range.char_range.end == current_item.char_end as usize,
        ) {
            let item_char_end = item.range.char_range.end;

            if shape_item(
                self,
                text,
                &item,
                &current_item.options,
                &mut select_font,
                analysis.char_info(),
                shaped_text,
            )
            .is_err()
            {
                // Abort on error. This happens iff `FontSelector::select_font` failed to return a
                // font. By aborting we ensure `ShapedText` covers the source text contiguously (as
                // we need a font to construct `ShapedRun`).
                return;
            };

            if item_char_end == current_item.char_end as usize {
                let Some(next_item) = items.next() else {
                    // The last item's end coincides with the end of the source text, so the
                    // itemizer should be finished as well.
                    assert!(
                        itemizer.next(|_| false).is_none(),
                        "`items` does not cover the entire source text"
                    );
                    break;
                };
                current_item = next_item;
            }
        }
    }
}

/// Shape one item.
///
/// Returns `Err(())` if shaping should be aborted, which happens iff [`FontSelector::select_font`]
/// returned `None`.
fn shape_item(
    scx: &mut Shaper,
    text: &str,
    item: &Item,
    options: &ShapeOptions<'_>,
    select_font: &mut impl FontSelector,
    char_info: &[CharInfo],
    shaped_text: &mut ShapedText,
) -> Result<(), ()> {
    select_font.begin_item(item, options);

    let text_range = &item.range.byte_range;
    let char_range = &item.range.char_range;

    let item_text = &text[text_range.clone()];

    // Only process current item
    let item_char_info = &char_info[char_range.start..char_range.end];
    let item_char_style_indices = &options.char_style_indices[char_range.start..char_range.end];

    if item_text.is_empty() {
        return Ok(()); // No clusters
    }

    let mut item_infos_iter = item_char_info
        .iter()
        .copied()
        .zip(item_char_style_indices.iter().copied());
    let mut code_unit_offset_in_string = text_range.start;
    let char_cluster = &mut scx.char_cluster;

    // Build an iterator of boundaries and consume the first segment to seed the loop
    let mut boundaries_iter = item_text
        .char_indices()
        .zip(item_char_info.iter())
        .skip(1)
        .filter_map(|((byte_pos, _), info)| info.is_grapheme_start().then_some(byte_pos))
        .chain(core::iter::once(item_text.len()));
    let mut last_boundary = 0_usize;
    let mut current_boundary = boundaries_iter.next().unwrap();

    char_cluster.fill(
        &item_text[last_boundary..current_boundary],
        &mut item_infos_iter,
        &mut code_unit_offset_in_string,
    );

    let Some(next_font) = select_font.select_font(item, options, char_cluster) else {
        return Err(());
    };
    let mut current_font = Some(next_font);

    // Main segmentation loop (based on swash shape_clusters) - only within current item
    while let Some(font) = current_font.take() {
        // Collect all clusters for this font segment
        let cluster_range = char_cluster.range();
        let segment_start_offset = cluster_range.start as usize - text_range.start;
        let mut segment_end_offset = cluster_range.end as usize - text_range.start;

        for next_boundary in boundaries_iter.by_ref() {
            // Build next cluster in-place
            last_boundary = current_boundary;
            current_boundary = next_boundary;
            char_cluster.fill(
                &item_text[last_boundary..current_boundary],
                &mut item_infos_iter,
                &mut code_unit_offset_in_string,
            );

            let Some(next_font) = select_font.select_font(item, options, char_cluster) else {
                return Err(());
            };
            if next_font != font {
                current_font = Some(next_font);
                break;
            } else {
                // Same font - add to current segment
                segment_end_offset = char_cluster.range().end as usize - text_range.start;
            }
        }

        // Shape this font segment with harfrust
        let segment_text = &item_text[segment_start_offset..segment_end_offset];
        // Shape the entire segment text including newlines
        // The line breaking algorithm will handle newlines automatically

        // TODO: How do we want to handle errors like this?
        let font_ref =
            harfrust::FontRef::from_index(font.font.data.as_ref(), font.font.index).unwrap();

        // Create harfrust shaper
        let shaper_data = scx.shape_data_cache.entry(
            cache::ShapeDataKey::new(font.font.data.id(), font.font.index),
            || harfrust::ShaperData::new(&font_ref),
        );
        let instance = scx.shape_instance_cache.entry(
            cache::ShapeInstanceKey::new(
                font.font.data.id(),
                font.font.index,
                &font.synthesis,
                Some(options.variations),
            ),
            || {
                harfrust::ShaperInstance::from_variations(
                    &font_ref,
                    variations_iter(&font.synthesis, options.variations),
                )
            },
        );

        let direction = if item.bidi_level.is_rtl() {
            harfrust::Direction::RightToLeft
        } else {
            harfrust::Direction::LeftToRight
        };
        let hb_script = script_to_harfrust(item.script);
        let language = options
            .language
            .as_ref()
            .and_then(|lang| lang.language().parse::<harfrust::Language>().ok());
        scx.features.clear();
        for feature in options.features {
            scx.features.push(harfrust::Feature::new(
                harfrust::Tag::new(&feature.tag.to_bytes()),
                feature.value as u32,
                ..,
            ));
        }
        let harf_shaper = shaper_data
            .shaper(&font_ref)
            .instance(Some(instance))
            .build();
        let shaper_plan = scx.shape_plan_cache.entry(
            cache::ShapePlanKey::new(
                font.font.data.id(),
                font.font.index,
                &font.synthesis,
                direction,
                hb_script,
                language.clone(),
                &scx.features,
                Some(options.variations),
            ),
            || {
                harfrust::ShapePlan::new(
                    &harf_shaper,
                    direction,
                    Some(hb_script),
                    language.as_ref(),
                    &scx.features,
                )
            },
        );

        // Prepare harfrust buffer
        let mut buffer = mem::take(&mut scx.unicode_buffer).unwrap();
        buffer.clear();
        buffer.set_cluster_level(harfrust::BufferClusterLevel::MonotoneCharacters);

        // Use the entire segment text including newlines
        buffer.reserve(segment_text.len());
        #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
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

        buffer.set_script(hb_script);

        if let Some(lang) = language {
            buffer.set_language(lang);
        }

        let glyph_buffer = harf_shaper.shape(
            buffer,
            HarfShapeOptions::new()
                .plan(Some(shaper_plan))
                .features(&scx.features)
                .point_size(Some(options.font_size)),
        );

        let char_start = char_range.start + item_text[..segment_start_offset].chars().count();
        let segment_char_count = segment_text.chars().count();
        let range = TextRange {
            byte_range: (item.range.byte_range.start + segment_start_offset)
                ..(item.range.byte_range.start + segment_end_offset),
            char_range: char_start..char_start + segment_char_count,
        };
        shaped_text.push_run(
            text,
            range,
            item,
            options,
            char_info,
            &font,
            &glyph_buffer,
            harf_shaper.coords(),
        );

        // Replace buffer to reuse allocation in next iteration.
        scx.unicode_buffer = Some(glyph_buffer.clear());
    }

    Ok(())
}

#[inline]
fn variations_iter<'a>(
    synthesis: &'a fontique::Synthesis,
    item: &'a [FontVariation],
) -> impl Iterator<Item = harfrust::Variation> + 'a {
    synthesis
        .variation_settings()
        .iter()
        .map(|(tag, value)| harfrust::Variation {
            tag: *tag,
            value: *value,
        })
        .chain(item.iter().map(|variation| harfrust::Variation {
            tag: harfrust::Tag::new(&variation.tag.to_bytes()),
            value: variation.value,
        }))
}

pub(crate) fn script_to_harfrust(script: fontique::Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&script.to_bytes()))
        .unwrap_or(harfrust::script::UNKNOWN)
}

/// Implements font selection for shaping.
pub trait FontSelector {
    /// Called when a new item starts.
    ///
    /// This can be useful to inspect, e.g., the item's script.
    fn begin_item(&mut self, item: &Item, options: &ShapeOptions<'_>) {
        let _ = (item, options);
    }

    /// Called once per character cluster within the current segment.
    ///
    /// A character cluster will usually be a grapheme, though if text direction or script changes
    /// mid-grapheme, it will be split over segments.
    ///
    /// Shaping is aborted if this returns `None`; [`ShapedText`] then contains a partial result.
    fn select_font(
        &mut self,
        item: &Item,
        options: &ShapeOptions<'_>,
        cluster: &mut CharCluster,
    ) -> Option<FontInstance>;
}
