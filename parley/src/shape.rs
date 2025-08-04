// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping implementation using HarfBuzz (via harfrust) for shaping
//! and swash for text analysis and font selection.

use alloc::vec::Vec;

// Parley imports
use super::layout::Layout;
use super::resolve::{RangedStyle, ResolveContext, Resolved};
use super::style::{Brush, FontFeature, FontVariation};
use crate::inline_box::InlineBox;
use crate::layout::data::HarfSynthesis;
use crate::util::nearly_eq;
use crate::Font;

// External crate imports
use fontique::{self, Query, QueryFamily, QueryFont};
use harfrust;
use swash::shape::partition::Selector as _;
use swash::text::cluster::{CharCluster, CharInfo, Token};
use swash::text::{Language, Script};
use swash::{FontRef, Synthesis};

/// Capacity hint for deferred inline boxes to avoid repeated allocations
const DEFERRED_BOXES_CAPACITY: usize = 16;

/// Convert swash synthesis information to our HarfSynthesis format.
/// This extracts the bold and italic adjustments for use with harfrust.
fn synthesis_to_harf_simple(synthesis: Synthesis) -> HarfSynthesis {
    HarfSynthesis {
        bold: if synthesis.embolden() { 1.0 } else { 0.0 },
        italic: synthesis.skew().unwrap_or(0.0),
        small_caps: false,
    }
}

/// Convert a swash Tag (u32) to a harfrust Tag for OpenType feature/script handling.
fn convert_swash_tag_to_harfrust(swash_tag: u32) -> harfrust::Tag {
    harfrust::Tag::from_be_bytes(swash_tag.to_be_bytes())
}

/// Convert swash Script enum to harfrust Script for proper text shaping.
/// Maps Unicode script codes to their corresponding OpenType script tags.
fn convert_script_to_harfrust(swash_script: Script) -> harfrust::Script {
    let tag = match swash_script {
        Script::Arabic => harfrust::Tag::from_be_bytes(*b"arab"),
        Script::Latin => harfrust::Tag::from_be_bytes(*b"latn"),
        Script::Common => harfrust::Tag::from_be_bytes(*b"zyyy"),
        Script::Unknown => harfrust::Tag::from_be_bytes(*b"zzzz"),
        Script::Inherited => harfrust::Tag::from_be_bytes(*b"zinh"),
        Script::Cyrillic => harfrust::Tag::from_be_bytes(*b"cyrl"),
        Script::Greek => harfrust::Tag::from_be_bytes(*b"grek"),
        Script::Hebrew => harfrust::Tag::from_be_bytes(*b"hebr"),
        Script::Han => harfrust::Tag::from_be_bytes(*b"hani"),
        Script::Hiragana => harfrust::Tag::from_be_bytes(*b"hira"),
        Script::Katakana => harfrust::Tag::from_be_bytes(*b"kana"),
        Script::Devanagari => harfrust::Tag::from_be_bytes(*b"deva"),
        Script::Thai => harfrust::Tag::from_be_bytes(*b"thai"),
        // For unmapped scripts, default to Latin
        _ => harfrust::Tag::from_be_bytes(*b"latn"),
    };
    
    // Convert the OpenType script tag to a harfrust Script, with Latin fallback
    harfrust::Script::from_iso15924_tag(tag).unwrap_or_else(|| {
        harfrust::Script::from_iso15924_tag(harfrust::Tag::from_be_bytes(*b"latn"))
            .expect("Latin script should always be available")
    })
}

