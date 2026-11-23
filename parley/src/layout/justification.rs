// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Justification opportunities, and their distribution over a line's atoms.
//!
//! Following the [minimum requirements for `text-justify: auto`][css] in CSS Text Level 3, a
//! line's justification opportunities are:
//!
//! - after each [word separator](is_word_separator), and
//! - at each visual boundary between two atoms where at least one of them is "justify expandable"
//!   (a character of a block script, i.e., mostly CJK ideographs, kana and fullwidth symbols, or
//!   an emoji, see [`ShapedCluster::is_justify_expandable`]) and neither is a word separator.
//!
//! The clustered scripts (e.g., Thai) don't provide justification opportunities yet.
//!
//! A word separator owns the whole opportunity after itself. An opportunity between two expandable
//! atoms is shared equally between the two, and one between an expandable and some other atom is
//! owned in whole by the expandable atom. Opportunities at the line's edges (including next to
//! the atoms excluded from justification, like hanging whitespace) are dropped. Zero-width
//! default ignorable atoms are transparent, i.e., the boundary goes through them.
//!
//! [css]: https://www.w3.org/TR/css-text-3/#justify-algos
//! [`ShapedCluster::is_justify_expandable`]: parley_engine::shape::ShapedCluster::is_justify_expandable

use alloc::vec::Vec;

use parley_engine::{Atom, ShapedText};

use super::data::{LineData, LineItemData};
use super::spacing::{Gaps, is_word_separator, visual_edges};

/// The justification class of an atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// An edge of the line, or an atom excluded from justification (like hanging whitespace).
    Edge,
    /// A [word separator](is_word_separator).
    Space,
    /// A justify expandable atom (e.g., a CJK ideograph or an emoji).
    Expandable,
    /// Any other atom.
    Opaque,
}

impl Class {
    /// The class of `atom`, or `None` if the atom is transparent.
    ///
    /// Atoms starting at or beyond shaped cluster `end_cluster` are excluded from justification.
    fn of(atom: &Atom<'_>, end_cluster: u32) -> Option<Self> {
        // The first shaped cluster caches the relevant properties of the atom's first character.
        let first = atom.shaped_clusters()[0];
        Some(if atom.shaped_clusters_range().start >= end_cluster {
            Self::Edge
        } else if first.is_inline_object() {
            Self::Opaque
        } else if is_word_separator(first.whitespace()) {
            Self::Space
        } else if first.is_justify_expandable() {
            Self::Expandable
        } else if first.is_default_ignorable() && atom.advance() == 0. {
            // A zero-width ignorable does not interrupt visual adjacency.
            return None;
        } else {
            Self::Opaque
        })
    }
}

/// The shares of the justification opportunities owned by a shaped cluster, visually before and
/// after it, in units of half an opportunity.
///
/// An atom's share before itself is owned by its visually first shaped cluster, and its share after
/// itself by its visually last shaped cluster (see [`visual_edges`]), so that's where the spacing is
/// applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ClusterShares(u8);

impl ClusterShares {
    const AFTER_SHIFT: u8 = 2;

    /// The half opportunities visually before the cluster.
    #[inline(always)]
    fn before(self) -> u8 {
        self.0 & ((1 << Self::AFTER_SHIFT) - 1)
    }

    /// The half opportunities visually after the cluster.
    #[inline(always)]
    fn after(self) -> u8 {
        self.0 >> Self::AFTER_SHIFT
    }
}

/// The justification opportunities of a line.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opportunities {
    /// The shares of the line's opportunities owned by its shaped clusters.
    ///
    /// These are the shaped clusters of the line's text items, one item after the other, in the
    /// order of the line's items (i.e., visual order).
    shares: Vec<ClusterShares>,
    /// The line's total number of half opportunities.
    halves: u32,
}

impl Opportunities {
    /// Find the justification opportunities of `line`, whose items are `items`.
    pub(crate) fn find(
        &mut self,
        shaped_text: &ShapedText,
        line: &LineData,
        items: &[LineItemData],
    ) {
        self.shares.clear();
        self.halves = 0;

        // The class of the visually previous (non-transparent) atom, and the index of its
        // visually last shaped cluster.
        let mut previous = (Class::Edge, 0);
        // The index in `Self::shares` of the item's first shaped cluster.
        let mut item_start = 0;
        // The line's items are in visual order.
        for item in items {
            let clusters = item.shaped_cluster_range;
            let cluster_len = (clusters.end - clusters.start) as usize;
            let is_rtl = item.is_rtl();
            self.shares
                .resize(item_start + cluster_len, ClusterShares::default());
            let slice = shaped_text
                .run_slice(item.index as u32)
                .narrow(clusters.into());
            // Visit the item's atoms in visual order.
            let mut atoms = if is_rtl {
                slice.atoms_end()
            } else {
                slice.atoms_start()
            };
            while let Some(atom) = if is_rtl { atoms.prev() } else { atoms.next() } {
                let Some(class) = Class::of(&atom, line.justification_end_cluster) else {
                    continue;
                };
                let atom_start =
                    item_start + (atom.shaped_clusters_range().start - clusters.start) as usize;
                let (first, last) = visual_edges(atom.shaped_clusters(), is_rtl);
                // The opportunity at the boundary with the previous atom.
                let (after_previous, before) = match (previous.0, class) {
                    (Class::Expandable, Class::Expandable) => (1, 1),
                    (Class::Expandable, Class::Opaque) => (2, 0),
                    (Class::Opaque, Class::Expandable) => (0, 2),
                    _ => (0, 0),
                };
                // A word separator owns the opportunity after itself, regardless of what follows.
                let after = if class == Class::Space { 2 } else { 0 };
                if after_previous != 0 {
                    self.shares[previous.1].0 |= after_previous << ClusterShares::AFTER_SHIFT;
                }
                if before != 0 {
                    self.shares[atom_start + first].0 |= before;
                }
                if after != 0 {
                    self.shares[atom_start + last].0 |= after << ClusterShares::AFTER_SHIFT;
                }
                self.halves += u32::from(after_previous + before + after);
                previous = (class, atom_start + last);
            }
            item_start += cluster_len;
        }
    }

    /// Distribute `free_space` over the opportunities, adding it to `gaps`, the gaps of the shaped
    /// clusters of the line's text items (in the order of the line's items).
    ///
    /// Returns `false` if there are no opportunities, in which case nothing is changed.
    pub(crate) fn justify(&self, gaps: &mut [Gaps], free_space: f32) -> bool {
        if self.halves == 0 {
            return false;
        }
        let amount_per_half = free_space / self.halves as f32;
        for (gaps, shares) in gaps.iter_mut().zip(&self.shares) {
            gaps.before += f32::from(shares.before()) * amount_per_half;
            gaps.after += f32::from(shares.after()) * amount_per_half;
        }
        true
    }
}

#[cfg(test)]
mod tests;
