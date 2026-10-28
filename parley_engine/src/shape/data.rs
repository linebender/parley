// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![expect(missing_docs, reason = "Deferred")]

use core::ops::Range;

use crate::shape::Whitespace;

/// Data for a single character of the source text.
///
/// This is a character in the Unicode scalar value sense, i.e., it corresponds to a single `char`
/// of the source text.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Character {
    pub text_byte_start: u32,
    /// Style index for this character.
    pub style_index: u16,
    /// The whitespace class of this character.
    pub whitespace: Whitespace,
    /// Properties of this character.
    pub flags: CharacterFlags,
    /// Whether this character begins a grapheme cluster.
    pub grapheme_start: bool,
}

impl Character {
    /// The byte range of this character in the source text.
    #[inline(always)]
    pub fn text_byte_range(&self) -> Range<usize> {
        self.text_byte_start as usize..self.text_byte_start as usize + self.flags.len_utf8()
    }

    /// Returns if this character is any whitespace.
    #[inline(always)]
    pub fn is_whitespace(self) -> bool {
        self.whitespace != Whitespace::None
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShapedClusterFlags(u32);

impl ShapedClusterFlags {
    const GLYPH_LEN_MASK: u32 = 0x00FF;
    const INLINE_GLYPH: u32 = 1 << 8;
    const GRAPHEME_START: u32 = 1 << 9;
    /// Whether a soft wrap opportunity exists before the cluster's first character.
    const SOFT_WRAP_BEFORE: u32 = 1 << 10;
    /// [`Whitespace`] class of the cluster's first character (3 bits).
    const WHITESPACE_SHIFT: u32 = 11;
    const WHITESPACE_MASK: u32 = 0b111 << Self::WHITESPACE_SHIFT;
    /// Whether the cluster is an inline object.
    const INLINE_OBJECT: u32 = 1 << 14;
    const SAFE_TO_BREAK_BEFORE: u32 = 1 << 15;
    const SAFE_TO_CONCAT_BEFORE: u32 = 1 << 16;
    /// Whether the cluster's first character is default ignorable.
    const DEFAULT_IGNORABLE: u32 = 1 << 17;

    #[inline(always)]
    pub(crate) const fn new(glyph_len: u8) -> Self {
        Self(glyph_len as u32)
    }

    #[inline(always)]
    pub(crate) const fn with_inline_glyph(mut self, set: bool) -> Self {
        self.0 = self.0 & !Self::INLINE_GLYPH | if set { Self::INLINE_GLYPH } else { 0 };
        self
    }

    #[inline(always)]
    pub(crate) const fn with_grapheme_start(mut self, set: bool) -> Self {
        self.0 = self.0 & !Self::GRAPHEME_START | if set { Self::GRAPHEME_START } else { 0 };
        self
    }

    #[inline(always)]
    pub(crate) const fn with_safe_to_break_before(mut self, set: bool) -> Self {
        self.0 =
            self.0 & !Self::SAFE_TO_BREAK_BEFORE | if set { Self::SAFE_TO_BREAK_BEFORE } else { 0 };
        self
    }

    #[inline(always)]
    pub(crate) const fn with_safe_to_concat_before(mut self, set: bool) -> Self {
        self.0 = self.0 & !Self::SAFE_TO_CONCAT_BEFORE
            | if set { Self::SAFE_TO_CONCAT_BEFORE } else { 0 };
        self
    }

    #[inline(always)]
    pub(crate) const fn with_inline_object(mut self, set: bool) -> Self {
        self.0 = self.0 & !Self::INLINE_OBJECT | if set { Self::INLINE_OBJECT } else { 0 };
        self
    }

    #[inline(always)]
    pub(crate) const fn with_first_char(
        mut self,
        soft_wrap_opportunity: bool,
        whitespace: Whitespace,
        default_ignorable: bool,
    ) -> Self {
        self.0 = self.0
            & !(Self::SOFT_WRAP_BEFORE | Self::WHITESPACE_MASK | Self::DEFAULT_IGNORABLE)
            | if soft_wrap_opportunity {
                Self::SOFT_WRAP_BEFORE
            } else {
                0
            }
            | ((whitespace as u32) << Self::WHITESPACE_SHIFT)
            | if default_ignorable {
                Self::DEFAULT_IGNORABLE
            } else {
                0
            };
        self
    }

    #[inline(always)]
    const fn is_soft_wrap_opportunity_before(self) -> bool {
        self.0 & Self::SOFT_WRAP_BEFORE != 0
    }

    #[inline(always)]
    const fn whitespace(self) -> Whitespace {
        Whitespace::from_u8(((self.0 & Self::WHITESPACE_MASK) >> Self::WHITESPACE_SHIFT) as u8)
    }

    #[inline(always)]
    const fn is_default_ignorable(self) -> bool {
        self.0 & Self::DEFAULT_IGNORABLE != 0
    }

    #[inline(always)]
    const fn glyph_len(self) -> u8 {
        (self.0 & Self::GLYPH_LEN_MASK) as u8
    }

    #[inline(always)]
    const fn has_inline_glyph(self) -> bool {
        self.0 & Self::INLINE_GLYPH != 0
    }

    #[inline(always)]
    const fn is_grapheme_start(self) -> bool {
        self.0 & Self::GRAPHEME_START != 0
    }

    #[inline(always)]
    const fn is_safe_to_break_before(self) -> bool {
        self.0 & Self::SAFE_TO_BREAK_BEFORE != 0
    }

    #[inline(always)]
    const fn is_safe_to_concat_before(self) -> bool {
        self.0 & Self::SAFE_TO_CONCAT_BEFORE != 0
    }

    #[inline(always)]
    const fn is_inline_object(self) -> bool {
        self.0 & Self::INLINE_OBJECT != 0
    }
}

/// A span of characters and the glyphs they shaped into.
///
/// Shaping may reorder, compose, decompose, etc.; there is no finer-grained correspondence between
/// characters and glyphs. For example, a base letter with a combining mark may shape into a single
/// glyph, a single character may shape into multiple glyphs, and a ligature may combine multiple
/// characters into shared glyphs.
///
/// Shaped cluster boundaries are not necessarily [`Grapheme`][crate::Grapheme] boundaries. The
/// shared boundaries are encoded by [`Atom`][crate::Atom]s.
///
/// For more information about clusters, see [HarfBuzz's documentation][harfbuzz].
///
/// [harfbuzz]: https://harfbuzz.github.io/working-with-harfbuzz-clusters.html
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShapedCluster {
    /// The characters of this cluster, as a range into
    /// [`ShapedText::characters`](crate::ShapedText::characters).
    //
    // TODO: this currently stores the full range, but perhaps we could store only the start index.
    // See <https://github.com/linebender/parley/pull/715#discussion_r3693794119>.
    pub(crate) chars_range: (u32, u32),

    /// Style index for this cluster.
    pub style_index: u16,

    /// The index into the glyph array where this cluster's glyphs start.
    ///
    /// If [`Self::has_inline_glyph`] is `true`, this is a glyph identifier instead. For more, see
    /// the documentation on that method.
    pub glyph_offset: u32,

    pub(crate) flags: ShapedClusterFlags,

    /// Advance width for this cluster
    pub advance: f32,
}

impl ShapedCluster {
    /// The characters of this cluster, as a range into [`ShapedText::characters`](crate::ShapedText::characters).
    ///
    /// This is also the range of character positions in the source text (in the Unicode scalar
    /// value sense).
    #[inline(always)]
    pub fn chars_range(&self) -> Range<u32> {
        self.chars_range.0..self.chars_range.1
    }

    /// The number of glyphs of this cluster.
    #[inline(always)]
    pub fn glyph_len(self) -> u8 {
        self.flags.glyph_len()
    }

    /// Whether this cluster's glyph is stored inline in [`Self::glyph_offset`].
    ///
    /// This is only possible if [`Self::glyph_len`] is one, and the glyph has no offset.
    /// [`Self::glyph_offset`] then encodes the glyph identifier rather than an index into the glyph
    /// array. The glyph's advance then is this cluster's advance.
    #[inline(always)]
    pub fn has_inline_glyph(self) -> bool {
        self.flags.has_inline_glyph()
    }

    /// Whether this shaped cluster's logical start also starts a grapheme.
    #[inline(always)]
    pub fn is_grapheme_start(self) -> bool {
        self.flags.is_grapheme_start()
    }

    /// Whether the text can be broken logically before this shaped cluster without reshaping.
    ///
    /// If so, reshaping the text on either side of the break, would result in the same glyphs and
    /// advances. For example, if a font kerns the glyphs of "To", the "o" is not safe to break
    /// before. Breaking inside a shaped cluster (such as between the letters of an "ffi" ligature)
    /// always requires shaping.
    #[inline(always)]
    pub fn is_safe_to_break_before(self) -> bool {
        self.flags.is_safe_to_break_before()
    }

    /// Whether shaping did not look across this shaped cluster's logical start.
    ///
    /// For example, if a font kerns "T" with some letters but not "e", the "e" in "Te" is safe to
    /// break before, but not to concatenate before: say "To" would get kerning applied,
    /// concatenating the "T" with "o" instead would miss that kern.
    ///
    /// If concatenating is safe for this cluster, [`Self::is_safe_to_break_before`] will also be
    /// true.
    ///
    /// If the text was shaped without these flags, this is always `false`. See the
    /// `produce_safe_to_concat_flags` argument of
    /// [`Shaper::shape_text`](crate::Shaper::shape_text).
    #[inline(always)]
    pub fn is_safe_to_concat_before(self) -> bool {
        self.flags.is_safe_to_concat_before()
    }

    /// Whether a soft wrap opportunity exists before this cluster's first character.
    ///
    /// This is [`Character::flags`]' soft-wrap bit of the first character of [`Self::chars_range`],
    /// cached here so that measuring text does not need to touch the character array.
    #[inline(always)]
    pub fn is_soft_wrap_opportunity_before(self) -> bool {
        self.flags.is_soft_wrap_opportunity_before()
    }

    /// The [`Whitespace`] class of this cluster's first character.
    ///
    /// Cached here so that measuring text does not need to read the character array.
    #[inline(always)]
    pub fn whitespace(self) -> Whitespace {
        self.flags.whitespace()
    }

    /// Whether this cluster's first character's [`CharacterFlags::is_default_ignorable`] flag is `true`.
    ///
    /// Cached here so that measuring text does not need to read the character array.
    #[inline(always)]
    pub fn is_default_ignorable(self) -> bool {
        self.flags.is_default_ignorable()
    }

    /// Whether this cluster is an inline object.
    ///
    /// An inline object's cluster consists of just its U+FFFC OBJECT REPLACEMENT CHARACTER (see
    /// [`AnalysisOptions::inline_objects`]), and it is an [`Atom`] of its own. It has no glyphs, and
    /// its advance is zero until the caller, which lays out the inline object, sets it with
    /// [`ShapedText::set_inline_object_advance`].
    ///
    /// [`AnalysisOptions::inline_objects`]: crate::AnalysisOptions::inline_objects
    /// [`Atom`]: crate::Atom
    /// [`ShapedText::set_inline_object_advance`]: crate::ShapedText::set_inline_object_advance
    #[inline(always)]
    pub fn is_inline_object(self) -> bool {
        self.flags.is_inline_object()
    }

    /// The number of characters in this cluster.
    #[inline(always)]
    pub fn char_len(self) -> u32 {
        self.chars_range.1 - self.chars_range.0
    }

    /// The number of graphemes this cluster overlaps.
    pub(crate) fn graphemes_overlapped(&self, characters: &[Character]) -> u32 {
        let start = self.chars_range().start as usize + 1;
        let end = self.chars_range().end as usize;
        let mut graphemes = 1;
        for character in &characters[start..end] {
            graphemes += u32::from(character.grapheme_start);
        }
        graphemes
    }
}

/// Properties of a [`Character`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CharacterFlags(u8);

impl CharacterFlags {
    /// Bits 0..2: the UTF-8 length of the source character minus one.
    ///
    /// (Note the UTF-8 length of any character is between 1 and 4 inclusive.)
    const LEN_UTF8_MASK: u8 = 0b11;
    /// Whether the source character is an emoji.
    const EMOJI: u8 = 1 << 2;
    /// Whether there is a word boundary before the character.
    const WORD_BOUNDARY: u8 = 1 << 3;
    /// Whether there is a soft wrap opportunity before the character.
    const SOFT_WRAP_OPPORTUNITY: u8 = 1 << 4;
    /// Whether the character stands in for an inline object.
    const INLINE_OBJECT: u8 = 1 << 5;
    /// Whether the character has the `Default_Ignorable_Code_Point` unicode property.
    const DEFAULT_IGNORABLE: u8 = 1 << 6;

    #[inline(always)]
    pub(crate) fn new(
        source_char: char,
        is_word_boundary: bool,
        is_soft_wrap_opportunity: bool,
        is_default_ignorable: bool,
    ) -> Self {
        // TODO: Defer to ICU4X properties (see: https://docs.rs/icu/latest/icu/properties/props/struct.Emoji.html).
        let is_emoji = matches!(source_char as u32, 0x1F600..=0x1F64F | 0x1F300..=0x1F5FF | 0x1F680..=0x1F6FF | 0x2600..=0x26FF | 0x2700..=0x27BF);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "`len_utf8` is between 1 and 4 inclusive"
        )]
        let len_utf8 = source_char.len_utf8() as u8;
        Self(
            (len_utf8 - 1)
                | if is_emoji { Self::EMOJI } else { 0 }
                | if is_word_boundary {
                    Self::WORD_BOUNDARY
                } else {
                    0
                }
                | if is_soft_wrap_opportunity {
                    Self::SOFT_WRAP_OPPORTUNITY
                } else {
                    0
                }
                | if is_default_ignorable {
                    Self::DEFAULT_IGNORABLE
                } else {
                    0
                },
        )
    }

