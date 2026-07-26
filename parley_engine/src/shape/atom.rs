// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ops::Range;

use crate::{
    Glyph,
    shape::data::{Character, ShapedCluster},
};

/// A slice of shaped text.
///
// NOTE: The motivation for this type, is that once `parley_engine` supports reshaping, this can
// hold reshaped slices that don't depend on `ShapedText`.
pub struct ShapedSlice<'a> {
    characters: &'a [Character],
    shaped_clusters: &'a [ShapedCluster],
    glyphs: &'a [Glyph],
}

/// An atom of shaped text.
///
/// An atom cannot be broken without reshaping.
pub struct Atom<'a> {
    shaped_slice: ShapedSlice<'a>,
    shaped_cluster_range: Range<u32>,
    num_graphemes: u32,
    advance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grapheme {
    text_char_start: u32,
    text_char_end: u32,
    shaped_cluster_start: u32,
    shaped_cluster_end: u32,

    /// The advance of the grapheme.
    ///
    /// In case this grapheme's atom contains more than one grapheme, this is a partial advance.
    advance: f32,
}

impl<'a> Atom<'a> {
    #[inline]
    pub fn graphemes(&self) -> impl ExactSizeIterator<Item = Grapheme> {}

    #[inline(always)]
    pub fn safe_to_break(&self) -> bool {}
}

impl<'a> ShapedSlice<'a> {
    #[inline]
    pub fn atoms(&self) -> impl DoubleEndedIterator<Item = Atom> {
        AtomsIter {}
    }

    pub fn atoms_from(&self, char_pos: u32) -> impl DoubleEndedIterator<Item = Atom> {
        AtomsIter {}
    }
}

pub struct AtomsIter<'a> {
    shaped_slice: ShapedSlice<'a>,
}

impl Iterator for AtomsIter {
    type Item = Atom;

    #[inline]
    fn next(&mut self) -> Option<Atom> {
        todo!()
    }
}

impl DoubleEndedIterator for AtomsIter {
    #[inline]
    fn next_back(&mut self) -> Option<Atom> {
        todo!()
    }
}
