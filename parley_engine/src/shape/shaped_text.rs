// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Data structures to store the result of shaping.

use core::ops::Range;

use alloc::vec::Vec;
use parlance::BidiLevel;

use crate::{
    CharInfo, FontInstance, Glyph, ShapeOptions,
    itemize::{Item, TextRange},
    shape::ShapedClusterFlags,
};

use super::{
    ClusterInfo, Whitespace,
    atom::ShapedSlice,
    data::{Character, ShapedCluster},
};

/// A normalized font coordinate.
///
/// This is a 16-bit fixed-point number with a 14-bit fractional part. For font coordinates, its
/// useful values are in the range -1.0..=1.0.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NormalizedCoord(i16);

impl NormalizedCoord {
    /// Create a fixed-point value from the bit representation.
    ///
    /// The [`i16`] is interpreted as `bits / 16384`.
    #[inline(always)]
    pub fn from_bits(bits: i16) -> Self {
        Self(bits)
    }

    /// Create a fixed-point value from the bit representation.
    ///
    /// The `i16` can be interpreted as `bits / 16384`.
    #[inline(always)]
    pub fn to_bits(self) -> i16 {
        self.0
    }

    /// The value of the normalized coordinate, represented as a floating-point number.
    ///
    /// This will usually be in the inclusive range `-1.0..=1.0`, and is guaranteed to be in the
    /// half-open range `-2.0..2.0`.
    #[inline]
    pub fn to_f32(self) -> f32 {
        f32::from(self.0) / (1 << 14) as f32
    }
}

/// Metrics that apply to the glyphs of a font.
///
/// These metrics are scaled by the font size.
///
/// The `underline_*` and `strikethrough_*` fields are derived if not set by the font.
///
// TODO: Perhaps it'd be nicer if we exposed unscaled metrics (design units) instead. This currently
// just follows what `parley` used to do in its `RunMetrics`. If we go unscaled, we should then
// either also store units per em, or em-normalize the values like CSS does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    /// Distance from the baseline to the top of the alignment box.
    pub ascent: f32,
    /// Distance from the baseline to the bottom of the alignment box.
    pub descent: f32,
    /// Recommended additional spacing between lines.
    pub leading: f32,
    /// Offset of the top of underline decoration from the baseline.
    pub underline_offset: f32,
    /// Thickness of the underline decoration.
    pub underline_size: f32,
    /// Offset of the top of strikethrough decoration from the baseline.
    pub strikethrough_offset: f32,
    /// Thickness of the strikethrough decoration.
    pub strikethrough_size: f32,
    /// Distance from the baseline to the top of a typical English capital.
    pub cap_height: Option<f32>,
    /// Distance from the baseline to the top of the lowercase "x" or
    /// similar character.
    pub x_height: Option<f32>,
}

/// The result of shaping.
///
/// After [analyzing][crate::Analysis] your text, [shape the text][crate::Shaper::shape_text],
/// writing the result into this [`ShapedText`]. This then holds your shaped paragraph of text.
///
/// This shaped text holds spans of the source text's characters that were shaped into
/// [`ShapedCluster`]s. Note that the boundaries of shaped clusters and graphemes need not coincide;
/// see [`ShapedCluster`]s documentation for more information.
///
/// Use [`Self::run_slice`] to get a run's [`ShapedSlice`]. From there, you can interact with slices
/// of shaped text, such as walking [atoms](crate::Atom) (e.g., [`ShapedSlice::atoms_start`]) and
/// [graphemes](crate::Grapheme) (e.g., [`ShapedSlice::graphemes_start`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapedText {
    runs: Vec<ShapedRun>,
    characters: Vec<Character>,
    shaped_clusters: Vec<ShapedCluster>,
    glyphs: Vec<Glyph>,
    fonts: Vec<FontInstance>,
    normalized_coords: Vec<NormalizedCoord>,
}

impl ShapedText {
    /// Create a reusable [`ShapedText`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve capacity for at least `additional_chars` more characters.
    ///
    /// This speculatively reserves capacity for `additional_chars` more clusters and glyphs.
    #[inline]
    pub(crate) fn reserve(&mut self, additional_chars: usize) {
        self.characters.reserve(additional_chars);
        self.shaped_clusters.reserve(additional_chars);
        self.glyphs.reserve(additional_chars);
    }

