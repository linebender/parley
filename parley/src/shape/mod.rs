// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping implementation using `harfrust`for shaping
//! and `icu` for text analysis.

use parley_engine::shape::{CharCluster, Coverage};
use parley_engine::{Analysis, AnalysisDataSources, FontInstance, ShapeOptions, Shaper};
use smallvec::SmallVec;

use super::layout::Layout;
use super::resolve::{ResolveContext, ResolvedStyle};
use super::style::{Brush, FontFeature, FontVariation};
use crate::inline_box::InlineBox;
use crate::util::{nearly_eq, nearly_zero};
use crate::{FontContext, FontData};
use fontique::Language;

use fontique::{self, Query, QueryFamily, QueryFont};
use parlance::{GenericFamily, Script, Tag};

/// If these font features are passed to the shaper, optional ligatures are not applied.
///
/// Required ligation such as is common in cursive scripts' fonts is still applied.
///
/// Following [CSS Text 3 § 7.2][css-letter-spacing], pass these features if you are going to apply
/// something like `letter-spacing`. Unfortunately, that specification doesn't say which font
/// features specifically should be disabled. Here, we disable the OpenType features for "common",
/// "discretionary" and "historic" ligatures as given by [CSS Fonts 4 § 6.4][css-fonts-ligatures].
///
/// As of writing, this list matches Gecko. Blink additionally disables contextual alternates
/// (`calt`), but that's perhaps not desirable: disabling contextual alternates potentially makes
/// text render wrong.
///
/// [css-letter-spacing]: https://www.w3.org/TR/css-text-3/#letter-spacing-property
/// [css-fonts-ligatures]: https://www.w3.org/TR/css-fonts-4/#font-variant-ligatures-prop
const OPTIONAL_LIGATURES_OFF: [FontFeature; 4] = [
    FontFeature::new(Tag::new(b"liga"), 0),
    FontFeature::new(Tag::new(b"clig"), 0),
    FontFeature::new(Tag::new(b"dlig"), 0),
    FontFeature::new(Tag::new(b"hlig"), 0),
];

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
            layout.data.push_inline_box(box_idx);
        }
        return;
    }

    let mut fq = fcx.collection.query(&mut fcx.source_cache);

    let mut inline_box_iter = inline_boxes.iter().peekable();
    let split_after = |item_range: parley_engine::itemize::TextRange| {
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

    let mut features_scratch = SmallVec::<[FontFeature; 8]>::new();
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

        let style_features = rcx.features(style.font_features).unwrap_or(&[]);
        let features = if !nearly_zero(style.letter_spacing) {
            if style_features.is_empty() {
                OPTIONAL_LIGATURES_OFF.as_slice()
            } else {
                features_scratch.clear();
                features_scratch
                    .extend(OPTIONAL_LIGATURES_OFF.iter().chain(style_features).copied());
                features_scratch.as_slice()
            }
        } else {
            style_features
        };

        let shaped_runs_range = scx.shape_item(
            text,
            analysis,
            &item,
            &ShapeOptions {
                language: style.locale,
                font_size: style.font_size,
                features,
                variations: rcx.variations(style.font_variations).unwrap_or(&[]),
                char_style_indices,
            },
            #[inline(always)]
            |char_cluster| font_selector.select_font(char_cluster, analysis_data_sources),
            &mut layout.data.shaped_text,
        );
        for shaped_run_idx in shaped_runs_range {
            let shaped_run = &layout.data.shaped_text.runs()[shaped_run_idx];
            let run_style_index = char_style_indices[shaped_run.range.char_range.start];
            let run_style = &styles[usize::from(run_style_index)];
            layout.data.process_shaped_run(
                shaped_run_idx,
                run_style,
                style.word_spacing,
                style.letter_spacing,
            );
        }
    }

    // Process any remaining inline boxes whose index is greater than the length of the text
    for (box_idx, _inline_box) in inline_box_iter {
        layout.data.push_inline_box(box_idx);
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
}

impl<'a, 'b, B: Brush> FontSelector<'a, 'b, B> {
    /// Construct a new `FontSelector`.
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
            last_resort_font: LastResortFont::Unresolved,
        }
    }

    fn select_font(
        &mut self,
        cluster: &mut CharCluster,
        analysis_data_sources: &AnalysisDataSources,
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
                analysis_data_sources,
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
