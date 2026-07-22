// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Data structures to store the result of shaping.

use core::ops::Range;

use alloc::vec::Vec;

use crate::{
    CharInfo, FontInstance, Glyph, ShapeOptions,
    itemize::{Item, TextRange},
    shape::{ClusterData, ClusterInfo, Whitespace, to_whitespace},
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
/// After [itemizing][crate::itemize::Item] your text,
/// [shape each item][crate::Shaper::shape_item], appending the result into this
/// [`ShapedText`]. This then holds your shaped paragraph of text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapedText {
    runs: Vec<ShapedRun>,
    clusters: Vec<ClusterData>,
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
    /// This speculatively reserves capacity for `additional_chars` more glyphs.
    #[inline]
    pub(crate) fn reserve(&mut self, additional_chars: usize) {
        self.clusters.reserve(additional_chars);
        self.glyphs.reserve(additional_chars);
    }

    /// Clear the result while retaining capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.runs.clear();
        self.clusters.clear();
        self.glyphs.clear();
        self.fonts.clear();
        self.normalized_coords.clear();
    }

    /// The shaped runs.
    ///
    /// Each run contains glyphs that can be rendered with a single font.
    #[inline(always)]
    pub fn runs(&self) -> &[ShapedRun] {
        &self.runs
    }

    /// The cluster data.
    ///
    /// [`ShapedRun::clusters_range`] splits this into per-run slices.
    ///
    /// Each [`ClusterData`] corresponds to one character in the source text.
    #[inline(always)]
    pub fn clusters(&self) -> &[ClusterData] {
        &self.clusters
    }

    #[doc(hidden)] // Escape hatch for `parley` while it's still mutating these in place
    #[inline(always)]
    pub fn clusters_mut(&mut self) -> &mut [ClusterData] {
        &mut self.clusters
    }

    #[doc(hidden)] // Escape hatch for `parley` while it's still mutating these in place
    #[inline(always)]
    pub fn glyphs_mut(&mut self) -> &mut [Glyph] {
        &mut self.glyphs
    }

    #[doc(hidden)] // Escape hatch for `parley` while it's still mutating these in place
    #[inline(always)]
    pub fn clusters_and_glyphs_mut(&mut self) -> (&mut [ClusterData], &mut [Glyph]) {
        (&mut self.clusters, &mut self.glyphs)
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
        let clusters_start = self.clusters.len();
        let is_rtl = !item.bidi_level.is_multiple_of(2);

        let glyphs_start = self.glyphs.len();
        if !is_rtl {
            process_clusters(
                Direction::Ltr,
                &mut self.clusters,
                &mut self.glyphs,
                scale_factor,
                glyph_infos,
                glyph_positions,
                &char_info[range.char_range.clone()],
                &options.char_style_indices[range.char_range.clone()],
                text[range.byte_range.clone()].char_indices(),
            );
        } else {
            process_clusters(
                Direction::Rtl,
                &mut self.clusters,
                &mut self.glyphs,
                scale_factor,
                glyph_infos,
                glyph_positions,
                &char_info[range.char_range.clone()],
                &options.char_style_indices[range.char_range.clone()],
                text[range.byte_range.clone()].char_indices().rev(),
            );
            // Reverse clusters into logical order for RTL
            let clusters_len = self.clusters.len();
            self.clusters[clusters_start..clusters_len].reverse();
        }

        let clusters_range = clusters_start..self.clusters.len();
        let run_advance = self.clusters[clusters_range.clone()]
            .iter()
            .map(|cluster| cluster.advance)
            .sum();

        self.runs.push(ShapedRun {
            range,
            font_size: options.font_size,
            font_index,
            clusters_range,
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
    /// This run's clusters, as a range into [`ShapedText::clusters`].
    pub clusters_range: Range<usize>,
    /// This run's glyphs, as a range into [`ShapedText::glyphs`].
    pub glyphs_range: Range<usize>,
    /// The normalized variation coords of this run, as a range into [`ShapedText::normalized_coords`].
    pub normalized_coords_range: Range<usize>,
    /// The bidi level of the run.
    pub bidi_level: u8,
    /// Total advance of the run.
    pub advance: f32,
    /// The font metrics of this run.
    pub font_metrics: FontMetrics,
}

/// Processes shaped glyphs from `HarfRust` and converts them into `ClusterData` and `Glyph`.
///
/// # Parameters
///
/// ## Output Parameters (mutated by this function):
/// * `clusters` - Vector where new `ClusterData` entries will be pushed.
/// * `glyphs` - Vector where new `Glyph` entries will be pushed. Note: single-glyph clusters
///   with zero offsets may be inlined directly into `ClusterData`.
///
/// ## Input Parameters:
/// * `direction` - Direction of the text.
/// * `scale_factor` - Scaling factor used to convert font units to the target size.
/// * `glyph_infos` - `HarfRust` glyph information in visual order.
/// * `glyph_positions` - `HarfRust` glyph positioning data in visual order.
/// * `char_infos` - Character information from text analysis, indexed by cluster ID.
/// * `char_indices_iter` - Iterator over (`byte_offset`, `char`) pairs from the source text.
///   Should be in logical order (forward for LTR, reverse for RTL).
#[expect(clippy::missing_assert_message, reason = "Deferred")]
#[expect(clippy::cast_possible_truncation, reason = "Deferred")]
fn process_clusters<I: Iterator<Item = (usize, char)>>(
    direction: Direction,
    clusters: &mut Vec<ClusterData>,
    glyphs: &mut Vec<Glyph>,
    scale_factor: f32,
    glyph_infos: &[harfrust::GlyphInfo],
    glyph_positions: &[harfrust::GlyphPosition],
    char_infos: &[CharInfo],
    char_style_indices: &[u16],
    char_indices_iter: I,
) {
    let char_info_at = |i: usize| (char_infos[i], char_style_indices[i]);
    let mut char_indices_iter = char_indices_iter.peekable();
    let mut cluster_start_char = char_indices_iter.next().unwrap();
    let mut total_glyphs: u32 = 0;
    let mut cluster_glyph_offset: u32 = 0;
    let start_cluster_id = glyph_infos.first().unwrap().cluster;
    let mut cluster_id = start_cluster_id;
    let mut char_info = char_info_at(cluster_id as usize);
    let mut cluster_advance = 0.0;
    // If the current cluster might be a single-glyph, zero-offset cluster, we defer
    // pushing the first glyph to `glyphs` because it might be inlined into `ClusterData`.
    let mut pending_inline_glyph: Option<Glyph> = None;

    // The mental model for understanding this function is best grasped by first reading
    // the HarfBuzz docs on [clusters](https://harfbuzz.github.io/working-with-harfbuzz-clusters.html).
    //
    // `num_components` is the number of characters in the current cluster. Since source text's characters
    // were inserted into `HarfRust`'s buffer using their logical indices as the cluster ID, `HarfRust` will
    // assign the first character's cluster ID (in logical order) to the merged cluster because the minimum
    // ID is selected for [merging](https://github.com/harfbuzz/harfrust/blob/a38025fb336230b492366740c86021bb406bcd0d/src/hb/buffer.rs#L920-L924).
    //
    //  So, the number of components in a given cluster is dependent on `direction`.
    //   - In LTR, `num_components` is the difference between the next cluster and the current cluster.
    //   - In RTL, `num_components` is the difference between the last cluster and the current cluster.
    // This is because we must compare the current cluster to its next larger ID (in other words, the next
    // logical index, which is visually downstream in LTR and visually upstream in RTL).
    //
    // For example, consider the LTR text for "afi" where "fi" form a ligature.
    //   Initial cluster values: 0, 1, 2 (logical + visual order)
    //   `HarfRust` assignation: 0, 1, 1
    //   Cluster count:          2
    //   `num_components`:       (1 - 0 =) 1, (3 - 1 =) 2
    //
    // Now consider the RTL text for "حداً".
    //   Initial cluster values:  0, 1, 2, 3 (logical, or in-memory, order)
    //   Reversed cluster values: 3, 2, 1, 0 (visual order - the return order of `HarfRust` for RTL)
    //   `HarfRust` assignation:  3, 2, 0, 0
    //   Cluster count:           3
    //   `num_components`:        (4 - 3 =) 1, (3 - 2 =) 1, (2 - 0 =) 2
    let num_components =
        |next_cluster: u32, current_cluster: u32, last_cluster: u32| match direction {
            Direction::Ltr => next_cluster - current_cluster,
            Direction::Rtl => last_cluster - current_cluster,
        };
    let mut last_cluster_id: u32 = match direction {
        Direction::Ltr => 0,
        Direction::Rtl => char_infos.len() as u32,
    };

    for (glyph_info, glyph_pos) in glyph_infos.iter().zip(glyph_positions.iter()) {
        // Flush previous cluster if we've reached a new cluster
        if cluster_id != glyph_info.cluster {
            let num_components = num_components(glyph_info.cluster, cluster_id, last_cluster_id);
            cluster_advance /= num_components as f32;
            let is_newline = to_whitespace(cluster_start_char.1) == Whitespace::Newline;
            let cluster_type = if num_components > 1 {
                debug_assert!(!is_newline);
                ClusterType::LigatureStart
            } else if is_newline {
                ClusterType::Newline
            } else {
                ClusterType::Regular
            };

            let inline_glyph_id = if matches!(cluster_type, ClusterType::Regular) {
                pending_inline_glyph.take().map(|g| g.id)
            } else {
                // This isn't a regular cluster, so we don't inline the glyph and push
                // it to `glyphs`.
                if let Some(pending) = pending_inline_glyph.take() {
                    glyphs.push(pending);
                    total_glyphs += 1;
                }
                None
            };

            push_cluster(
                clusters,
                char_info,
                cluster_start_char,
                cluster_glyph_offset,
                cluster_advance,
                total_glyphs,
                cluster_type,
                inline_glyph_id,
            );
            cluster_glyph_offset = total_glyphs;

            if num_components > 1 {
                // Skip characters until we reach the current cluster
                for i in 1..num_components {
                    cluster_start_char = char_indices_iter.next().unwrap();
                    if to_whitespace(cluster_start_char.1) == Whitespace::Space {
                        break;
                    }
                    let char_info_ = match direction {
                        Direction::Ltr => char_info_at((cluster_id + i) as usize),
                        Direction::Rtl => char_info_at((cluster_id + num_components - i) as usize),
                    };
                    push_cluster(
                        clusters,
                        char_info_,
                        cluster_start_char,
                        cluster_glyph_offset,
                        cluster_advance,
                        total_glyphs,
                        ClusterType::LigatureComponent,
                        None,
                    );
                }
            }
            cluster_start_char = char_indices_iter.next().unwrap();

            cluster_advance = 0.0;
            last_cluster_id = cluster_id;
            cluster_id = glyph_info.cluster;
            char_info = char_info_at(cluster_id as usize);
            pending_inline_glyph = None;
        }

        let glyph = Glyph {
            id: glyph_info.glyph_id,
            x: (glyph_pos.x_offset as f32) * scale_factor,
            // Convert from font space (Y-up) to layout space (Y-down)
            y: -(glyph_pos.y_offset as f32) * scale_factor,
            advance: (glyph_pos.x_advance as f32) * scale_factor,
        };
        cluster_advance += glyph.advance;
        // Push any pending glyph. If it was a zero-offset, single glyph cluster, it would
        // have been pushed in the first `if` block.
        if let Some(pending) = pending_inline_glyph.take() {
            glyphs.push(pending);
            total_glyphs += 1;
        }
        if total_glyphs == cluster_glyph_offset && glyph.x == 0.0 && glyph.y == 0.0 {
            // Defer this potential zero-offset, single glyph cluster
            pending_inline_glyph = Some(glyph);
        } else {
            glyphs.push(glyph);
            total_glyphs += 1;
        }
    }

    // Push the last cluster
    {
        // See comment above `num_components` for why we use `char_infos.len()` for LTR and 0 for RTL.
        let next_cluster_id = match direction {
            Direction::Ltr => char_infos.len() as u32,
            Direction::Rtl => 0,
        };
        let num_components = num_components(next_cluster_id, cluster_id, last_cluster_id);
        if num_components > 1 {
            // This is a ligature - create ligature start + ligature components

            if let Some(pending) = pending_inline_glyph.take() {
                glyphs.push(pending);
                total_glyphs += 1;
            }
            let ligature_advance = cluster_advance / num_components as f32;
            push_cluster(
                clusters,
                char_info,
                cluster_start_char,
                cluster_glyph_offset,
                ligature_advance,
                total_glyphs,
                ClusterType::LigatureStart,
                None,
            );

            cluster_glyph_offset = total_glyphs;

            // Create ligature component clusters for the remaining characters
            for (i, char) in (1..).zip(char_indices_iter) {
                if to_whitespace(char.1) == Whitespace::Space {
                    break;
                }
                let component_char_info = match direction {
                    Direction::Ltr => char_info_at((cluster_id + i) as usize),
                    Direction::Rtl => char_info_at((cluster_id + num_components - i) as usize),
                };
                push_cluster(
                    clusters,
                    component_char_info,
                    char,
                    cluster_glyph_offset,
                    ligature_advance,
                    total_glyphs,
                    ClusterType::LigatureComponent,
                    None,
                );
            }
        } else {
            let is_newline = to_whitespace(cluster_start_char.1) == Whitespace::Newline;
            let cluster_type = if is_newline {
                ClusterType::Newline
            } else {
                ClusterType::Regular
            };
            let mut inline_glyph_id = None;
            match cluster_type {
                ClusterType::Regular => {
                    if total_glyphs == cluster_glyph_offset
                        && let Some(pending) = pending_inline_glyph.take()
                    {
                        inline_glyph_id = Some(pending.id);
                    }
                }
                _ => {
                    if let Some(pending) = pending_inline_glyph.take() {
                        glyphs.push(pending);
                        total_glyphs += 1;
                    }
                }
            }
            push_cluster(
                clusters,
                char_info,
                cluster_start_char,
                cluster_glyph_offset,
                cluster_advance,
                total_glyphs,
                cluster_type,
                inline_glyph_id,
            );
        }
    }
}

#[derive(PartialEq)]
enum Direction {
    Ltr,
    Rtl,
}

enum ClusterType {
    LigatureStart,
    LigatureComponent,
    Regular,
    Newline,
}

impl From<&ClusterType> for u16 {
    fn from(cluster_type: &ClusterType) -> Self {
        match cluster_type {
            ClusterType::LigatureStart => ClusterData::LIGATURE_START,
            ClusterType::LigatureComponent => ClusterData::LIGATURE_COMPONENT,
            ClusterType::Regular | ClusterType::Newline => 0, // No special flags
        }
    }
}

#[expect(clippy::missing_assert_message, reason = "Deferred")]
#[expect(clippy::cast_possible_truncation, reason = "Deferred")]
fn push_cluster(
    clusters: &mut Vec<ClusterData>,
    char_info: (CharInfo, u16),
    cluster_start_char: (usize, char),
    glyph_offset: u32,
    advance: f32,
    total_glyphs: u32,
    cluster_type: ClusterType,
    inline_glyph_id: Option<u32>,
) {
    let glyph_len = (total_glyphs - glyph_offset) as u8;

    let (final_glyph_len, final_glyph_offset, final_advance) = match cluster_type {
        ClusterType::LigatureComponent => {
            // Ligature components have no glyphs, only advance.
            debug_assert_eq!(glyph_len, 0);
            (0_u8, 0_u32, advance)
        }
        ClusterType::Newline => {
            // Newline clusters are stripped of their glyph contribution.
            debug_assert_eq!(glyph_len, 1);
            (0_u8, 0_u32, 0.0)
        }
        _ if inline_glyph_id.is_some() => {
            // Inline glyphs are stored inline within `ClusterData`
            debug_assert_eq!(glyph_len, 0);
            (0xFF_u8, inline_glyph_id.unwrap(), advance)
        }
        ClusterType::Regular | ClusterType::LigatureStart => {
            // Regular and ligature start clusters maintain their glyphs and advance.
            debug_assert_ne!(glyph_len, 0);
            (glyph_len, glyph_offset, advance)
        }
    };

    clusters.push(ClusterData {
        info: ClusterInfo::new(char_info.0.boundary, cluster_start_char.1),
        flags: (&cluster_type).into(),
        style_index: char_info.1,
        glyph_len: final_glyph_len,
        text_len: cluster_start_char.1.len_utf8() as u8,
        glyph_offset: final_glyph_offset,
        text_offset: cluster_start_char.0 as u16,
        advance: final_advance,
    });
}

#[cfg(test)]
mod tests {
    use alloc::{sync::Arc, vec};

    use fontique::Synthesis;
    use linebender_resource_handle::{Blob, FontData};

    use crate::{Analysis, AnalysisOptions, Analyzer, FontInstance, ShapeOptions, Shaper};

    use super::ShapedText;

    const ROBOTO: &[u8] =
        include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");

    fn shape(text: &str) -> ShapedText {
        let mut analysis = Analysis::new();
        Analyzer::new().analyze(
            text,
            &AnalysisOptions {
                word_break: &[],
                line_break_override: None,
            },
            &mut analysis,
        );
        let font = FontInstance {
            font: FontData::new(Blob::new(Arc::new(ROBOTO)), 0),
            synthesis: Synthesis::default(),
        };
        let char_style_indices = vec![0; text.chars().count()];
        let mut shaper = Shaper::default();
        let mut shaped = ShapedText::new();
        for item in analysis.itemize(text, |_| false) {
            shaper.shape_item(
                text,
                &analysis,
                &item,
                &ShapeOptions {
                    font_size: 32.0,
                    language: None,
                    features: &[],
                    variations: &[],
                    char_style_indices: &char_style_indices,
                },
                |_| Some(font.clone()),
                &mut shaped,
            );
        }
        shaped
    }

    #[test]
    fn single_cluster_run_advance_matches_its_cluster() {
        let shaped = shape("A");
        let run = &shaped.runs()[0];
        let cluster = &shaped.clusters()[run.clusters_range.clone()][0];

        assert!(cluster.advance > 0.0);
        assert_eq!(run.advance, cluster.advance);
    }
}