    /// Clear the result while retaining capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.runs.clear();
        self.characters.clear();
        self.shaped_clusters.clear();
        self.glyphs.clear();
        self.fonts.clear();
        self.normalized_coords.clear();
    }

    /// Get a [`ShapedSlice`] of the run at `run_index`.
    pub fn run_slice(&self, run_index: u32) -> ShapedSlice<'_> {
        let run = &self.runs[run_index as usize];
        ShapedSlice {
            characters: &self.characters,

            shaped_clusters: &self.shaped_clusters,
            glyphs: &self.glyphs,
            bidi_level: run.bidi_level,

            clusters: (
                run.shaped_clusters_range.start,
                run.shaped_clusters_range.end,
            ),
        }
    }

    /// The shaped runs.
    ///
    /// Each run contains glyphs that can be rendered with a single font.
    #[inline(always)]
    pub fn runs(&self) -> &[ShapedRun] {
        &self.runs
    }

    /// Escape hatch for `parley` while it's still mutating advances in place (spacing and
    /// justification). This is not for public consumption!
    #[doc(hidden)]
    #[inline(always)]
    pub fn characters_shaped_clusters_and_glyphs_mut(
        &mut self,
    ) -> (&[Character], &mut [ShapedCluster], &mut [Glyph]) {
        (
            &self.characters,
            &mut self.shaped_clusters,
            &mut self.glyphs,
        )
    }

    /// Escape hatch for `parley` tests that remap style indices to check whether layouts are
    /// otherwise identical. This is not for public consumption!
    #[doc(hidden)]
    #[inline(always)]
    pub fn remap_styles(&mut self, remap: &[u16]) {
        for character in self.characters.iter_mut() {
            character.style_index = remap[character.style_index as usize];
        }
        for shaped_cluster in self.shaped_clusters.iter_mut() {
            shaped_cluster.style_index = remap[shaped_cluster.style_index as usize];
        }
    }

    /// The characters of the shaped runs, in logical order.
    ///
    /// [`ShapedRun::characters_range`] indexes into this.
    ///
    /// Each [`Character`] corresponds to one `char` of shaped source text. This only contains the
    /// characters of runs that were actually shaped into this [`ShapedText`].
    #[inline(always)]
    pub fn characters(&self) -> &[Character] {
        &self.characters
    }

    /// The shaped clusters.
    ///
    /// [`ShapedRun::shaped_clusters_range`] indexes into this.
    #[inline(always)]
    pub fn shaped_clusters(&self) -> &[ShapedCluster] {
        &self.shaped_clusters
    }

    /// The shaped glyphs.
    ///
    /// [`ShapedRun::glyphs_range`] indexes into this.
    #[inline(always)]
    pub fn glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    /// The fonts.
    ///
    /// [`ShapedRun::font_index`] indexes into this.
    #[inline(always)]
    pub fn fonts(&self) -> &[FontInstance] {
        &self.fonts
    }

    /// The normalized font coordinates used by runs in this shaped text.
    ///
    /// [`ShapedRun::normalized_coords_range`] splits this into per-run slices.
    #[inline(always)]
    pub fn normalized_coords(&self) -> &[NormalizedCoord] {
        &self.normalized_coords
    }

    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    pub(crate) fn push_run(
        &mut self,
        text: &str,
        range: TextRange,
        item: &Item,
        options: &ShapeOptions<'_>,
        char_info: &[CharInfo],
        font: &FontInstance,
        glyph_buffer: &harfrust::GlyphBuffer,
        normalized_coords: &[harfrust::NormalizedCoord],
    ) {
        let glyph_infos = glyph_buffer.glyph_infos();
        if glyph_infos.is_empty() {
            return;
        }

        let normalized_coords_range = {
            let start = self.normalized_coords.len();
            self.normalized_coords.extend(
                normalized_coords
                    .iter()
                    .map(|c| NormalizedCoord::from_bits(c.to_bits())),
            );
            start..self.normalized_coords.len()
        };

        let font_index = self
            .fonts
            .iter()
            .position(|f| f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font.clone());
                index
            });

        let metrics = {
            let font = &self.fonts[font_index];
            let font_ref =
                skrifa::FontRef::from_index(font.font.data.as_ref(), font.font.index).unwrap();
            skrifa::metrics::Metrics::new(
                &font_ref,
                skrifa::prelude::Size::new(options.font_size),
                normalized_coords,
            )
        };
        let units_per_em = metrics.units_per_em as f32;

        // TODO: The following seems to be in the wrong scale, as its staying in design units rather
        // than scaled to the font size like the other fields for `FontMetrics`.
        let (underline_offset, underline_size) = if let Some(underline) = metrics.underline {
            (underline.offset, underline.thickness)
        } else {
            // Default values from Harfbuzz: https://github.com/harfbuzz/harfbuzz/blob/00492ec7df0038f41f78d43d477c183e4e4c506e/src/hb-ot-metrics.cc#L334
            let default = units_per_em / 18.0;
            (default, default)
        };
        let (strikethrough_offset, strikethrough_size) = if let Some(strikeout) = metrics.strikeout
        {
            (strikeout.offset, strikeout.thickness)
        } else {
            // Default values from HarfBuzz: https://github.com/harfbuzz/harfbuzz/blob/00492ec7df0038f41f78d43d477c183e4e4c506e/src/hb-ot-metrics.cc#L334-L347
            (metrics.ascent / 2.0, units_per_em / 18.0)
        };

        let font_metrics = FontMetrics {
            ascent: metrics.ascent,
            descent: -metrics.descent,
            leading: metrics.leading,
            underline_offset,
            underline_size,
            strikethrough_offset,
            strikethrough_size,
            x_height: metrics.x_height,
            cap_height: metrics.cap_height,
        };

        // `HarfRust` returns glyphs in visual order, so we need to process them as such while
        // maintaining logical ordering of clusters.

        let glyph_positions = glyph_buffer.glyph_positions();
        let scale_factor = options.font_size / units_per_em;
        let shaped_clusters_start = self.shaped_clusters.len();

        // Push all characters.
        let characters_start = self.characters.len();
        for (((byte_offset, ch), info), style_index) in text[range.byte_range.clone()]
            .char_indices()
            .zip(&char_info[range.char_range.clone()])
            .zip(&options.char_style_indices[range.char_range.clone()])
        {
            self.characters.push(Character {
                text_byte_start: (range.byte_range.start + byte_offset) as u32,
                info: ClusterInfo::new(info.boundary, ch),
                style_index: *style_index,
                grapheme_start: info.is_grapheme_start(),
            });
        }
        // TODO: we force a grapheme break at the start of the run, as itemization could have split
        // a grapheme into two. Ideally, grapheme segmentation should be performed after
        // itemization, at which case this will always be true.
        self.characters[characters_start].grapheme_start = true;

        let glyphs_start = self.glyphs.len();
        if item.bidi_level.is_ltr() {
            process_shaped_clusters(
                &mut self.shaped_clusters,
                &mut self.glyphs,
                scale_factor,
                glyph_infos.iter(),
                glyph_positions.iter(),
                &options.char_style_indices[range.char_range.clone()],
                &self.characters,
                characters_start,
            );
        } else {
            process_shaped_clusters(
                &mut self.shaped_clusters,
                &mut self.glyphs,
                scale_factor,
                glyph_infos.iter().rev(),
                glyph_positions.iter().rev(),
                &options.char_style_indices[range.char_range.clone()],
                &self.characters,
                characters_start,
            );
            // Reverse each cluster's glyphs, such that they are in paint order.
            //
            // TODO: could this become a single `reverse` over the range?
            for cluster in &self.shaped_clusters[shaped_clusters_start..] {
                if !cluster.has_inline_glyph() && cluster.glyph_len() > 1 {
                    let start = cluster.glyph_offset as usize;
                    self.glyphs[start..start + cluster.glyph_len() as usize].reverse();
                }
            }
        }

        let shaped_clusters_range = shaped_clusters_start as u32..self.shaped_clusters.len() as u32;
        let run_advance = self.shaped_clusters[shaped_clusters_start..]
            .iter()
            .map(|cluster| cluster.advance)
            .sum();

        self.runs.push(ShapedRun {
            range,
            font_size: options.font_size,
            font_index,
            characters_range: characters_start as u32..self.characters.len() as u32,
            shaped_clusters_range,
            glyphs_range: glyphs_start..self.glyphs.len(),
            normalized_coords_range,
            bidi_level: item.bidi_level,
            advance: run_advance,
            font_metrics,
        });
    }
}