    /// Whether there is a soft wrap opportunity before this character ([UAX #14][line-breaking]).
    ///
    /// Note mandatory breaks (like `\n`) are encoded as [`Whitespace`].
    ///
    /// [line-breaking]: https://www.unicode.org/reports/tr14/
    #[inline(always)]
    pub fn is_soft_wrap_opportunity(self) -> bool {
        self.0 & Self::SOFT_WRAP_OPPORTUNITY != 0
    }

    /// Returns if there is a word boundary before the character.
    #[inline(always)]
    pub fn is_word_boundary(self) -> bool {
        self.0 & Self::WORD_BOUNDARY != 0
    }

    /// Returns if the character is an emoji.
    #[inline(always)]
    pub fn is_emoji(self) -> bool {
        self.0 & Self::EMOJI != 0
    }

    /// Returns if the character is default ignorable.
    ///
    /// These are the characters with the Unicode [`Default_Ignorable_Code_Point`] property.
    /// This property is used in Parley to disable letter-spacing for some atoms, matching Blink.
    ///
    /// [`Default_Ignorable_Code_Point`]: https://www.unicode.org/reports/tr44/#Default_Ignorable_Code_Point
    #[inline(always)]
    pub fn is_default_ignorable(self) -> bool {
        self.0 & Self::DEFAULT_IGNORABLE != 0
    }

