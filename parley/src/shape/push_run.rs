// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    Brush, Glyph, LineHeight, RunMetrics,
    analysis::{CharInfo, cluster::Whitespace},
    resolve::ResolvedStyle,
};
use core::ops::Range;
use linebender_resource_handle::FontData;

use super::data::{ClusterData, ClusterInfo, LayoutItem, LayoutItemKind, RunData, to_whitespace};

pub(crate) trait ShapeSink {
    fn push_coords(&mut self, coords: &[harfrust::NormalizedCoord]) -> (usize, usize);
    fn push_font(&mut self, font: &FontData) -> usize;
    fn push_cluster(&mut self, cluster: ClusterData);
    fn push_glyph(&mut self, glyph: Glyph);
    fn push_run(&mut self, run: RunData);
    fn push_item(&mut self, item: LayoutItem);

    fn cluster_count(&self) -> usize;
    fn glyph_count(&self) -> usize;
    fn run_count(&self) -> usize;

    fn reverse_cluster_range(&mut self, range: Range<usize>);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_run<B: Brush>(
    sink: &mut impl ShapeSink,
    font: FontData,
    font_size: f32,
    font_attrs: fontique::Attributes,
    synthesis: fontique::Synthesis,
    glyph_buffer: &harfrust::GlyphBuffer,
    bidi_level: u8,
    style: &ResolvedStyle<B>,
    word_spacing: f32,
    letter_spacing: f32,
    source_text: &str,
    char_infos: &[(CharInfo, u16)], // From text analysis
    text_range: Range<usize>,       // The text range this run covers
    coords: &[harfrust::NormalizedCoord],
) {
    let (coords_start, coords_end) = sink.push_coords(coords);
    let font_index = sink.push_font(&font);

    let metrics = {
        let font_ref = skrifa::FontRef::from_index(font.data.as_ref(), font.index).unwrap();
        skrifa::metrics::Metrics::new(&font_ref, skrifa::prelude::Size::new(font_size), coords)
    };
    let units_per_em = metrics.units_per_em as f32;

    let metrics = {
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

        // Compute line height
        let line_height = match style.line_height {
            LineHeight::Absolute(value) => value,
            LineHeight::FontSizeRelative(value) => value * font_size,
            LineHeight::MetricsRelative(value) => {
                (metrics.ascent - metrics.descent + metrics.leading) * value
            }
        };

        RunMetrics {
            ascent: metrics.ascent,
            descent: -metrics.descent,
            leading: metrics.leading,
            underline_offset,
            underline_size,
            strikethrough_offset,
            strikethrough_size,
            line_height,
            x_height: metrics.x_height,
            cap_height: metrics.cap_height,
        }
    };

    let cluster_range = sink.cluster_count()..sink.cluster_count();

    let mut run = RunData {
        font_index,
        font_size,
        font_attrs,
        synthesis,
        coords_range: coords_start..coords_end,
        text_range,
        bidi_level,
        cluster_range,
        glyph_start: sink.glyph_count(),
        metrics,
        word_spacing,
        letter_spacing,
        advance: 0.,
    };

    // `HarfRust` returns glyphs in visual order, so we need to process them as such while
    // maintaining logical ordering of clusters.

    let glyph_infos = glyph_buffer.glyph_infos();
    if glyph_infos.is_empty() {
        return;
    }
    let glyph_positions = glyph_buffer.glyph_positions();
    let scale_factor = font_size / units_per_em;
    let cluster_range_start = sink.cluster_count();
    let is_rtl = bidi_level & 1 == 1;
    if !is_rtl {
        run.advance = process_clusters(
            sink,
            Direction::Ltr,
            scale_factor,
            glyph_infos,
            glyph_positions,
            char_infos,
            source_text.char_indices(),
        );
    } else {
        run.advance = process_clusters(
            sink,
            Direction::Rtl,
            scale_factor,
            glyph_infos,
            glyph_positions,
            char_infos,
            source_text.char_indices().rev(),
        );
        // Reverse clusters into logical order for RTL
        let clusters_len = sink.cluster_count();
        sink.reverse_cluster_range(cluster_range_start..clusters_len);
    }

    run.cluster_range = cluster_range_start..sink.cluster_count();
    if !run.cluster_range.is_empty() {
        sink.push_run(run);
        sink.push_item(LayoutItem {
            kind: LayoutItemKind::TextRun,
            index: sink.run_count() - 1,
            bidi_level,
        });
    }
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
fn process_clusters<I: Iterator<Item = (usize, char)>>(
    sink: &mut impl ShapeSink,
    direction: Direction,
    scale_factor: f32,
    glyph_infos: &[harfrust::GlyphInfo],
    glyph_positions: &[harfrust::GlyphPosition],
    char_infos: &[(CharInfo, u16)],
    char_indices_iter: I,
) -> f32 {
    let mut char_indices_iter = char_indices_iter.peekable();
    let mut cluster_start_char = char_indices_iter.next().unwrap();
    let mut total_glyphs: u32 = 0;
    let mut cluster_glyph_offset: u32 = 0;
    let start_cluster_id = glyph_infos.first().unwrap().cluster;
    let mut cluster_id = start_cluster_id;
    let mut char_info = char_infos[cluster_id as usize];
    let mut run_advance = 0.0;
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
            run_advance += cluster_advance;
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
                    sink.push_glyph(pending);
                    total_glyphs += 1;
                }
                None
            };

