// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping implementation using `harfrust`for shaping
//! and `icu` for text analysis.

use parley_engine::shape::{CharCluster, Coverage};
use parley_engine::{Analysis, AnalysisDataSources, FontInstance, ShapeOptions, Shaper};

use super::layout::Layout;
use super::resolve::{ResolveContext, ResolvedStyle};
use super::style::{Brush, FontFeature, FontVariation};
use crate::inline_box::InlineBox;
use crate::util::nearly_eq;
use crate::{FontContext, FontData};
use fontique::Language;

use fontique::{self, Query, QueryFamily, QueryFont};
use parlance::{BidiLevel, GenericFamily, Script};

#[allow(clippy::too_many_arguments)]
pub(crate) fn shape_text<'a, B: Brush>(
    rcx: &'a ResolveContext,
    fcx: &'a mut FontContext,
    styles: &'a [ResolvedStyle<B>],
    inline_boxes: &[InlineBox],
    analysis: &Analysis,
    char_style_indices: &[u16],
    scx: &mut Shaper,
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
            layout.data.push_inline_box(box_idx, BidiLevel::new(0));
        }
        return;
    }

    let mut fq = fcx.collection.query(&mut fcx.source_cache);

    let mut inline_box_iter = inline_boxes.iter().peekable();

    // Split when shaping-relevant style properties change and at inline boxes.
    let items = {
        let char_count = char_style_indices.len();

        // The first character of the item currently being built.
        let mut item_start_char = 0_usize;

        // Positioned at `item_start_char`, i.e., the first character not yet covered by an item.
        let mut chars = text.char_indices().enumerate().peekable();
        core::iter::from_fn(move || {
            if item_start_char == char_count {
                return None;
            }

            let item_style_index = char_style_indices[item_start_char];
            let item_style = &styles[usize::from(item_style_index)];

            // Items are at least one character long, therefore the item's own first character is
            // never a split point.
            chars.next();

            let char_end = loop {
                let Some(&(char_index, (byte_index, _))) = chars.peek() else {
                    // End of text.
                    break char_count;
                };

                // Split at inlines boxes, so each box falls on a shaping boundary.
                //
                // We loop because there may be multiple boxes at this index.
                let mut split = false;
                while let Some(inline_box) = inline_box_iter.peek() {
                    if inline_box.index < byte_index {
                        // Inline boxes *before* this index are popped (this occurs if the itemizer
                        // split a run and we were not called, such as at a bidi boundary).
                        inline_box_iter.next();
                    } else if inline_box.index == byte_index {
                        inline_box_iter.next();
                        split = true;
                    } else {
                        break;
                    }
                }

                if split {
                    break char_index;
                }

                let style_index = char_style_indices[char_index];
                if style_index != item_style_index {
                    let style = &styles[usize::from(style_index)];
                    split = !nearly_eq(style.font_size, item_style.font_size)
                        || style.locale != item_style.locale
                        || style.font_variations != item_style.font_variations
                        || style.font_features != item_style.font_features
                        || !nearly_eq(style.letter_spacing, item_style.letter_spacing)
                        || !nearly_eq(style.word_spacing, item_style.word_spacing);
                }

                if split {
                    break char_index;
                }

                chars.next();
            };

            item_start_char = char_end;
            Some(parley_engine::itemize::Item_ {
                char_end: char_end as u32,
                options: ShapeOptions {
                    language: item_style.locale,
                    font_size: item_style.font_size,
                    features: rcx.features(item_style.font_features).unwrap_or(&[]),
                    variations: rcx.variations(item_style.font_variations).unwrap_or(&[]),
                    char_style_indices,
                },
            })
        })
    };

    let font_selector = FontSelector::new(
        &mut fq,
        rcx,
        styles,
        char_style_indices,
        analysis_data_sources,
    );

    scx.shape_text(
        text,
        analysis,
        items,
        font_selector,
        &mut layout.data.shaped_text,
    );

    let mut inline_box_iter = inline_boxes.iter().enumerate().peekable();
    for shaped_run_idx in 0..layout.data.shaped_text.runs().len() {
        let shaped_run = &layout.data.shaped_text.runs()[shaped_run_idx];
        let run_text_byte_start = shaped_run.range.byte_range.start;
        let run_style_index = char_style_indices[shaped_run.range.char_range.start];
        let run_style = &styles[usize::from(run_style_index)];

        // Push inline boxes positioned before the start of this item.
        //
        // TODO: this lets the inline box take the bidi level of the previous run, but in principle
        // inline boxes should be included in bidi analysis as an object replacement character
        // (U+FFFC). The box should then take the bidi level of that character.
        let prev_bidi_level = if shaped_run_idx > 0 {
            layout.data.shaped_text.runs()[&shaped_run_idx - 1].bidi_level
        } else {
            BidiLevel::new(0)
        };
        while let Some((box_idx, inline_box)) = inline_box_iter.peek() {
            if inline_box.index <= run_text_byte_start {
                layout.data.push_inline_box(*box_idx, prev_bidi_level);
                inline_box_iter.next();
            } else {
                break;
            }
        }

        layout.data.process_shaped_run(
            shaped_run_idx,
            // TODO: should we get the *item's* style here?
            //
            // Using the run's style for word and letter spacing is probably correct (and in fact,
            // we probably shouldn't itemize on them; we should just ensure we don't ligate if
            // they're non-zero).
            run_style,
            run_style.word_spacing,
            run_style.letter_spacing,
        );
    }

    // Process any remaining inline boxes whose index is greater than the length of the text
    //
    // Give the box the same bidi level as the last text run (or else default to 0 if there is no
    // text run).
    let bidi_level = layout
        .data
        .shaped_text
        .runs()
        .last()
        .map(|r| r.bidi_level)
        .unwrap_or(BidiLevel::new(0));
    for (box_idx, _inline_box) in inline_box_iter {
        layout.data.push_inline_box(box_idx, bidi_level);
    }
}