/// One shaped run, belonging to a [`ShapedText`].
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedRun {
    /// The range of text this run corresponds to.
    pub range: TextRange,
    /// Font size.
    pub font_size: f32,
    /// This run's font, as an index into [`ShapedText::fonts`].
    //
    // Note: We carry the font index instead of having `ShapedRun` carry `Arc<FontInstance>`,
    // meaning `ShapedRun` could become `Copy` once we have access to `core::range::Range`.
    pub font_index: usize,
    /// This run's characters, as a range into [`ShapedText::characters`].
    pub characters_range: Range<u32>,
    /// This run's shaped clusters, as a range into [`ShapedText::shaped_clusters`].
    pub shaped_clusters_range: Range<u32>,
    /// This run's glyphs, as a range into [`ShapedText::glyphs`].
    pub glyphs_range: Range<usize>,
    /// The normalized variation coords of this run, as a range into [`ShapedText::normalized_coords`].
    pub normalized_coords_range: Range<usize>,
    /// The bidi level of the run.
    pub bidi_level: BidiLevel,
    /// Total advance of the run.
    pub advance: f32,
    /// The font metrics of this run.
    pub font_metrics: FontMetrics,
}

/// Processes shaped glyphs from `HarfRust` and converts them into `ShapedCluster` and `Glyph`.
///
/// # Parameters
///
/// ## Output Parameters (mutated by this function):
/// * `shaped_clusters` - Vector where new `ShapedCluster` entries will be pushed.
/// * `glyphs` - Vector where new `Glyph` entries will be pushed. Note: single-glyph clusters
///   with zero offsets are inlined directly into `ShapedCluster`.
///
/// ## Input Parameters:
/// * `scale_factor` - Scaling factor used to convert font units to the target size.
/// * `glyph_infos` - `HarfRust` glyph information in logical order (i.e., reversed for RTL runs).
/// * `glyph_positions` - `HarfRust` glyph positioning data in logical order (i.e., reversed for RTL
///   runs).
/// * `char_style_indices` - The run's slice of per-character style indices, indexed by cluster ID.
/// * `characters` must contain the shaped characters whose clusters we're now processing, starting at
///   index `characters_start`.
/// * `characters_start` - See `characters`.
fn process_shaped_clusters<'a>(
    shaped_clusters: &mut Vec<ShapedCluster>,
    glyphs: &mut Vec<Glyph>,
    scale_factor: f32,
    glyph_infos: impl Iterator<Item = &'a harfrust::GlyphInfo>,
    glyph_positions: impl Iterator<Item = &'a harfrust::GlyphPosition>,
    char_style_indices: &[u16],
    characters: &[Character],
    characters_start: usize,
) {
    struct Cluster {
        id: u32,
        characters_start: usize,
        glyphs_start: usize,
        glyphs: u32,
        inline_glyph: Option<Glyph>,
        advance: f32,
    }

    /// Flush `cluster`, whose characters end at `char_end`, onto `shaped_clusters`.
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    fn flush(
        cluster: &mut Cluster,
        char_end: usize,
        style_index: u16,
        characters: &[Character],
        shaped_clusters: &mut Vec<ShapedCluster>,
    ) {
        let first_character = &characters[cluster.characters_start];
        let is_newline = first_character.info.whitespace() == Whitespace::Newline;
        let (glyph_offset, glyph_len, inline_glyph, advance) = if is_newline {
            // Elide glyphs of newlines.
            (cluster.glyphs_start as u32, 0, false, 0.)
        } else if let Some(inline_glyph) = cluster.inline_glyph.take() {
            (inline_glyph.id, 1, true, cluster.advance)
        } else {
            (
                cluster.glyphs_start as u32,
                cluster.glyphs as u8,
                false,
                cluster.advance,
            )
        };

        shaped_clusters.push(ShapedCluster {
            chars_range: (cluster.characters_start as u32, char_end as u32),
            style_index,
            flags: ShapedClusterFlags::new(glyph_len)
                .with_grapheme_start(first_character.grapheme_start)
                // TODO: fill with actual shaping data (`parley` currently just ignores this)
                .with_safe_to_break_before(false)
                .with_inline_glyph(inline_glyph),
            glyph_offset,
            advance,
        });
    }

    let mut cluster = Cluster {
        id: 0,
        characters_start,
        glyphs_start: glyphs.len(),
        glyphs: 0,
        inline_glyph: None,
        advance: 0.,
    };

    for (glyph_info, glyph_pos) in glyph_infos.zip(glyph_positions) {
        if glyph_info.cluster != cluster.id {
            let char_end = characters_start + glyph_info.cluster as usize;
            let style_index = char_style_indices[cluster.id as usize];
            flush(
                &mut cluster,
                char_end,
                style_index,
                characters,
                shaped_clusters,
            );

            cluster = Cluster {
                id: glyph_info.cluster,
                characters_start: char_end,
                glyphs_start: glyphs.len(),
                glyphs: 0,
                inline_glyph: None,
                advance: 0.,
            };
        }

        let glyph = Glyph {
            id: glyph_info.glyph_id,
            x: (glyph_pos.x_offset as f32) * scale_factor,
            // Convert from font space (Y-up) to layout space (Y-down)
            y: -(glyph_pos.y_offset as f32) * scale_factor,
            advance: (glyph_pos.x_advance as f32) * scale_factor,
        };
        if cluster.glyphs == 0 && glyph.x == 0. && glyph.y == 0. {
            // Defer this potential zero-offset, single glyph cluster
            cluster.inline_glyph = Some(glyph);
        } else {
            if let Some(pending_glyph) = cluster.inline_glyph.take() {
                glyphs.push(pending_glyph);
            }
            glyphs.push(glyph);
        }
        cluster.advance += glyph.advance;
        cluster.glyphs += 1;
    }

    // Flush the final cluster.
    let style_index = char_style_indices[cluster.id as usize];
    flush(
        &mut cluster,
        characters.len(),
        style_index,
        characters,
        shaped_clusters,
    );
}

