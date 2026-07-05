// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shaping of text.

use alloc::vec::Vec;
use core::mem;
use harfrust::ShapeOptions as HarfShapeOptions;
use parlance::{FontFeature, FontVariation, Language};

use crate::{
    Analysis, AnalysisDataSources, CharInfo, ShapedRun,
    itemize::{Item, TextRange},
    lru_cache::LruCache,
    shape::{CharCluster, cache, fill_cluster_in_place},
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
    // Currently this field is not actually read, but once `parley_core` puts shaping into a
    // `ShapedText`, perhaps these tags will be copied along (so users don't have to bookkeep
    // manually).
    pub char_style_indices: &'a [u16],
}

/// The font instance to shape an item with.
#[derive(Clone, Debug, PartialEq)]
pub struct FontInstance {
    /// The font blob.
    // TODO: perhaps use `raw_resource_handle` directly; i.e., perhaps we can remove the `fontique`
    // dependency
    pub blob: fontique::Blob<u8>,
    /// The index of the font.
    pub index: u32,
    /// Font synthesis suggestions.
    // TODO: Synthesis carries more than we need, and ties us to `fontique`. We can likely drop this
    // in the future.
    pub synthesis: fontique::Synthesis,
}

/// Reusable scratch to shape [items][`Item`] into shaped text using [`Self::shape_item`].
// TODO: rename to `Shaper`
pub struct ShapeContext {
    shape_data_cache: LruCache<cache::ShapeDataKey, harfrust::ShaperData>,
    shape_instance_cache: LruCache<cache::ShapeInstanceId, harfrust::ShaperInstance>,
    shape_plan_cache: LruCache<cache::ShapePlanId, harfrust::ShapePlan>,
    unicode_buffer: Option<harfrust::UnicodeBuffer>,
    features: Vec<harfrust::Feature>,
    char_cluster: CharCluster,
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
            char_cluster: CharCluster::default(),
        }
    }
}

impl core::fmt::Debug for ShapeContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShapeContext").finish_non_exhaustive()
    }
}

impl ShapeContext {
    /// Shape an [`Item`] produced by [`Analysis::itemize`] into glyphs.
    ///
    /// The `shaped_runs` callback is called for each run of glyphs.
    ///
    /// `text` must be the same text as originally passed to create [`Analysis`]. `item` must be an
    /// [`Item`] produced by [`Analysis::itemize`].
    ///
    // TODO: Once we have a `ShapedText`, this will probably take a `&mut ShapedText` instead.
    pub fn shape_item(
        &mut self,
        text: &str,
        analysis: &Analysis,
        item: &Item,
        options: &ShapeOptions<'_>,
        select_font: impl FnMut(&mut CharCluster) -> Option<FontInstance>,
        analysis_data_sources: &AnalysisDataSources,
        shaped_runs: impl FnMut(ShapedRun<'_>),
    ) {
        shape_item(
            self,
            text,
            item,
            options,
            select_font,
            analysis.char_info(),
            options.char_style_indices,
            analysis_data_sources,
            shaped_runs,
        )
    }
}

fn shape_item(
    scx: &mut ShapeContext,
    text: &str,
    item: &Item,
    options: &ShapeOptions<'_>,
    mut select_font: impl FnMut(&mut CharCluster) -> Option<FontInstance>,
    char_info: &[CharInfo],
    char_style_indices: &[u16],
    analysis_data_sources: &AnalysisDataSources,
    mut shaped_runs: impl FnMut(ShapedRun<'_>),
) {
    let text_range = &item.range.byte_range;
    let char_range = &item.range.char_range;

    let item_text = &text[text_range.clone()];

    // Only process current item
    let item_char_info = &char_info[char_range.start..char_range.end];
    let item_char_style_indices = &char_style_indices[char_range.start..char_range.end];

    let grapheme_cluster_boundaries = analysis_data_sources
        .grapheme_segmenter()
        .segment_str(item_text);
    let mut item_infos_iter = item_char_info
        .iter()
        .copied()
        .zip(item_char_style_indices.iter().copied());
    let mut code_unit_offset_in_string = text_range.start;
    let char_cluster = &mut scx.char_cluster;

    // Build an iterator of boundaries and consume the first segment to seed the loop
    let mut boundaries_iter = grapheme_cluster_boundaries.skip(1);
    let mut last_boundary = 0_usize;
    let Some(mut current_boundary) = boundaries_iter.next() else {
        return; // No clusters
    };

    fill_cluster_in_place(
        &item_text[last_boundary..current_boundary],
        &mut item_infos_iter,
        &mut code_unit_offset_in_string,
        char_cluster,
    );

    let mut current_font = select_font(char_cluster);

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
            fill_cluster_in_place(
                &item_text[last_boundary..current_boundary],
                &mut item_infos_iter,
                &mut code_unit_offset_in_string,
                char_cluster,
            );

            if let Some(next_font) = select_font(char_cluster) {
                if next_font != font {
                    current_font = Some(next_font);
                    break;
                } else {
                    // Same font - add to current segment
                    segment_end_offset = char_cluster.range().end as usize - text_range.start;
                }
            } else {
                // No font determined, continue to next cluster
                continue;
            }
        }

        // Shape this font segment with harfrust
        let segment_text = &item_text[segment_start_offset..segment_end_offset];
        // Shape the entire segment text including newlines
        // The line breaking algorithm will handle newlines automatically

        // TODO: How do we want to handle errors like this?
        let font_ref = harfrust::FontRef::from_index(font.blob.as_ref(), font.index).unwrap();

        // Create harfrust shaper
        let shaper_data = scx
            .shape_data_cache
            .entry(cache::ShapeDataKey::new(font.blob.id(), font.index), || {
                harfrust::ShaperData::new(&font_ref)
            });
        let instance = scx.shape_instance_cache.entry(
            cache::ShapeInstanceKey::new(
                font.blob.id(),
                font.index,
                &font.synthesis,
                Some(options.variations),
            ),
            || {
                harfrust::ShaperInstance::from_variations(
                    &font_ref,
                    variations_iter(&font.synthesis, Some(options.variations)),
                )
            },
        );

        let direction = if item.bidi_level & 1 != 0 {
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
                font.blob.id(),
                font.index,
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

        // Extract relevant CharInfo slice for this segment
        let char_start = char_range.start + item_text[..segment_start_offset].chars().count();
        let segment_char_count = segment_text.chars().count();

        shaped_runs(ShapedRun {
            range: TextRange {
                byte_range: (item.range.byte_range.start + segment_start_offset)
                    ..(item.range.byte_range.start + segment_end_offset),
                char_range: char_start..char_start + segment_char_count,
            },
            font: font.clone(),
            glyph_buffer: &glyph_buffer,
            coords: harf_shaper.coords(),
        });

        // Replace buffer to reuse allocation in next iteration.
        scx.unicode_buffer = Some(glyph_buffer.clear());
    }
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
                    tag: harfrust::Tag::new(&variation.tag.to_bytes()),
                    value: variation.value,
                }),
        )
}

pub(crate) fn script_to_harfrust(script: fontique::Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&script.to_bytes()))
        .unwrap_or(harfrust::script::UNKNOWN)
}