    /// Returns these flags, marking whether the character stands in for an inline object.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn with_inline_object(self, is_inline_object: bool) -> Self {
        Self(
            self.0 & !Self::INLINE_OBJECT
                | if is_inline_object {
                    Self::INLINE_OBJECT
                } else {
                    0
                },
        )
    }

    /// Returns if the character stands in for an inline object.
    ///
    /// See [`AnalysisOptions::inline_objects`](crate::AnalysisOptions::inline_objects).
    #[inline(always)]
    pub fn is_inline_object(self) -> bool {
        self.0 & Self::INLINE_OBJECT != 0
    }

    /// Returns the number of bytes the source character would need if encoded in UTF-8.
    ///
    /// That number of bytes is always between 1 and 4, inclusive.
    #[inline(always)]
    pub fn len_utf8(self) -> usize {
        (self.0 & Self::LEN_UTF8_MASK) as usize + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_flags() {
        for (ch, len, emoji) in [
            ('a', 1, false),
            (' ', 1, false),
            ('\t', 1, false),
            ('\r', 1, false),
            ('\n', 1, false),
            ('\u{00e9}', 2, false),
            ('\u{2028}', 3, false),
            ('\u{2600}', 3, true),
            ('\u{1F600}', 4, true),
        ] {
            for bits in 0..8_u8 {
                let is_word_boundary = bits & 1 != 0;
                let is_soft_wrap_opportunity = bits & 2 != 0;
                let is_default_ignorable = bits & 4 != 0;
                let flags = CharacterFlags::new(
                    ch,
                    is_word_boundary,
                    is_soft_wrap_opportunity,
                    is_default_ignorable,
                );
                assert_eq!(flags.len_utf8(), len, "{ch:?}");
                assert_eq!(flags.is_emoji(), emoji, "{ch:?}");
                assert_eq!(flags.is_word_boundary(), is_word_boundary, "{ch:?}");
                assert_eq!(
                    flags.is_soft_wrap_opportunity(),
                    is_soft_wrap_opportunity,
                    "{ch:?}"
                );
                assert_eq!(flags.is_default_ignorable(), is_default_ignorable, "{ch:?}");
            }

            let character = Character {
                text_byte_start: 3,
                style_index: 7,
                whitespace: Whitespace::from_char(ch),
                flags: CharacterFlags::new(ch, false, false, false),
                grapheme_start: true,
            };
            assert_eq!(character.text_byte_range(), 3..3 + len, "{ch:?}");
        }
    }

    #[test]
    fn first_char_flags_round_trip() {
        let soft_wraps = [false, true];
        let whitespaces = [
            Whitespace::None,
            Whitespace::Space,
            Whitespace::NoBreakSpace,
            Whitespace::IdeographicSpace,
            Whitespace::OtherSpaceSeparator,
            Whitespace::Tab,
            Whitespace::Newline,
            Whitespace::ControlWhitespace,
        ];
        for soft_wrap in soft_wraps {
            for whitespace in whitespaces {
                for default_ignorable in [false, true] {
                    let flags = ShapedClusterFlags::new(3)
                        .with_grapheme_start(true)
                        .with_first_char(soft_wrap, whitespace, default_ignorable);
                    assert_eq!(flags.is_soft_wrap_opportunity_before(), soft_wrap);
                    assert_eq!(flags.whitespace(), whitespace);
                    assert_eq!(flags.is_default_ignorable(), default_ignorable);
                    assert_eq!(flags.glyph_len(), 3);
                    assert!(flags.is_grapheme_start());
                }
            }
        }
    }
}