/// The font to use if a font query doesn't return any font candidates at all.
#[derive(Debug)]
enum LastResortFont {
    Unresolved,
    Resolved(FontInstance),
    Unavailable,
}

struct FontSelector<'a, 'b, B: Brush> {
    char_style_indices: &'a [u16],

    query: &'b mut Query<'a>,
    fonts_id: Option<usize>,
    rcx: &'a ResolveContext,
    styles: &'a [ResolvedStyle<B>],
    style_index: u16,
    attrs: fontique::Attributes,
    variations: &'a [FontVariation],
    features: &'a [FontFeature],

    /// The font to use if [`Self::query`] doesn't return any font.
    last_resort_font: LastResortFont,

    analysis_data_sources: &'a AnalysisDataSources,
}

impl<'a, 'b, B: Brush> parley_engine::FontSelector for FontSelector<'a, 'b, B> {
    fn begin_item(&mut self, item: &parley_engine::itemize::Item, options: &ShapeOptions<'_>) {
        self.query.set_fallbacks(fontique::FallbackKey::new(
            item.script,
            options.language.as_ref(),
        ));
    }

    fn select_font(
        &mut self,
        _item: &parley_engine::itemize::Item,
        _options: &ShapeOptions<'_>,
        cluster: &mut CharCluster,
    ) -> Option<FontInstance> {
        let style_index = cluster.style_index();
        let is_emoji = cluster.is_emoji();

        if style_index != self.style_index || is_emoji || self.fonts_id.is_none() {
            self.style_index = style_index;
            let style = &self.styles[style_index as usize];

            let fonts_id = style.font_family.id();
            let fonts = self.rcx.stack(style.font_family).unwrap_or(&[]);
            let fonts = fonts.iter().copied().map(QueryFamily::Id);
            if is_emoji {
                use core::iter::once;
                let emoji_family = QueryFamily::Generic(GenericFamily::Emoji);
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
        let mut best_coverage = Coverage::NONE;

        self.query.matches_with(|font| {
            let Some(charmap) = font.charmap() else {
                return fontique::QueryStatus::Continue;
            };

            let coverage = cluster.calculate_coverage(
                |ch| {
                    charmap
                        .map(ch)
                        .map(|g| {
                            // Any non-zero value indicates the existence of a glyph.
                            g != 0
                        })
                        .unwrap_or_default()
                },
                self.analysis_data_sources,
            );
            if coverage > best_coverage {
                selected_font = Some(SelectedFont { font: font.clone() });
                best_coverage = coverage;

                if coverage.is_complete() {
                    fontique::QueryStatus::Stop
                } else {
                    fontique::QueryStatus::Continue
                }
            } else {
                if selected_font.is_none() {
                    selected_font = Some(SelectedFont { font: font.clone() });
                }
                fontique::QueryStatus::Continue
            }
        });

        selected_font
            .map(|selected_font| selected_font.font)
            .map(|font| FontInstance {
                font: FontData {
                    data: font.blob,
                    index: font.index,
                },
                synthesis: font.synthesis,
            })
            .or_else(|| {
                if matches!(self.last_resort_font, LastResortFont::Unresolved) {
                    if let Some(font) = any_font(self.query) {
                        self.last_resort_font = LastResortFont::Resolved(FontInstance {
                            font: FontData {
                                data: font.blob,
                                index: font.index,
                            },
                            synthesis: font.synthesis,
                        });
                    } else {
                        self.last_resort_font = LastResortFont::Unavailable;
                    }

                    self.fonts_id = None;
                }

                if let LastResortFont::Resolved(ref font) = self.last_resort_font {
                    Some(font.clone())
                } else {
                    None
                }
            })
    }
}

impl<'a, 'b, B: Brush> FontSelector<'a, 'b, B> {
    /// Construct a new `FontSelector`.
    ///
    /// If `query` ends up not returning a font for a query, the `last_resort_font` is returned
    /// instead.
    fn new(
        query: &'b mut Query<'a>,
        rcx: &'a ResolveContext,
        styles: &'a [ResolvedStyle<B>],
        char_style_indices: &'a [u16],
        analysis_data_sources: &'a AnalysisDataSources,
    ) -> Self {
        let attrs = fontique::Attributes::default();

        Self {
            query,
            fonts_id: None,
            rcx,
            styles,
            style_index: 0,
            attrs,
            variations: &[],
            features: &[],
            last_resort_font: LastResortFont::Unresolved,

            char_style_indices,
            analysis_data_sources,
        }
    }
}

/// Just query for any generic family's font from the font collection.
///
/// This sets generic families on `query`, so callers need to make sure to set families back again.
/// This returns `None` if it doesn't find any fonts, but note this only checks generic families.
///
/// This can be used to have a font at hand for shaping, in case more sophisticated font querying
/// returns no fonts.
fn any_font(query: &mut Query<'_>) -> Option<QueryFont> {
    let mut found = None;

    // Try the system's generic text families.
    //
    // At this point, the font itself doesn't really matter: we mostly just need any font to work
    // with. This has a cost, as it extends our query's candidate font families with all the generic
    // families registered on the system. A better future may be to allow users to register some
    // last resort in `fontique`, or pass it directly to `parley`.
    query.set_families([
        QueryFamily::Generic(GenericFamily::SansSerif),
        QueryFamily::Generic(GenericFamily::Serif),
        QueryFamily::Generic(GenericFamily::Monospace),
    ]);
    query.matches_with(
        #[inline]
        |font: &QueryFont| {
            found = Some(font.clone());
            fontique::QueryStatus::Stop
        },
    );

    found
}

struct SelectedFont {
    font: QueryFont,
}

impl PartialEq for SelectedFont {
    fn eq(&self, other: &Self) -> bool {
        self.font.family == other.font.family && self.font.synthesis == other.font.synthesis
    }
}
