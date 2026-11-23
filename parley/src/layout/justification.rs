// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Justification opportunities and their distribution across a line's atoms.
//!
//! Following the [minimum requirements for `text-justify: auto`][css] in CSS Text Level 3,
//! justification opportunities occur:
//!
//! - after each [word separator](is_word_separator), and
//! - at each visual boundary between two atoms where at least one is justify-expandable (a block
//!   script character or emoji; see [`ShapedCluster::is_justify_expandable`]) and neither is a
//!   word separator.
//!
//! Clustered scripts (e.g., Thai) do not yet provide justification opportunities.
//!
//! A word separator owns the full opportunity after itself. Between two expandable atoms, the
//! opportunity is split equally; between an expandable and a non-expandable atom, the expandable
//! atom owns the full opportunity. Atoms excluded from justification (such as hanging whitespace)
//! act as line edges, and zero-width default-ignorable atoms are transparent to adjacency.
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
            return None;
        } else {
            Self::Opaque
        })
    }
}

/// Justification opportunity shares owned visually before and after a shaped cluster, packed as
/// two 2-bit counts of half-opportunities (each 0, 1, or 2).
///
/// An atom assigns its before-share to its visually first shaped cluster and its after-share to
/// its visually last (see [`visual_edges`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ClusterShares(u8);

impl ClusterShares {
    const AFTER_SHIFT: u8 = 2;

    #[inline(always)]
    fn before(self) -> u8 {
        self.0 & ((1 << Self::AFTER_SHIFT) - 1)
    }

    #[inline(always)]
    fn after(self) -> u8 {
        self.0 >> Self::AFTER_SHIFT
    }
}

/// The justification opportunities of a line.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opportunities {
    /// Per-cluster shares for the line's text items, concatenated in visual (item) order.
    shares: Vec<ClusterShares>,
    /// Total half-opportunities on the line.
    halves: u32,
}

impl Opportunities {
    /// Finds the justification opportunities on `line` (`items` must be in visual order).
    pub(crate) fn find(
        &mut self,
        shaped_text: &ShapedText,
        line: &LineData,
        items: &[LineItemData],
    ) {
        self.shares.clear();
        self.halves = 0;

        // Visually previous non-transparent atom: its class and the `shares` index of its visually
        // last cluster.
        let mut previous = (Class::Edge, 0);
        let mut item_start = 0;
        for item in items {
            let clusters = item.shaped_cluster_range;
            let cluster_len = (clusters.end - clusters.start) as usize;
            let is_rtl = item.is_rtl();
            self.shares
                .resize(item_start + cluster_len, ClusterShares::default());
            let slice = shaped_text
                .run_slice(item.index as u32)
                .narrow(clusters.into());
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
                let (after_previous, before) = match (previous.0, class) {
                    (Class::Expandable, Class::Expandable) => (1, 1),
                    (Class::Expandable, Class::Opaque) => (2, 0),
                    (Class::Opaque, Class::Expandable) => (0, 2),
                    _ => (0, 0),
                };
                // Word separators always own the opportunity after themselves (trailing ones are
                // already `Edge`).
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

    /// Distributes `free_space` across the found opportunities, adding it to `gaps`.
    ///
    /// `gaps` must align with the shaped clusters passed to [`Self::find`]. Returns `false`
    /// without modifying `gaps` if there are no opportunities.
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
