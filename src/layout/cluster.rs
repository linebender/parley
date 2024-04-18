// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

impl<'a, B: Brush> Cluster<'a, B> {
    /// Returns the range of text that defines the cluster.
    pub fn text_range(&self) -> Range<usize> {
        self.data.text_range(self.run.data)
    }

    /// Returns the advance of the cluster.
    pub fn advance(&self) -> f32 {
        self.data.advance
    }

    /// Returns true if the cluster is the beginning of a ligature.
    pub fn is_ligature_start(&self) -> bool {
        self.data.is_ligature_start()
    }

    /// Returns true if the cluster is a ligature continuation.
    pub fn is_ligature_continuation(&self) -> bool {
        self.data.is_ligature_component()
    }

    /// Returns true if the cluster is a word boundary.
    pub fn is_word_boundary(&self) -> bool {
        self.data.info.is_boundary()
    }

    /// Returns true if the cluster is a soft line break.
    pub fn is_soft_line_break(&self) -> bool {
        self.data.info.boundary() == Boundary::Line
    }

    /// Returns true if the cluster is a hard line break.
    pub fn is_hard_line_break(&self) -> bool {
        self.data.info.boundary() == Boundary::Mandatory
    }

    /// Returns true if the cluster is a space or no-break space.
    pub fn is_space_or_nbsp(&self) -> bool {
        self.data.info.whitespace().is_space_or_nbsp()
    }

    /// Returns an iterator over the glyphs in the cluster.
    pub fn glyphs(&self) -> impl Iterator<Item = Glyph> + 'a + Clone {
        if self.data.glyph_len == 0xFF {
            GlyphIter::Single(Some(Glyph {
                id: self.data.glyph_offset,
                style_index: self.data.style_index,
                x: 0.,
                y: 0.,
                advance: self.data.advance,
            }))
        } else {
            let start = self.run.data.glyph_start + self.data.glyph_offset as usize;
            GlyphIter::Slice(
                self.run.layout.glyphs[start..start + self.data.glyph_len as usize].iter(),
            )
        }
    }

    pub(crate) fn info(&self) -> ClusterInfo {
        self.data.info
    }
}

#[derive(Clone)]
enum GlyphIter<'a> {
    Single(Option<Glyph>),
    Slice(core::slice::Iter<'a, Glyph>),
}

impl<'a> Iterator for GlyphIter<'a> {
    type Item = Glyph;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Single(glyph) => glyph.take(),
            Self::Slice(iter) => {
                let glyph = *iter.next()?;
                Some(glyph)
            }
        }
    }
}