struct Item {
    style_index: u16,
    size: f32,
    script: Script,
    level: u8,
    locale: Option<Language>,
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
    levels: &[u8],
    _scx: &mut swash::shape::ShapeContext, // Not used in harfrust approach
    mut text: &str,
    layout: &mut Layout<B>,
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
        level: levels.first().copied().unwrap_or(0),
        script: infos
            .iter()
            .map(|x| x.0.script())
            .find(|&script| real_script(script))
            .unwrap_or(Script::Latin),
        locale: style.locale,
        variations: style.font_variations,
        features: style.font_features,
        word_spacing: style.word_spacing,
        letter_spacing: style.letter_spacing,
    };
    let mut char_range = 0..0;
    let mut text_range = 0..0;

    let mut inline_box_iter = inline_boxes.iter().enumerate();
    let mut current_box = inline_box_iter.next();
    let mut deferred_boxes: Vec<usize> = Vec::with_capacity(DEFERRED_BOXES_CAPACITY);

    // Define macro to shape using swash-style segmentation + harfrust shaping
    macro_rules! shape_item {
        () => {
            let item_text = &text[text_range.clone()];
            let item_infos = &infos[char_range.start..char_range.end]; // Only process current item
            let first_style_index = item_infos[0].1;
            let mut font_selector = FontSelector::new(
                &mut fq,
                rcx,
                styles,
                first_style_index,
                item.script,
                item.locale,
            );

            // Parse text into clusters (exactly like swash does) - but only for current item
            let tokens: Vec<Token> = item_text.char_indices().zip(item_infos).map(
                |((offset, ch), (info, style_index))| Token {
                    ch,
                    offset: (text_range.start + offset) as u32,
                    len: ch.len_utf8() as u8,
                    info: *info,
                    data: *style_index as u32,
                }
            ).collect();

            let mut parser = swash::text::cluster::Parser::new(item.script, tokens.into_iter());
            let mut cluster = CharCluster::new();
            
            // Reimplement swash's shape_clusters algorithm - but only for current item
            if !parser.next(&mut cluster) {
                return;  // No clusters to process
            }
            
            let mut current_font = font_selector.select_font(&mut cluster);
            
            // Main segmentation loop (based on swash shape_clusters) - only within current item
            while let Some(font) = current_font.take() {
                // Collect all clusters for this font segment
                let mut segment_clusters = vec![cluster.clone()];
                let segment_start_offset = cluster.range().start as usize - text_range.start;
                let mut segment_end_offset = cluster.range().end as usize - text_range.start;
                
                loop {
                    if !parser.next(&mut cluster) {
                        // End of current item - process final segment
                        break;
                    }
                    
                    if let Some(next_font) = font_selector.select_font(&mut cluster) {
                        if next_font != font {
                            // Font changed - end current segment, start new one
                            current_font = Some(next_font);
                            break;
                        }
                        // Same font - add to current segment
                        segment_clusters.push(cluster.clone());
                        segment_end_offset = cluster.range().end as usize - text_range.start;
                    } else {
                        // No font found - skip this cluster
                        if !parser.next(&mut cluster) {
                            break;
                        }
                    }
                }
                
                // Shape this font segment with harfrust
                let segment_text = &item_text[segment_start_offset..segment_end_offset];
                let _segment_text_range = (text_range.start + segment_start_offset)..(text_range.start + segment_end_offset);
                
                // Split segment at newlines and shape each part separately
                let mut current_offset = segment_start_offset;
                for part in segment_text.split_inclusive('\n') {
                    if part.is_empty() {
                        continue;
                    }
                    
                    let part_start = current_offset;
                    let part_end = current_offset + part.len();
                    let part_text = &item_text[part_start..part_end];
                    let part_text_range = (text_range.start + part_start)..(text_range.start + part_end);
                    
                    // Filter out only the actual newline character for shaping (but preserve for ranges)
                    let part_for_shaping = if part.ends_with('\n') {
                        &part[..part.len()-1]  // Remove trailing newline for shaping
                    } else {
                        part
                    };
                    
                    if part_for_shaping.is_empty() {
                        current_offset = part_end;
                        continue; // Skip empty parts
                    }
                    
                    current_offset = part_end;
                    
                    if let Ok(harf_font) = harfrust::FontRef::from_index(
                        font.font.blob.as_ref(),
                        font.font.index as u32
                    ) {
                        // Create harfrust shaper
                        let shaper_data = harfrust::ShaperData::new(&harf_font);
                        let mut variations: Vec<harfrust::Variation> = vec![];
                        
                        // Extract variations from swash synthesis
                        for setting in font.synthesis.variations() {
                            variations.push(harfrust::Variation { 
                                tag: convert_swash_tag_to_harfrust(setting.tag), 
                                value: setting.value 
                            });
                        }
                        
                        let instance = harfrust::ShaperInstance::from_variations(&harf_font, &variations);
                        let harf_shaper = shaper_data
                            .shaper(&harf_font)
                            .instance(Some(&instance))
                            .point_size(Some(item.size))
                            .build();
                        
                        // Prepare harfrust buffer
                        let mut buffer = harfrust::UnicodeBuffer::new();
                        
                        // Use the part for shaping (without newlines)
                        buffer.push_str(part_for_shaping);
                        
                        let direction = if item.level & 1 != 0 {
                            harfrust::Direction::RightToLeft
                        } else {
                            harfrust::Direction::LeftToRight
                        };
                        buffer.set_direction(direction);
                        
                        let script = convert_script_to_harfrust(item.script);
                        buffer.set_script(script);
                        
                            // Shape the text
                            let mut features: Vec<harfrust::Feature> = vec![];
                            
                            // Add Arabic-specific OpenType features for proper shaping
                            if item.script == Script::Arabic {
                                features.extend([
                                    // Required Arabic features
                                    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"ccmp"), 1, 0..),  // Glyph composition/decomposition
                                    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"isol"), 1, 0..),  // Isolated forms
                                    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"fina"), 1, 0..),  // Final forms
                                    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"medi"), 1, 0..),  // Medial forms
                                    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"init"), 1, 0..),  // Initial forms
                                    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"rlig"), 1, 0..),  // Required ligatures
                                    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"liga"), 1, 0..),  // Standard ligatures
                                    harfrust::Feature::new(harfrust::Tag::from_be_bytes(*b"calt"), 1, 0..),  // Contextual alternates
                                ]);
                            }
                            
                            let glyph_buffer = harf_shaper.shape(buffer, &features);
                            
                            // Calculate character range for this part within the current item
                        let char_start = char_range.start + item_text[..part_start].chars().count();
                        let char_end = char_start + part_text.chars().count();
                        let segment_char_range = char_start..char_end;
                        
                        // Extract relevant CharInfo slice for this part
                        let segment_char_start = char_start - char_range.start;
                        let segment_char_count = part_text.chars().count();
                        let segment_infos = if segment_char_start < item_infos.len() {
                            let end_idx = (segment_char_start + segment_char_count).min(item_infos.len());
                            &item_infos[segment_char_start..end_idx]
                        } else {
                            &[]
                        };
                        
                        // Push harfrust-shaped run
                        layout.data.push_run_from_harfrust(
                            Font::new(font.font.blob.clone(), font.font.index),
                            item.size,
                            synthesis_to_harf_simple(font.synthesis),
                            font.font.synthesis, // Use the original fontique synthesis from QueryFont
                            &glyph_buffer,
                            item.level,
                            item.word_spacing,
                            item.letter_spacing,
                            part_text,
                            segment_infos,
                            part_text_range,
                            segment_char_range,
                            &variations,
                        );
                    }
                }
            }
        };
    }

    // Iterate over characters in the text (same as original)
    for ((char_index, (byte_index, ch)), (info, style_index)) in
        text.char_indices().enumerate().zip(infos)
    {
        let mut break_run = false;
        let mut script = info.script();
        if !real_script(script) {
            script = item.script;
        }
        let level = levels.get(char_index).copied().unwrap_or(0);
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
        while let Some((box_idx, inline_box)) = current_box {
            if inline_box.index == byte_index {
                break_run = true;
                deferred_boxes.push(box_idx);
                current_box = inline_box_iter.next();
            } else {
                break;
            }
        }

        if break_run && !text_range.is_empty() {
            shape_item!();
            item.size = style.font_size;
            item.level = level;
            item.script = script;
            item.locale = style.locale;
            item.variations = style.font_variations;
            item.features = style.font_features;
            text_range.start = text_range.end;
            char_range.start = char_range.end;
        }

        for box_idx in deferred_boxes.drain(0..) {
            layout.data.push_inline_box(box_idx);
        }

        text_range.end += ch.len_utf8();
        char_range.end += 1;
    }

    if !text_range.is_empty() {
        shape_item!();
    }

    // Process any remaining inline boxes
    if let Some((box_idx, _inline_box)) = current_box {
        layout.data.push_inline_box(box_idx);
    }
    for (box_idx, _inline_box) in inline_box_iter {
        layout.data.push_inline_box(box_idx);
    }
}