#[cfg(test)]
mod tests {
    use alloc::{sync::Arc, vec, vec::Vec};

    use fontique::Synthesis;
    use linebender_resource_handle::{Blob, FontData};

    use crate::{
        Analysis, AnalysisOptions, Analyzer, FontInstance, FontSelector, ShapeOptions, Shaper,
        itemize::{Item, Item_},
        shape::CharCluster,
    };

    use super::ShapedText;

    const ROBOTO: &[u8] =
        include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");
    const NOTO_KUFI_ARABIC: &[u8] =
        include_bytes!("../../../parley_dev/assets/fonts/noto_fonts/NotoKufiArabic-Regular.otf");

    /// A [`FontSelector`] shaping everything with a single font.
    struct SingleFont(FontInstance);

    impl FontSelector for SingleFont {
        fn select_font(
            &mut self,
            _item: &Item,
            _options: &ShapeOptions<'_>,
            _cluster: &mut CharCluster,
        ) -> Option<FontInstance> {
            Some(self.0.clone())
        }
    }

    fn analyze(text: &str) -> Analysis {
        let mut analysis = Analysis::new();
        Analyzer::new().analyze(
            text,
            &AnalysisOptions {
                word_break: &[],
                line_break_override: None,
                ..AnalysisOptions::default()
            },
            &mut analysis,
        );
        analysis
    }

