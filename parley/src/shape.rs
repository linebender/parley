// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::layout::Layout;
use super::resolve::{RangedStyle, ResolveContext, Resolved};
use super::style::{Brush, FontFeature, FontVariation};
use crate::Font;
use crate::util::nearly_eq;
use crate::layout::data::HarfSynthesis;
use fontique::QueryFamily;
use fontique::{self, Query, QueryFont};
// Keep swash for text analysis
use swash::text::cluster::{CharCluster, CharInfo};
use swash::text::{Language, Script};
// Use harfrust for shaping
use harfrust::{FontRef as HarfFontRef, ShaperData, ShaperInstance, UnicodeBuffer, Direction};

use alloc::vec::Vec;

use crate::inline_box::InlineBox;

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
    _scx: &mut swash::shape::ShapeContext, // Keep for compatibility but don't use
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
    let mut deferred_boxes: Vec<usize> = Vec::with_capacity(16);

    // Define macro to shape using harfrust
    macro_rules! shape_item {
        () => {
            let item_text = &text[text_range.clone()];
            
            if item_text.is_empty() {
                return;
            }
            
            // changed: Restoring harfrust implementation step by step
            // Use our font selector to get the best font
            let mut font_selector = FontSelector::new(
                &mut fq,
                rcx,
                styles,
                item.style_index,
                item.script,
                item.locale,
            );
            
            // For simplicity, get font for the first character info in this range
            let char_info = &infos[char_range.start].0;
            let char_style_index = infos[char_range.start].1;  // ✅ Use actual character's style index
            if let Some(selected_font) = font_selector.select_font_for_text(item_text, char_info, char_style_index) {

                // Try to create harfrust font reference
                if let Ok(harf_font) = HarfFontRef::from_index(
                    selected_font.font.blob.as_ref(), 
                    selected_font.font.index
                )
                {

                    // Create harfrust shaper
                    let shaper_data = ShaperData::new(&harf_font);
                    // ✅ Extract variations from fontique synthesis for font weight, style, etc.
                    let mut variations: Vec<harfrust::Variation> = vec![];
                    
                    
                    
                    for (tag, value) in selected_font.font.synthesis.variation_settings() {
                        variations.push(harfrust::Variation { tag: *tag, value: *value });
                    }
                    

                    

                    let instance = ShaperInstance::from_variations(&harf_font, &variations);
                    let harf_shaper = shaper_data
                        .shaper(&harf_font)
                        .instance(Some(&instance))
                        .point_size(Some(item.size))
                        .build();
                    

                    
                    // Prepare harfrust buffer
                    let mut buffer = UnicodeBuffer::new();
                    buffer.push_str(item_text);
                    buffer.set_direction(level_to_direction(item.level));
                    // TODO: buffer.set_script(script_to_harf(item.script)); // Need proper harfrust Script constants
                    // TODO: buffer.set_language(item.locale);
                    
                    // Convert features
                    let features: Vec<harfrust::Feature> = vec![]; // TODO: Convert from item.features
                    
                    // Shape the text
                    let glyph_buffer = harf_shaper.shape(buffer, &features);
                    

                    
                    // Push the shaped run to layout using our harfrust data structures
                    layout.data.push_run_from_harfrust(
                        Font::new(selected_font.font.blob.clone(), selected_font.font.index),
                        item.size,
                        synthesis_to_harf(selected_font.font.synthesis),
                        selected_font.font.synthesis,
                        &glyph_buffer,
                        item.level,
                        item.word_spacing,
                        item.letter_spacing,
                        // NEW: Pass text analysis data for proper clustering
                        item_text,
                        infos,
                        text_range.clone(),
                        char_range.clone(),
                        // ✅ Pass the actual font variations that were used for shaping
                        &variations,
                    );
                } else {
                    // Fallback to temporary stub if harfrust font creation fails
                    // TODO: Handle this case better
                }
            }
        };
    }

    // Iterate over characters in the text
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
            // ✅ Break run on ANY style change since we need proper font selection
            // This ensures FontWeight, FontStyle, and FontWidth changes trigger new runs
            break_run = true;
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

/// Convert fontique synthesis to harfrust synthesis
fn synthesis_to_harf(synthesis: fontique::Synthesis) -> HarfSynthesis {
    let result = HarfSynthesis {
        bold: if synthesis.embolden() { 1.0 } else { 0.0 },  // ✅ Extract embolden flag
        italic: synthesis.skew().unwrap_or(0.0),              // ✅ Extract skew angle for italic
        small_caps: false, // TODO: Add small caps support if fontique has it
    };
    

    
    result
}

/// Convert swash script to harfrust script (disabled for now)
fn script_to_harf(_script: Script) -> harfrust::Script {
    // DISABLED: Script setting causes compilation issues with harfrust API
    // The visual cutoff issue should be resolved by the text mapping fixes regardless
    panic!("script_to_harf should not be called when script setting is disabled")
}

/// Convert swash direction from bidi level to harfrust direction
fn level_to_direction(level: u8) -> Direction {
    if level & 1 != 0 {
        Direction::RightToLeft
    } else {
        Direction::LeftToRight
    }
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

    /// Select a font for the given text cluster - simplified for harfrust
    fn select_font_for_text(&mut self, _text: &str, _char_info: &CharInfo, style_index: u16) -> Option<SelectedFont> {
        if style_index != self.style_index {
            self.style_index = style_index;
            let style = &self.styles[style_index as usize].style;

            let fonts_id = style.font_stack.id();
            let fonts = self.rcx.stack(style.font_stack).unwrap_or(&[]);
            
            if self.fonts_id != Some(fonts_id) {
                self.query.set_families(fonts.iter().copied());
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
            selected_font = Some(SelectedFont {
                font: font.clone(),
                synthesis: synthesis_to_harf(font.synthesis),
            });
            
            return fontique::QueryStatus::Stop;
        });
        selected_font
    }
}

struct SelectedFont {
    font: QueryFont,
    synthesis: HarfSynthesis,
}

impl From<&QueryFont> for SelectedFont {
    fn from(font: &QueryFont) -> Self {
        Self {
            font: font.clone(),
            synthesis: synthesis_to_harf(font.synthesis),
        }
    }
}

impl PartialEq for SelectedFont {
    fn eq(&self, other: &Self) -> bool {
        self.font.family == other.font.family && self.synthesis == other.synthesis
    }
}
