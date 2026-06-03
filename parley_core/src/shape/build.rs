// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Turning a [`harfrust::GlyphBuffer`] into the [`ShapedText`](crate::ShapedText) representation.

use alloc::vec::Vec;

use harfrust::{GlyphBuffer, GlyphInfo};

use crate::analysis::CharInfo;
use crate::common::Whitespace;
use crate::shaped_text::{ClusterData, Glyph, INLINE_GLYPH};
use crate::util::nearly_zero;

/// Builds the clusters and glyphs for one shaped font run into `out_clusters` and `out_glyphs`,
/// returning the font run's total advance.
///
/// `char_offsets` are the font run's characters and indices in logical order; `char_infos` is
/// the parallel per-character analysis slice — see [`crate::Analysis::char_infos`].
pub(super) fn build_clusters(
    glyph_buffer: &GlyphBuffer,
    is_rtl: bool,
    is_vertical: bool,
    scale: f32,
    char_offsets: &[(usize, char)],
    char_infos: &[CharInfo],
    out_clusters: &mut Vec<ClusterData>,
    out_glyphs: &mut Vec<Glyph>,
) -> f32 {
    // `HarfRust` returns glyphs in visual order, so we need to process them as such while
    // maintaining logical ordering of clusters.

    let glyph_infos = glyph_buffer.glyph_infos();
    if glyph_infos.is_empty() {
        return 0.0;
    }
    let glyph_positions = glyph_buffer.glyph_positions();
    let cluster_range_start = out_clusters.len();
    let run_advance = if !is_rtl {
        process_clusters(
            Direction::Ltr,
            is_vertical,
            out_clusters,
            out_glyphs,
            scale,
            glyph_infos,
            glyph_positions,
            char_infos,
            char_offsets.iter().copied(),
        )
    } else {
        let advance = process_clusters(
            Direction::Rtl,
            is_vertical,
            out_clusters,
            out_glyphs,
            scale,
            glyph_infos,
            glyph_positions,
            char_infos,
            char_offsets.iter().rev().copied(),
        );
        // Reverse clusters into logical order for RTL
        let clusters_len = out_clusters.len();
        out_clusters[cluster_range_start..clusters_len].reverse();
        advance
    };
    run_advance
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
/// * `is_vertical` - Whether the run is laid out along the vertical axis (Y-down main axis).
/// * `scale_factor` - Scaling factor used to convert font units to the target size.
/// * `glyph_infos` - `HarfRust` glyph information in visual order.
/// * `glyph_positions` - `HarfRust` glyph positioning data in visual order.
/// * `char_infos` - Character information from text analysis, indexed by cluster ID.
/// * `char_indices_iter` - Iterator over (`byte_offset`, `char`) pairs from the source text.
///   Should be in logical order (forward for LTR, reverse for RTL).
fn process_clusters<I: Iterator<Item = (usize, char)>>(
    direction: Direction,
    is_vertical: bool,
    clusters: &mut Vec<ClusterData>,
    glyphs: &mut Vec<Glyph>,
    scale_factor: f32,
    glyph_infos: &[GlyphInfo],
    glyph_positions: &[harfrust::GlyphPosition],
    char_infos: &[CharInfo],
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
    let mut cluster_flags: u16 = 0;
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
            let is_newline = to_whitespace(cluster_start_char.1) == Whitespace::Newline;
            // Newline clusters contribute no advance to the run.
            if !is_newline {
                run_advance += cluster_advance;
            }
            let num_components = num_components(glyph_info.cluster, cluster_id, last_cluster_id);
            cluster_advance /= num_components as f32;
            let cluster_type = if num_components > 1 {
                debug_assert!(!is_newline, "a multi-character cluster cannot be a newline");
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
                cluster_flags,
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
                    // The ligature's single glyph spans this position; a break here would have
                    // to reshape both sides to decompose the glyph. Mark the component unsafe so
                    // boundary scans sever the ligature exactly as they sever a cursive join.
                    push_cluster(
                        clusters,
                        char_info_,
                        cluster_start_char,
                        cluster_glyph_offset,
                        cluster_advance,
                        total_glyphs,
                        ClusterType::LigatureComponent,
                        None,
                        ClusterData::UNSAFE_TO_BREAK | ClusterData::UNSAFE_TO_CONCAT,
                    );
                }
            }
            cluster_start_char = char_indices_iter.next().unwrap();

            cluster_advance = 0.0;
            cluster_flags = 0;
            last_cluster_id = cluster_id;
            cluster_id = glyph_info.cluster;
            char_info = char_infos[cluster_id as usize];
            pending_inline_glyph = None;
        }

        let main_advance = if is_vertical {
            // `harfrust` is Y-up, but we're Y-down.
            -(glyph_pos.y_advance as f32)
        } else {
            glyph_pos.x_advance as f32
        };
        let glyph = Glyph {
            id: glyph_info.glyph_id,
            x: (glyph_pos.x_offset as f32) * scale_factor,
            // Convert from font space (Y-up) to layout space (Y-down)
            y: -(glyph_pos.y_offset as f32) * scale_factor,
            advance: main_advance * scale_factor,
        };
        cluster_advance += glyph.advance;
        cluster_flags |= harf_flags(glyph_info);
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
            run_advance += cluster_advance;
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
                cluster_flags,
            );

            cluster_glyph_offset = total_glyphs;

            // Create ligature component clusters for the remaining characters
            for (i, char) in (1..).zip(char_indices_iter) {
                if to_whitespace(char.1) == Whitespace::Space {
                    break;
                }
                let component_char_info = match direction {
                    Direction::Ltr => char_infos[(cluster_id + i) as usize],
                    Direction::Rtl => char_infos[(cluster_id + num_components - i) as usize],
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
                    ClusterData::UNSAFE_TO_BREAK | ClusterData::UNSAFE_TO_CONCAT,
                );
            }
        } else {
            let is_newline = to_whitespace(cluster_start_char.1) == Whitespace::Newline;
            let cluster_type = if is_newline {
                ClusterType::Newline
            } else {
                ClusterType::Regular
            };
            if !is_newline {
                run_advance += cluster_advance;
            }
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
                cluster_flags,
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
    clusters: &mut Vec<ClusterData>,
    char_info: CharInfo,
    cluster_start_char: (usize, char),
    glyph_offset: u32,
    advance: f32,
    total_glyphs: u32,
    cluster_type: ClusterType,
    inline_glyph_id: Option<u32>,
    extra_flags: u16,
) {
    let glyph_len = (total_glyphs - glyph_offset) as u8;

    let (final_glyph_len, final_glyph_offset, final_advance) = match cluster_type {
        ClusterType::LigatureComponent => {
            // Ligature components have no glyphs, only advance.
            debug_assert_eq!(glyph_len, 0, "ligature components carry no glyphs");
            (0_u8, 0_u32, advance)
        }
        ClusterType::Newline => {
            // Newline clusters are stripped of their glyph contribution.
            debug_assert_eq!(glyph_len, 1, "a newline cluster has exactly one glyph");
            (0_u8, 0_u32, 0.0)
        }
        _ if inline_glyph_id.is_some() => {
            // Inline glyphs are stored inline within `ClusterData`
            debug_assert_eq!(glyph_len, 0, "the inlined glyph is not in the glyph array");
            (INLINE_GLYPH, inline_glyph_id.unwrap(), advance)
        }
        ClusterType::Regular | ClusterType::LigatureStart => {
            // Regular and ligature start clusters maintain their glyphs and advance.
            debug_assert_ne!(
                glyph_len, 0,
                "a non-inlined cluster must own at least one glyph"
            );
            (glyph_len, glyph_offset, advance)
        }
    };

    clusters.push(ClusterData {
        advance: final_advance,
        glyph_offset: final_glyph_offset,
        text_offset: cluster_start_char.0 as u16,
        text_len: cluster_start_char.1.len_utf8() as u8,
        glyph_len: final_glyph_len,
        flags: u16::from(&cluster_type) | extra_flags,
        boundary: char_info.boundary(),
        whitespace: to_whitespace(cluster_start_char.1),
    });
}