    fn font_instance(font_data: &'static [u8]) -> FontInstance {
        FontInstance {
            font: FontData::new(Blob::new(Arc::new(font_data)), 0),
            synthesis: Synthesis::default(),
        }
    }

    fn shape_with_font(text: &str, font_data: &'static [u8]) -> ShapedText {
        let analysis = analyze(text);
        let font = font_instance(font_data);
        let mut shaper = Shaper::default();
        let mut shaped = ShapedText::new();

        let char_style_indices = vec![0; text.chars().count()];
        let items = [Item_ {
            char_end: text.chars().count().try_into().unwrap(),
            options: ShapeOptions {
                font_size: 32.0,
                language: None,
                features: &[],
                variations: &[],
                char_style_indices: &char_style_indices,
            },
        }];
        shaper.shape_text(text, &analysis, items, SingleFont(font), &mut shaped);
        shaped
    }

    #[test]
    fn single_cluster_run_advance_matches_its_cluster() {
        let shaped = shape_with_font("A", ROBOTO);
        let run = &shaped.runs()[0];
        let cluster = &shaped.shaped_clusters()[run.shaped_clusters_range.start as usize];

        assert!(cluster.advance > 0.0);
        assert_eq!(run.advance, cluster.advance);
    }

    /// [U+0600 U+0623 U+064F] is a single grapheme in text-wide segmentation, but U+0600 has
    /// bidi class AN while the following characters resolve to a different bidi level, so
    /// itemization splits the grapheme into the items [U+0600] [U+0623 U+064F]. The item
    /// boundary forces a grapheme start on U+0623.
    #[test]
    fn one_grapheme_split_across_items() {
        let shaped = shape_with_font("\u{0600}\u{0623}\u{064F}", NOTO_KUFI_ARABIC);

        let grapheme_starts: Vec<bool> =
            shaped.characters.iter().map(|c| c.grapheme_start).collect();
        assert_eq!(grapheme_starts, [true, true, false]);

        let clusters: Vec<(u32, u32, bool)> = shaped
            .shaped_clusters
            .iter()
            .map(|c| {
                (
                    c.chars_range().start,
                    c.chars_range().end,
                    c.is_grapheme_start(),
                )
            })
            .collect();
        assert_eq!(clusters, [(0, 1, true), (1, 3, true)]);
    }

