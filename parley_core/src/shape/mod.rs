// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping utilities.

mod cache;
mod cluster;
mod data;
pub(crate) mod shaped_text;
pub(crate) mod shaper;

use crate::CharInfo;

pub use cluster::{Char, CharCluster, SourceRange, Status, Whitespace};
pub use data::{ClusterData, ClusterInfo, to_whitespace};

/// Rebuilds the provided `char_cluster` in-place using the existing allocation
/// for the given grapheme `segment_text`, consuming items from `item_infos_iter`.
fn fill_cluster_in_place(
    segment_text: &str,
    item_infos_iter: &mut impl Iterator<Item = (CharInfo, u16)>,
    code_unit_offset_in_string: &mut usize,
    char_cluster: &mut CharCluster,
) {
    // Reset cluster but keep allocation
    char_cluster.clear();

    let mut force_normalize = false;
    let mut is_emoji_or_pictograph = false;
    let mut map_len: u8 = 0;
    let start = *code_unit_offset_in_string as u32;

    for ((_, ch), (info, style_index)) in segment_text.char_indices().zip(item_infos_iter.by_ref())
    {
        force_normalize |= info.force_normalize();
        // TODO - make emoji detection more complete, as per (except using composite Trie tables as
        //  much as possible:
        //  https://github.com/conor-93/parley/blob/4637d826732a1a82bbb3c904c7f47a16a21cceec/parley/src/shape/mod.rs#L221-L269
        is_emoji_or_pictograph |= info.is_emoji_or_pictograph();
        *code_unit_offset_in_string += ch.len_utf8();

        // TODO: Explore ignoring other modifiers in determining `contributes_to_shaping`:
        //  regional indicators, subdivision flag tag sequences, skin tone modifiers
        //  See also: https://github.com/google/emoji-segmenter

        // If the color emoji has a non-printing variation selector, ignore the variation selector.
        // Its presentation depends on the platform and font.
        //
        // e.g.
        //  - `U+270C + U+FE0F`: `✌`, force basic presentation
        //  - `U+270C + U+FE0F`: `✌️`, force emoji presentation
        //
        // <https://www.unicode.org/reports/tr37/>
        let is_emoji_with_non_printing_variation_selector =
            is_emoji_or_pictograph && info.is_variation_selector();

        let contributes_to_shaping =
            info.contributes_to_shaping() && !is_emoji_with_non_printing_variation_selector;
        if contributes_to_shaping {
            map_len += 1;
        }

        char_cluster.chars.push(Char {
            ch,
            contributes_to_shaping,
            glyph_id: 0,
            style_index,
            is_control_character: info.is_control(),
        });
    }

    // Finalize cluster metadata
    let end = *code_unit_offset_in_string as u32;
    char_cluster.is_emoji = is_emoji_or_pictograph;
    char_cluster.map_len = map_len;
    char_cluster.start = start;
    char_cluster.end = end;
    char_cluster.force_normalize = force_normalize;
}