/// Applies letter spacing/word spacing to clusters/glyphs, returning the advance added to the run.
pub(super) fn apply_spacing(
    clusters: &mut [ClusterData],
    glyphs: &mut [Glyph],
    letter_spacing: f32,
    word_spacing: f32,
) -> f32 {
    if nearly_zero(word_spacing) && nearly_zero(letter_spacing) {
        return 0.0;
    }
    let mut extra = 0.0;
    for cluster in clusters {
        let mut spacing = letter_spacing;
        if !nearly_zero(word_spacing) && cluster.whitespace.is_space_or_nbsp() {
            spacing += word_spacing;
        }
        if !nearly_zero(spacing) {
            cluster.advance += spacing;
            extra += spacing;
            if cluster.glyph_len != INLINE_GLYPH {
                let start = cluster.glyph_offset as usize;
                let end = start + cluster.glyph_len as usize;
                let glyphs = &mut glyphs[start..end];
                if let Some(last) = glyphs.last_mut() {
                    last.advance += spacing;
                }
            }
        }
    }
    extra
}

/// Maps a `harfrust` glyph's break flags to [`ClusterData`] flag bits.
///
/// Requires the buffer to have requested the unsafe-to-concat and tatweel flags; unsafe-to-break
/// is always produced.
fn harf_flags(info: &GlyphInfo) -> u16 {
    let mut flags = 0;
    if info.unsafe_to_break() {
        flags |= ClusterData::UNSAFE_TO_BREAK;
    }
    if info.unsafe_to_concat() {
        flags |= ClusterData::UNSAFE_TO_CONCAT;
    }
    if info.safe_to_insert_tatweel() {
        flags |= ClusterData::SAFE_TO_INSERT_TATWEEL;
    }
    flags
}

const fn to_whitespace(c: char) -> Whitespace {
    const LINE_SEPARATOR: char = '\u{2028}';
    const PARAGRAPH_SEPARATOR: char = '\u{2029}';

    match c {
        ' ' => Whitespace::Space,
        '\t' => Whitespace::Tab,
        '\n' | '\r' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR => Whitespace::Newline,
        '\u{00A0}' => Whitespace::NoBreakSpace,
        _ => Whitespace::None,
    }
}