    /// "ffi" is three graphemes but shapes into one cluster.
    #[test]
    fn three_graphemes_one_cluster() {
        let shaped = shape_with_font("ffi", ROBOTO);

        let grapheme_starts: Vec<bool> =
            shaped.characters.iter().map(|c| c.grapheme_start).collect();
        assert_eq!(grapheme_starts, [true, true, true]);

        let clusters: Vec<(u32, u32, bool)> = shaped
            .shaped_clusters
            .iter()
            .map(|c| {
                (
                    c.chars_range().start,
                    c.chars_range().end,
                    c.is_grapheme_start(),
                )
            })
            .collect();
        assert_eq!(clusters, [(0, 3, true)]);
    }

    #[test]
    fn newlines_removed() {
        for newline in ["\n", "\r", "\u{2028}", "\u{2029}"] {
            let shaped = shape_with_font(&alloc::format!("a{newline}b"), ROBOTO);

            let index = shaped
                .shaped_clusters
                .iter()
                .position(|cluster| cluster.chars_range().start == 1)
                .unwrap();
            let newline_cluster = &shaped.shaped_clusters[index];
            assert_eq!(newline_cluster.glyph_len(), 0, "newline glyphs are removed");
            assert_eq!(newline_cluster.advance, 0., "newline advance is set to 0");
        }
    }

    /// An itemization boundary forces a cluster break in shaping, and must force a grapheme start.
    #[test]
    fn item_boundaries_force_grapheme_start() {
        // Two regional indicators forming a single flag grapheme...
        let text = "\u{1F1E6}\u{1F1E7}";
        let char_style_indices = vec![0; text.chars().count()];
        let analysis = analyze(text);
        assert_eq!(
            analysis
                .char_info()
                .iter()
                .map(|info| info.is_grapheme_start())
                .collect::<Vec<_>>(),
            [true, false]
        );

        let font = font_instance(ROBOTO);
        let mut shaper = Shaper::default();
        let mut shaped = ShapedText::new();

        // ...split over two items.
        let items = [
            Item_ {
                char_end: 1,
                options: ShapeOptions {
                    font_size: 32.0,
                    language: None,
                    features: &[],
                    variations: &[],
                    char_style_indices: &char_style_indices,
                },
            },
            Item_ {
                char_end: text.chars().count() as u32,
                options: ShapeOptions {
                    font_size: 32.0,
                    language: None,
                    features: &[],
                    variations: &[],
                    char_style_indices: &char_style_indices,
                },
            },
        ];
        shaper.shape_text(text, &analysis, items, SingleFont(font), &mut shaped);

        let grapheme_starts: Vec<bool> =
            shaped.characters.iter().map(|c| c.grapheme_start).collect();
        assert_eq!(grapheme_starts, [true, true]);

        // But note that, had we had three regional indicator symbols, like `[R0 R1 R2]` itemized as
        // `[R0] [R1 R2]`, the last pair should form a single grapheme. We don't do that currently.
    }
}
