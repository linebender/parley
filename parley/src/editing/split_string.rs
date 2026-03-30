// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::{fmt::Display, ops::Range};

/// A string which is potentially discontiguous in memory.
///
/// This is returned by [`crate::PlainEditor::text`], as transient composition
/// text may need to be efficiently excluded from its return value.
#[derive(Debug, Clone, Copy)]
pub struct SplitString<'source>(pub(crate) [&'source str; 2]);

/// Text offset encoding for converting host-provided positions into UTF-8 byte
/// ranges over visible editor text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextIndexEncoding {
    /// Offsets are already UTF-8 byte indices.
    Utf8Bytes,
    /// Offsets are UTF-16 code-unit indices.
    Utf16CodeUnits,
    /// Offsets are Unicode scalar-value counts (`char`s in Rust terms).
    UnicodeCodePoints,
}

impl<'source> SplitString<'source> {
    /// Get the characters of this string.
    pub fn chars(self) -> impl Iterator<Item = char> + 'source {
        self.into_iter().flat_map(str::chars)
    }

    /// Convert encoded text offsets into a UTF-8 byte range over this visible
    /// text.
    ///
    /// Returns `None` if the offsets are reversed, out of bounds, or do not
    /// land on valid boundaries for the provided encoding.
    pub fn to_utf8_range(
        self,
        start: u32,
        end: u32,
        encoding: TextIndexEncoding,
    ) -> Option<Range<usize>> {
        let start = self.offset_to_utf8(start, encoding)?;
        let end = self.offset_to_utf8(end, encoding)?;
        (start <= end).then_some(start..end)
    }

    fn len(self) -> usize {
        self.0[0].len() + self.0[1].len()
    }

    fn is_char_boundary(self, offset: usize) -> bool {
        let mut prefix = 0_usize;
        for segment in self {
            let end = prefix + segment.len();
            if offset < end {
                return segment.is_char_boundary(offset - prefix);
            }
            if offset == end {
                return true;
            }
            prefix = end;
        }
        offset == prefix
    }

    fn offset_to_utf8(self, offset: u32, encoding: TextIndexEncoding) -> Option<usize> {
        match encoding {
            TextIndexEncoding::Utf8Bytes => split_utf8_offset_to_utf8_byte_offset(self, offset),
            TextIndexEncoding::Utf16CodeUnits => {
                split_utf16_offset_to_utf8_byte_offset(self, offset)
            }
            TextIndexEncoding::UnicodeCodePoints => {
                split_code_point_offset_to_utf8_byte_offset(self, offset)
            }
        }
    }
}

impl PartialEq<&'_ str> for SplitString<'_> {
    fn eq(&self, other: &&'_ str) -> bool {
        let [a, b] = self.0;
        let mid = a.len();
        match other.split_at_checked(mid) {
            Some((a_1, b_1)) => a_1 == a && b_1 == b,
            None => false,
        }
    }
}

// We intentionally choose not to:
// impl PartialEq<Self> for SplitString<'_> {}
// for simplicity, as the impl wouldn't be useful and is non-trivial

impl Display for SplitString<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [a, b] = self.0;
        write!(f, "{a}{b}")
    }
}

/// Iterate through the source strings.
impl<'source> IntoIterator for SplitString<'source> {
    type Item = &'source str;
    type IntoIter = <[&'source str; 2] as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

fn split_utf8_offset_to_utf8_byte_offset(text: SplitString<'_>, utf8_offset: u32) -> Option<usize> {
    let offset = usize::try_from(utf8_offset).ok()?;
    (offset <= text.len() && text.is_char_boundary(offset)).then_some(offset)
}

fn split_utf16_offset_to_utf8_byte_offset(
    text: SplitString<'_>,
    utf16_offset: u32,
) -> Option<usize> {
    if utf16_offset == 0 {
        return Some(0);
    }
    let target = usize::try_from(utf16_offset).ok()?;
    let mut utf16_count = 0_usize;
    let mut byte_offset = 0_usize;
    for segment in text {
        for ch in segment.chars() {
            if utf16_count == target {
                // The requested UTF-16 boundary can fall exactly at a split
                // seam, before the next segment contributes any code units.
                return Some(byte_offset);
            }
            utf16_count = utf16_count.checked_add(ch.len_utf16())?;
            byte_offset = byte_offset.checked_add(ch.len_utf8())?;
            if utf16_count == target {
                return Some(byte_offset);
            }
        }
    }
    (utf16_count == target).then_some(byte_offset)
}

fn split_code_point_offset_to_utf8_byte_offset(
    text: SplitString<'_>,
    code_point_offset: u32,
) -> Option<usize> {
    if code_point_offset == 0 {
        return Some(0);
    }
    let target = usize::try_from(code_point_offset).ok()?;
    let mut code_point_count = 0_usize;
    let mut byte_offset = 0_usize;
    for segment in text {
        for ch in segment.chars() {
            if code_point_count == target {
                return Some(byte_offset);
            }
            code_point_count = code_point_count.checked_add(1)?;
            byte_offset = byte_offset.checked_add(ch.len_utf8())?;
        }
    }
    (code_point_count == target).then_some(byte_offset)
}