            push_cluster(
                sink,
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
                        Direction::Ltr => char_infos[(cluster_id + i) as usize],
                        Direction::Rtl => char_infos[(cluster_id + num_components - i) as usize],
                    };
                    push_cluster(
                        sink,
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
            char_info = char_infos[cluster_id as usize];
            pending_inline_glyph = None;
        }

        let glyph = Glyph {
            id: glyph_info.glyph_id,
            style_index: char_info.1,
            x: (glyph_pos.x_offset as f32) * scale_factor,
            // Convert from font space (Y-up) to layout space (Y-down)
            y: -(glyph_pos.y_offset as f32) * scale_factor,
            advance: (glyph_pos.x_advance as f32) * scale_factor,
        };
        cluster_advance += glyph.advance;
        // Push any pending glyph. If it was a zero-offset, single glyph cluster, it would
        // have been pushed in the first `if` block.
        if let Some(pending) = pending_inline_glyph.take() {
            sink.push_glyph(pending);
            total_glyphs += 1;
        }
        if total_glyphs == cluster_glyph_offset && glyph.x == 0.0 && glyph.y == 0.0 {
            // Defer this potential zero-offset, single glyph cluster
            pending_inline_glyph = Some(glyph);
        } else {
            sink.push_glyph(glyph);
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
                sink.push_glyph(pending);
                total_glyphs += 1;
            }
            let ligature_advance = cluster_advance / num_components as f32;
            push_cluster(
                sink,
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
            let mut i = 1;
            for char in char_indices_iter {
                if to_whitespace(char.1) == Whitespace::Space {
                    break;
                }
                let component_char_info = match direction {
                    Direction::Ltr => char_infos[(cluster_id + i) as usize],
                    Direction::Rtl => char_infos[(cluster_id + num_components - i) as usize],
                };
                push_cluster(
                    sink,
                    component_char_info,
                    char,
                    cluster_glyph_offset,
                    ligature_advance,
                    total_glyphs,
                    ClusterType::LigatureComponent,
                    None,
                );
                i += 1;
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
                    if total_glyphs == cluster_glyph_offset {
                        if let Some(pending) = pending_inline_glyph.take() {
                            inline_glyph_id = Some(pending.id);
                        }
                    }
                }
                _ => {
                    if let Some(pending) = pending_inline_glyph.take() {
                        sink.push_glyph(pending);
                        total_glyphs += 1;
                    }
                }
            }
            push_cluster(
                sink,
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

    run_advance
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

fn push_cluster(
    sink: &mut impl ShapeSink,
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

    sink.push_cluster(ClusterData {
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
