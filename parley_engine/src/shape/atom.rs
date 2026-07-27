// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ops::Range;

use crate::Glyph;

use super::data::{Character, ShapedCluster};

/// A slice of shaped text.
///
// NOTE: The motivation for this type, is that once `parley_engine` supports reshaping, this can
// hold reshaped slices that don't depend on `ShapedText`.
#[derive(Copy, Clone, Debug)]
pub struct ShapedSlice<'a> {
    /// Character indices of shaped text.
    ///
    /// Most character indices (like [`ShapedCluster::char_start`]) index into this array; note this
    /// is not necessarily parallel to characters of the source text. Only shaped text gets these
    /// characters.
    pub(crate) characters: &'a [Character],

    pub(crate) shaped_clusters: &'a [ShapedCluster],
    pub(crate) glyphs: &'a [Glyph],

    /// The range of [`Self::shaped_clusters`] this slice covers.
    pub(crate) clusters: (u32, u32),
}

impl<'a> ShapedSlice<'a> {
    /// The range of [`Self::characters`] of this slice.
    #[inline(always)]
    fn char_range(&self) -> (u32, u32) {
        if self.clusters.0 == self.clusters.1 {
            (0, 0)
        } else {
            (
                self.shaped_clusters[self.clusters.0 as usize].char_start,
                self.shaped_clusters[self.clusters.1 as usize - 1].char_end,
            )
        }
    }

    /// Get a cursor to walk atoms from the logical start of this slice.
    #[inline(always)]
    pub fn atoms_start(&self) -> Atoms<'a> {
        self.atoms_from_cluster(self.clusters.0)
    }

    /// Get a cursor to walk atoms from the logical end of this slice.
    #[inline(always)]
    pub fn atoms_end(&self) -> Atoms<'a> {
        self.atoms_from_cluster(self.clusters.1)
    }

    /// Get a cursor to walk atoms of this slice, starting before the logical start of the given
    /// cluster.
    #[inline(always)]
    fn atoms_from_cluster(&self, cluster: u32) -> Atoms<'a> {
        Atoms {
            slice: *self,
            cluster_idx: cluster,
        }
    }

    /// Get the atom containing the character at `char_index`.
    ///
    /// Note: this is the index into this [`ShapedSlice`]'s character slice, which is not
    /// necessarily the same as the underlying source text's characters.
    ///
    /// To start walking from the returned atom, call [`Atom::cursor_before`] or
    /// [`Atom::cursor_after`].
    #[inline]
    pub fn atom_at_char(&self, char_index: u32) -> Option<Atom<'a>> {
        let shaped_clusters =
            &self.shaped_clusters[self.clusters.0 as usize..self.clusters.1 as usize];

        let idx = shaped_clusters
            .partition_point(|cluster| cluster.char_start <= char_index)
            .checked_sub(1)?;
        if shaped_clusters[idx].char_end <= char_index {
            return None;
        }

        let mut idx = idx as u32;
        idx += self.clusters.0;
        while idx > self.clusters.0 && !self.shaped_clusters[idx as usize].is_grapheme_start() {
            idx -= 1;
        }

        Some(self.atoms_from_cluster(idx).next().unwrap())
    }

    /// Get the atom containing the character at `text_byte`.
    ///
    /// `text_byte` is a byte into the source text.
    ///
    /// To start walking from the returned atom, call [`Atom::cursor_before`] or
    /// [`Atom::cursor_after`].
    #[inline]
    pub fn atom_at_text_byte(&self, text_byte: u32) -> Option<Atom<'a>> {
        let shaped_clusters =
            &self.shaped_clusters[self.clusters.0 as usize..self.clusters.1 as usize];

        let idx = shaped_clusters
            .partition_point(|cluster| {
                self.characters[cluster.char_start as usize].text_byte_start <= text_byte
            })
            .checked_sub(1)?;
        let last_character = self.characters[shaped_clusters[idx].char_end as usize - 1];
        if last_character.text_byte_start + last_character.info.source_char().len_utf8() as u32
            <= text_byte
        {
            return None;
        }

        let mut idx = idx as u32;
        idx += self.clusters.0;
        while idx > self.clusters.0 && !self.shaped_clusters[idx as usize].is_grapheme_start() {
            idx -= 1;
        }

        Some(self.atoms_from_cluster(idx).next().unwrap())
    }

    /// Narrow the shaped slice to the given range of clusters.
    #[inline(always)]
    pub fn narrow(&self, clusters: Range<u32>) -> Self {
        debug_assert!(
            clusters.is_empty()
                || (self.clusters.0 <= clusters.start && clusters.end <= self.clusters.1),
            "narrowed cluster range out of this slice's range"
        );
        Self {
            clusters: (clusters.start, clusters.end),
            ..*self
        }
    }

    /// Get a cursor to walk graphemes from the logical start of this slice.
    #[inline]
    pub fn graphemes_start(&self) -> Graphemes<'a> {
        let char_range = self.char_range();
        let cluster_idx = self.clusters.0;
        Graphemes {
            slice: *self,
            // char_range,
            char_idx: char_range.0,
            cluster_idx,
            partial_advance: self.partial_advance_at(cluster_idx),
        }
    }

    /// Get a cursor to walk graphemes from the logical end of this slice.
    #[inline]
    pub fn graphemes_end(&self) -> Graphemes<'a> {
        let char_range = self.char_range();
        let cluster_idx = self.clusters.1.max(self.clusters.0 + 1) - 1;
        Graphemes {
            slice: *self,
            // char_range,
            char_idx: char_range.1,
            cluster_idx,
            partial_advance: self.partial_advance_at(cluster_idx),
        }
    }

    #[inline(always)]
    fn partial_advance_at(&self, cluster_idx: u32) -> f32 {
        if self.clusters.0 == self.clusters.1 {
            return 0.0;
        }
        let cluster = &self.shaped_clusters[cluster_idx as usize];
        cluster.advance / cluster.graphemes_overlapped(self.characters) as f32
    }
}

