// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

use crate::text_range::validate_range;
use crate::{Error, TextRange, TextStorage};

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

    /// Replaces the underlying text and clears all applied attribute spans.
    ///
    /// This retains the allocated storage for spans so the same `AttributedText` value can be
    /// reused across rebuilds.
    #[inline]
    pub fn set_text(&mut self, text: T) {
        self.text = text;
        self.attributes.clear();
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

    /// Apply an `attribute` to a validated [`TextRange`] within the text.
    #[inline]
    pub fn apply_attribute(&mut self, range: TextRange, attribute: Attr) {
        self.attributes.push((range.into(), attribute));
    }

    /// Apply an `attribute` to a byte range within the text.
    ///
    /// This validates the range (bounds + UTF-8 codepoint boundaries) before applying it.
    #[inline]
    pub fn apply_attribute_bytes(
        &mut self,
        range: Range<usize>,
        attribute: Attr,
    ) -> Result<(), Error> {
        validate_range(&self.text, &range)?;
        self.attributes.push((range, attribute));
        Ok(())
    }

    /// Iterate over all attributes and the ranges they apply to.
    ///
    /// Attributes are yielded in the order they were applied.
    #[inline]
    pub fn attributes_iter(&self) -> impl ExactSizeIterator<Item = (&Range<usize>, &Attr)> {
        self.attributes.iter().map(|(range, attr)| (range, attr))
    }

    /// Get an iterator over the attributes (and their ranges) that apply at the given `index`.
    ///
    /// Attributes are yielded in the order they were applied. This doesn't handle conflicting
    /// attributes, it just reports everything.
    ///
    /// This performs a full scan of all attributes on each call (`O(n)` in applied span count).
    pub fn attributes_at(&self, index: usize) -> impl Iterator<Item = (&Range<usize>, &Attr)> {
        self.attributes.iter().filter_map(move |(attr_span, attr)| {
            if attr_span.contains(&index) {
                Some((attr_span, attr))
            } else {
                None
            }
        })
    }

    /// Get an iterator over the attributes (and their ranges) that overlap the given `range`.
    ///
    /// Attributes are yielded in the order they were applied. This doesn't handle conflicting
    /// attributes, it just reports everything.
    ///
    /// This performs a full scan of all attributes on each call (`O(n)` in applied span count).
    ///
    /// Use this for one-off overlap queries. For many queries over the same text, prefer
    /// segment-based iteration with [`AttributeSegmentsWorkspace`](crate::AttributeSegmentsWorkspace):
    /// segment once, then intersect your query ranges with yielded segments (which also provides
    /// the exact covered subranges).
    pub fn attributes_for_range(
        &self,
        range: Range<usize>,
    ) -> impl Iterator<Item = (&Range<usize>, &Attr)> {
        self.attributes.iter().filter_map(move |(attr_span, attr)| {
            if (attr_span.start < range.end) && (attr_span.end > range.start) {
                Some((attr_span, attr))
            } else {
                None
            }
        })
    }

    /// Returns the number of attribute spans applied to the text.
    pub fn attributes_len(&self) -> usize {
        self.attributes.len()
    }

    /// Returns the `(range, attribute)` pair at the given insertion-order span index.
    #[inline]
    pub(crate) fn attribute_at_idx(&self, index: usize) -> Option<(&Range<usize>, &Attr)> {
        self.attributes
            .get(index)
            .map(|(range, attr)| (range, attr))
    }

    /// Remove all applied attribute spans.
    pub fn clear_attributes(&mut self) {
        self.attributes.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::{AttributedText, Endpoint, ErrorKind, TextRange};
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

        at.apply_attribute(
            TextRange::new(at.text(), 1..3).unwrap(),
            TestAttribute::Keep,
        );
        at.apply_attribute(
            TextRange::new(at.text(), 2..5).unwrap(),
            TestAttribute::Remove,
        );

        // Index 0 is outside both spans.
        assert!(at.attributes_at(0).collect::<Vec<_>>().is_empty());

        // Index 2 is in both spans; returns ranges and attrs in application order.
        let at_2: Vec<_> = at.attributes_at(2).collect();
        assert_eq!(at_2.len(), 2);
        assert_eq!(at_2[0], (&(1..3), &TestAttribute::Keep));
        assert_eq!(at_2[1], (&(2..5), &TestAttribute::Remove));

        // Index 4 is only in the second span.
        let at_4: Vec<_> = at.attributes_at(4).collect();
        assert_eq!(at_4.len(), 1);
        assert_eq!(at_4[0], (&(2..5), &TestAttribute::Remove));
    }

    #[test]
    fn attributes_for_range() {
        let t = "Hello!";
        let mut at = AttributedText::new(t);

        at.apply_attribute(
            TextRange::new(at.text(), 1..3).unwrap(),
            TestAttribute::Keep,
        );
        at.apply_attribute(
            TextRange::new(at.text(), 4..6).unwrap(),
            TestAttribute::Remove,
        );

        // Range overlapping only the first span.
        let r: Vec<_> = at.attributes_for_range(0..2).collect();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0], (&(1..3), &TestAttribute::Keep));

        // Range overlapping both spans.
        let r: Vec<_> = at.attributes_for_range(2..5).collect();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0], (&(1..3), &TestAttribute::Keep));
        assert_eq!(r[1], (&(4..6), &TestAttribute::Remove));

        // Range between the two spans, overlapping neither.
        let r: Vec<_> = at.attributes_for_range(3..4).collect();
        assert!(r.is_empty());
    }

    #[test]
    fn apply_attribute_bytes_propagates_validation_errors() {
        let t = "Hello!";
        let mut at = AttributedText::new(t);

        at.apply_attribute(
            TextRange::new(at.text(), 0..3).unwrap(),
            TestAttribute::Keep,
        );
        at.apply_attribute(
            TextRange::new(at.text(), 0..6).unwrap(),
            TestAttribute::Keep,
        );
        match at.apply_attribute_bytes(0..7, TestAttribute::Keep) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::InvalidBounds);
                assert_eq!(e.start(), 0);
                assert_eq!(e.end(), 7);
                assert_eq!(e.len(), 6);
            }
            _ => panic!("expected InvalidBounds"),
        }

        // "é" is 2 bytes in UTF-8; index 1 is not a boundary.
        let t = "éclair";
        let mut at = AttributedText::new(t);
        match at.apply_attribute_bytes(1..2, TestAttribute::Keep) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::NotOnCharBoundary);
                let b = e.boundary().expect("boundary info");
                assert_eq!(b.which, Endpoint::Start);
                assert_eq!(b.index, 1);
            }
            _ => panic!("expected NotOnCharBoundary"),
        }
    }

    #[test]
    fn text_range_can_be_validated_once() {
        let t = "Hello!";
        let mut at = AttributedText::new(t);
        let range = TextRange::new(at.text(), 1..3).unwrap();
        at.apply_attribute(range, TestAttribute::Keep);
        assert_eq!(at.attributes_len(), 1);
    }

    #[test]
    fn set_text_clears_attributes() {
        let mut at = AttributedText::new("Hello!");
        at.apply_attribute(
            TextRange::new(at.text(), 0..5).unwrap(),
            TestAttribute::Keep,
        );
        assert_eq!(at.attributes_len(), 1);

        at.set_text("Replaced");
        assert_eq!(at.text(), &"Replaced");
        assert_eq!(at.attributes_len(), 0);
    }
}