fn real_script(script: Script) -> bool {
    script != Script::Common && script != Script::Unknown && script != Script::Inherited
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
        locale: Option<Language>,
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

        let fb_script = crate::swash_convert::script_to_fontique(script);
        let fb_language = locale.and_then(crate::swash_convert::locale_to_fontique);
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
}

impl<B: Brush> swash::shape::partition::Selector for FontSelector<'_, '_, B> {
    type SelectedFont = SelectedFont;

    fn select_font(&mut self, cluster: &mut CharCluster) -> Option<Self::SelectedFont> {
        let style_index = cluster.user_data() as u16;
        let is_emoji = cluster.info().is_emoji();
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
            use swash::text::cluster::Status as MapStatus;

            let Ok(font_ref) = skrifa::FontRef::from_index(font.blob.as_ref(), font.index) else {
                return fontique::QueryStatus::Continue;
            };

            let charmap = font_ref.charmap();
            let map_status = cluster.map(|ch| {
                charmap
                    .map(ch)
                    .map(|g| {
                        g.to_u32()
                            .try_into()
                            .expect("Swash requires u16 glyph, so we hope that the glyph fits")
                    })
                    .unwrap_or_default()
            });

            match map_status {
                MapStatus::Complete => {
                    selected_font = Some(font.into());
                    fontique::QueryStatus::Stop
                }
                MapStatus::Keep => {
                    selected_font = Some(font.into());
                    fontique::QueryStatus::Continue
                }
                MapStatus::Discard => {
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
    synthesis: Synthesis,
}

impl From<&QueryFont> for SelectedFont {
    fn from(font: &QueryFont) -> Self {
        use crate::swash_convert::synthesis_to_swash;
        Self {
            font: font.clone(),
            synthesis: synthesis_to_swash(font.synthesis),
        }
    }
}

impl PartialEq for SelectedFont {
    fn eq(&self, other: &Self) -> bool {
        self.font.family == other.font.family && self.synthesis == other.synthesis
    }
}

impl swash::shape::partition::SelectedFont for SelectedFont {
    fn font(&self) -> FontRef<'_> {
        FontRef::from_index(self.font.blob.as_ref(), self.font.index as _).unwrap()
    }

    fn id_override(&self) -> Option<[u64; 2]> {
        Some([self.font.blob.id(), self.font.index as _])
    }

    fn synthesis(&self) -> Option<Synthesis> {
        Some(self.synthesis)
    }
}