/// An [`Atom`] cursor.
///
/// This cursor can walk forwards and backwards over atoms.
#[derive(Copy, Clone, Debug)]
pub struct Atoms<'a> {
    slice: ShapedSlice<'a>,

    /// The gap we're pointing at.
    cluster_idx: u32,
}

impl<'a> Atoms<'a> {
    /// Get the atom logically preceding the cursor.
    ///
    /// See also [`Self::next`].
    #[inline]
    pub fn prev(&mut self) -> Option<Atom<'a>> {
        let end = self.cluster_idx;
        let bound = self.slice.clusters.0;
        if end == bound {
            return None;
        }

        let mut idx = end - 1;
        let mut advance = self.slice.shaped_clusters[idx as usize].advance;
        while idx > bound && !self.slice.shaped_clusters[idx as usize].is_grapheme_start() {
            idx -= 1;
            advance += self.slice.shaped_clusters[idx as usize].advance;
        }
        self.cluster_idx = idx;
        Some(Atom {
            slice: self.slice,
            clusters: (idx, end),
            chars: (
                self.slice.shaped_clusters[idx as usize].char_start,
                self.slice.shaped_clusters[end as usize - 1].char_end,
            ),
            advance,
        })
    }
}

impl<'a> Iterator for Atoms<'a> {
    type Item = Atom<'a>;

    /// Get the atom logically following the cursor.
    ///
    /// See also [`Self::prev`].
    #[inline]
    fn next(&mut self) -> Option<Atom<'a>> {
        let start = self.cluster_idx;
        let bound = self.slice.clusters.1;
        if start == bound {
            return None;
        }

        let mut advance = self.slice.shaped_clusters[start as usize].advance;
        let mut idx = start + 1;
        while idx < bound && !self.slice.shaped_clusters[idx as usize].is_grapheme_start() {
            advance += self.slice.shaped_clusters[idx as usize].advance;
            idx += 1;
        }
        self.cluster_idx = idx;
        Some(Atom {
            slice: self.slice,
            clusters: (start, idx),
            chars: (
                self.slice.shaped_clusters[start as usize].char_start,
                self.slice.shaped_clusters[idx as usize - 1].char_end,
            ),
            advance,
        })
    }
}

/// An atom of shaped text.
///
/// This is smallest span of text where both edges are a [`Grapheme`] boundary and [`ShapedCluster`]
/// boundary.
///
/// An atom cannot be broken without reshaping. Check [`Self::is_safe_to_break_before`] to see if
/// you can break the text before this atom without requiring reshaping.
#[derive(Copy, Clone, Debug)]
pub struct Atom<'a> {
    slice: ShapedSlice<'a>,

    /// The clusters this atom spans inside [`ShapedSlice::shaped_clusters`].
    clusters: (u32, u32),

    /// The characters this atom spans inside [`ShapedSlice::characters`].
    chars: (u32, u32),

    advance: f32,
}
impl<'a> Atom<'a> {
    /// The range of [`ShapedCluster`] into the underlying [`ShapedSlice`] this atom spans.
    #[inline(always)]
    pub fn clusters_range(&self) -> Range<u32> {
        self.clusters.0..self.clusters.1
    }

