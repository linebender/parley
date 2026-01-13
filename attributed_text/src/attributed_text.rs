// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

use crate::{Endpoint, Error, TextStorage};

/// A block of text with attributes applied to ranges within the text.
#[derive(Debug)]
pub struct AttributedText<T: Debug + TextStorage, Attr: Debug> {
    text: T,
    attributes: Vec<(Range<usize>, Attr)>,
}

impl<T: Debug + TextStorage, Attr: Debug> AttributedText<T, Attr> {
    /// Create an `AttributedText` with no attributes applied.
    pub fn new(text: T) -> Self {
        Self {
            text,
            attributes: Vec::default(),
        }
    }

    /// Borrow the underlying text storage.
    pub fn text(&self) -> &T {
        &self.text
    }

    /// Returns the length of the underlying text, in bytes.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns `true` if the underlying text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Borrow the underlying text as `&str` when the storage is contiguous.
    pub fn as_str(&self) -> &str
    where
        T: AsRef<str>,
    {
        self.text.as_ref()
    }

    /// Apply an `attribute` to a `range` within the text.
    pub fn apply_attribute(&mut self, range: Range<usize>, attribute: Attr) -> Result<(), Error> {
        let text_len = self.text.len();
        if range.start > range.end {
            return Err(Error::invalid_range(range.start, range.end, text_len));
        }
        if range.start > text_len || range.end > text_len {
            return Err(Error::invalid_bounds(range.start, range.end, text_len));
        }
        if !self.text.is_char_boundary(range.start) {
            return Err(Error::not_on_char_boundary(
                &self.text,
                range.start,
                range.end,
                text_len,
                Endpoint::Start,
                range.start,
            ));
        }
        if !self.text.is_char_boundary(range.end) {
            return Err(Error::not_on_char_boundary(
                &self.text,
                range.start,
                range.end,
                text_len,
                Endpoint::End,
                range.end,
            ));
        }
        self.attributes.push((range, attribute));
        Ok(())
    }

    /// Iterate over all attributes and the ranges they apply to.
    ///
    /// Attributes are yielded in the order they were applied.
    pub fn attributes_iter(&self) -> impl ExactSizeIterator<Item = (&Range<usize>, &Attr)> {
        self.attributes.iter().map(|(range, attr)| (range, attr))
    }

    /// Get an iterator over the attributes that apply at the given `index`.
    ///
    /// This doesn't handle conflicting attributes, it just reports everything.
    ///
    /// TODO: Decide if this should also return the spans' ranges.
    pub fn attributes_at(&self, index: usize) -> impl Iterator<Item = &Attr> {
        self.attributes.iter().filter_map(move |(attr_span, attr)| {
            if attr_span.contains(&index) {
                Some(attr)
            } else {
                None
            }
        })
    }

    /// Get an iterator over the attributes that apply to the given `range`.
    ///
    /// This doesn't handle conflicting attributes, it just reports everything.
    ///
    /// TODO: Decide if this should also return the spans' ranges.
    pub fn attributes_for_range(&self, range: Range<usize>) -> impl Iterator<Item = &Attr> {
        self.attributes.iter().filter_map(move |(attr_span, attr)| {
            if (attr_span.start < range.end) && (attr_span.end > range.start) {
                Some(attr)
            } else {
                None
            }
        })
    }

    /// Returns the number of attribute spans applied to the text.
    pub fn attributes_len(&self) -> usize {
        self.attributes.len()
    }

    /// Remove all applied attribute spans.
    pub fn clear_attributes(&mut self) {
        self.attributes.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::{AttributedText, Endpoint, ErrorKind};
    use alloc::format;
    use alloc::vec::Vec;

    #[derive(Debug, PartialEq)]
    enum TestAttribute {
        Keep,
        Remove,
    }

    #[test]
    fn attributes_at() {
        let t = "Hello!";
        let mut at = AttributedText::new(t);

        assert!(at.apply_attribute(1..3, TestAttribute::Keep).is_ok());
        assert!(at.apply_attribute(2..5, TestAttribute::Remove).is_ok());

        assert!(at.attributes_at(0).collect::<Vec<_>>().is_empty());
    }

    #[expect(
        clippy::reversed_empty_ranges,
        reason = "We want an invalid range for testing."
    )]
    #[test]
    fn bad_range_for_apply_attribute() {
        let t = "Hello!";
        let mut at = AttributedText::new(t);

        assert!(at.apply_attribute(0..3, TestAttribute::Keep).is_ok());
        assert!(at.apply_attribute(0..6, TestAttribute::Keep).is_ok());
        match at.apply_attribute(4..3, TestAttribute::Keep) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::InvalidRange);
                let msg = format!("{}", e);
                assert!(msg.contains("4..3"));
                assert!(msg.contains("invalid range"));
                assert!(msg.contains("start > end"));
            }
            _ => panic!("expected InvalidRange"),
        }
        match at.apply_attribute(0..7, TestAttribute::Keep) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::InvalidBounds);
                let msg = format!("{}", e);
                assert!(msg.contains("0..7"));
                assert!(msg.contains("len 6"));
            }
            _ => panic!("expected InvalidBounds"),
        }
        match at.apply_attribute(7..8, TestAttribute::Keep) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::InvalidBounds);
                assert_eq!(e.start(), 7);
                assert_eq!(e.end(), 8);
                assert_eq!(e.len(), 6);
                let msg = format!("{}", e);
                assert!(msg.contains("range 7..8"));
                assert!(msg.contains("len 6"));
            }
            _ => panic!("expected InvalidBounds"),
        }
    }

    #[test]
    fn not_on_char_boundary() {
        // "é" is 2 bytes in UTF-8; index 1 is not a boundary.
        let t = "éclair";
        let mut at = AttributedText::new(t);
        // Invalid start boundary at 1
        match at.apply_attribute(1..2, TestAttribute::Keep) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::NotOnCharBoundary);
                let b = e.boundary().expect("boundary info");
                assert_eq!(b.which, Endpoint::Start);
                assert_eq!(b.index, 1);
                assert_eq!(b.char_start, 0);
                assert_eq!(b.char_end, 2);
                let msg = format!("{}", e);
                assert!(msg.contains("range 1..2"));
                assert!(msg.contains("start"));
                assert!(msg.contains("index 1"));
                assert!(msg.contains("char 0..2"));
            }
            _ => panic!("expected NotOnCharBoundary for start"),
        }
        // Invalid end boundary at 1
        match at.apply_attribute(0..1, TestAttribute::Keep) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::NotOnCharBoundary);
                let b = e.boundary().expect("boundary info");
                assert_eq!(b.which, Endpoint::End);
                assert_eq!(b.index, 1);
                assert_eq!(b.char_start, 0);
                assert_eq!(b.char_end, 2);
                let msg = format!("{}", e);
                assert!(msg.contains("range 0..1"));
                assert!(msg.contains("end"));
                assert!(msg.contains("index 1"));
                assert!(msg.contains("char 0..2"));
            }
            _ => panic!("expected NotOnCharBoundary for end"),
        }
        // Using proper boundaries is OK
        assert!(at.apply_attribute(0..2, TestAttribute::Keep).is_ok());
    }
}