    /// The [`ShapedCluster`]s from the underlying [`ShapedSlice`] this atom spans.
    #[inline(always)]
    pub fn clusters(&self) -> &[ShapedCluster] {
        &self.slice.shaped_clusters[self.clusters.0 as usize..self.clusters.1 as usize]
    }

    /// Get a cursor to walk graphemes from the logical start of this atom.
    #[inline(always)]
    pub fn graphemes_start(&self) -> Graphemes<'a> {
        self.slice
            .narrow(self.clusters.0..self.clusters.1)
            .graphemes_start()
    }

    /// Get a cursor to walk graphemes from the logical end of this atom.
    #[inline(always)]
    pub fn graphemes_end(&self) -> Graphemes<'a> {
        self.slice
            .narrow(self.clusters.0..self.clusters.1)
            .graphemes_end()
    }

    /// The number of graphemes in this atom.
    #[inline]
    pub fn grapheme_count(&self) -> u32 {
        self.slice.characters[self.chars.0 as usize..self.chars.1 as usize]
            .iter()
            .filter(|c| c.grapheme_start)
            .count() as u32
    }

    /// Whether the atom can be broken
    #[inline(always)]
    pub fn boundary_before(&self) -> Boundary {
        self.slice.characters[self.chars.0 as usize].info.boundary()
    }

    /// Whether the atom can be broken
    #[inline(always)]
    pub fn is_safe_to_break_before(&self) -> bool {
        self.slice.shaped_clusters[self.clusters.0 as usize].is_safe_to_break_before()
    }

    /// The atom's total advance.
    ///
    /// This is the sum of the atom's clusters' advances.
    #[inline(always)]
    pub fn advance(&self) -> f32 {
        self.advance
    }

    /// Get a cursor to walk atoms, starting logically before this atom.
    pub fn cursor_before(&self) -> Atoms<'a> {
        Atoms {
            slice: self.slice,
            cluster_idx: self.clusters.0,
        }
    }

    /// Get a cursor to walk atoms, starting logically after this atom.
    pub fn cursor_after(&self) -> Atoms<'a> {
        Atoms {
            slice: self.slice,
            cluster_idx: self.clusters.1,
        }
    }
}

/// A [`Grapheme`] cursor.
///
/// This cursor can walk forwards and backwards over graphemes.
#[derive(Copy, Clone, Debug)]
pub struct Graphemes<'a> {
    slice: ShapedSlice<'a>,

    // /// The characters these graphemes span inside [`ShapedSlice::characters`].
    // char_range: (u32, u32),
    /// The gap we're pointing at into [`ShapedSlice::characters`].
    char_idx: u32,

    /// The gap we're pointing at into [`ShapedSlice::shaped_clusters`].
    cluster_idx: u32,

    /// The partial advance of `cluster_idx` shared among the graphemes it overlaps.
    partial_advance: f32,
}

impl<'a> Graphemes<'a> {
    /// Get the grapheme logically preceding the cursor.
    ///
    /// See also [`Self::next`].
    #[inline]
    pub fn prev(&mut self) -> Option<Grapheme> {
        let char_start = self.slice.char_range().0;
        if self.char_idx == char_start {
            return None;
        }
        let grapheme_end = self.char_idx;
        let mut idx = grapheme_end - 1;
        if idx < self.slice.shaped_clusters[self.cluster_idx as usize].char_start {
            self.cluster_idx -= 1;
            self.partial_advance = self.slice.partial_advance_at(self.cluster_idx);
        }
        let mut advance = self.partial_advance;
        while idx > char_start && !self.slice.characters[idx as usize].grapheme_start {
            idx -= 1;
            if idx < self.slice.shaped_clusters[self.cluster_idx as usize].char_start {
                // The grapheme continues into the preceding cluster.
                self.cluster_idx -= 1;
                self.partial_advance = self.slice.partial_advance_at(self.cluster_idx);
                advance += self.partial_advance;
            }
        }
        // let first = self.slice.characters[idx as usize];
        self.char_idx = idx;
        Some(Grapheme {
            chars: (idx, grapheme_end),
            advance,
            flags: 0,
        })
    }
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = Grapheme;

    /// Get the grapheme logically following the cursor.
    ///
    /// See also [`Self::prev`].
    fn next(&mut self) -> Option<Grapheme> {
        let char_end = self.slice.char_range().1;
        if self.char_idx == char_end {
            return None;
        }
        let grapheme_start = self.char_idx;
        let mut advance = self.partial_advance;
        let mut idx = grapheme_start + 1;
        while idx < char_end && !self.slice.characters[idx as usize].grapheme_start {
            if idx == self.slice.shaped_clusters[self.cluster_idx as usize].char_end {
                // The grapheme continues into the next cluster.
                self.cluster_idx += 1;
                self.partial_advance = self.slice.partial_advance_at(self.cluster_idx);
                advance += self.partial_advance;
            }
            idx += 1;
        }
        // If we stopped exactly on a cluster edge, the next cluster's share
        // belongs to the *next* grapheme: advance the lockstep without taking it.
        let atom_end = idx == char_end
            || idx == self.slice.shaped_clusters[self.cluster_idx as usize].char_end;
        if idx < char_end && idx == self.slice.shaped_clusters[self.cluster_idx as usize].char_end {
            self.cluster_idx += 1;
            self.partial_advance = self.slice.partial_advance_at(self.cluster_idx);
        }
        // let first = self.slice.characters[grapheme_start as usize];
        self.char_idx = idx;
        Some(Grapheme {
            chars: (grapheme_start, idx),
            advance,
            flags: 0,
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            0,
            Some(self.slice.characters.len() - self.char_idx as usize),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grapheme {
    /// The characters this grapheme spans inside [`ShapedSlice::characters`].
    chars: (u32, u32),
    
    /// The advance of the grapheme.
    ///
    /// This is the sum of the grapheme's clusters advances. If clusters cross the boundaries of
    /// this grapheme, this includes partial cluster advances.
    advance: f32,
    flags: u16,
}

impl Grapheme {
    const ATOM_START: u8 = 1;
}

#[cfg(test)]
mod tests {
    use alloc::{sync::Arc, vec, vec::Vec};

    use fontique::Synthesis;
    use linebender_resource_handle::{Blob, FontData};

    use crate::{
        Analysis, AnalysisOptions, Analyzer, FontInstance, ShapeOptions, ShapedText, Shaper,
        itemize::Item,
    };

    const ROBOTO: &[u8] =
        include_bytes!("../../../parley_dev/assets/fonts/roboto_fonts/Roboto-Regular.ttf");

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

    fn shape_item_with_font(
        text: &str,
        analysis: &Analysis,
        item: &Item,
        font: &FontInstance,
        shaper: &mut Shaper,
        shaped: &mut ShapedText,
    ) {
        let char_style_indices = vec![0; text.chars().count()];
        shaper.shape_item(
            text,
            analysis,
            item,
            &ShapeOptions {
                font_size: 32.0,
                language: None,
                features: &[],
                variations: &[],
                char_style_indices: &char_style_indices,
            },
            |_| Some(font.clone()),
            shaped,
        );
    }

    fn shape_with_font(text: &str, font_data: &'static [u8]) -> ShapedText {
        let analysis = analyze(text);
        let font = font_instance(font_data);
        let mut shaper = Shaper::default();
        let mut shaped = ShapedText::new();
        for item in analysis.itemize(text, |_| false) {
            shape_item_with_font(text, &analysis, &item, &font, &mut shaper, &mut shaped);
        }
        shaped
    }

    #[test]
    fn select_atom_at() {
        let shaped = shape_with_font("ffi éa\u{0301}", ROBOTO);
        // byte offsets:              0123467     ..9
        // chars:                     0123456     ..7
        // clusters:                  0001233     ..4

        let slice = shaped.run_slice(0);
        assert_eq!(slice.atom_at_char(0).unwrap().clusters_range(), 0..1);
        assert_eq!(slice.atom_at_char(1).unwrap().clusters_range(), 0..1);
        assert_eq!(slice.atom_at_char(2).unwrap().clusters_range(), 0..1);
        assert_eq!(slice.atom_at_char(3).unwrap().clusters_range(), 1..2);
        assert!(slice.atom_at_char(8).is_none());

        assert_eq!(slice.atom_at_text_byte(0).unwrap().clusters_range(), 0..1);
        assert_eq!(slice.atom_at_text_byte(1).unwrap().clusters_range(), 0..1);
        assert_eq!(slice.atom_at_text_byte(2).unwrap().clusters_range(), 0..1);
        assert_eq!(slice.atom_at_text_byte(3).unwrap().clusters_range(), 1..2);
        assert_eq!(slice.atom_at_text_byte(4).unwrap().clusters_range(), 2..3);
        assert_eq!(slice.atom_at_text_byte(5).unwrap().clusters_range(), 2..3);
        assert_eq!(slice.atom_at_text_byte(6).unwrap().clusters_range(), 3..4);
        assert_eq!(slice.atom_at_text_byte(7).unwrap().clusters_range(), 3..4);
        assert_eq!(slice.atom_at_text_byte(8).unwrap().clusters_range(), 3..4);
        assert!(slice.atom_at_text_byte(9).is_none());
    }
}
