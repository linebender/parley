// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;

use crate::TextStorage;

/// The errors that might happen as a result of [applying] an attribute.
///
/// [applying]: AttributedText::apply_attribute
///
/// TODO: impl Error for this.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ApplyAttributeError {
    /// The bounds given were invalid.
    ///
    /// TODO: Store some data about this here.
    InvalidBounds,
}

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

    /// Apply an `attribute` to a `range` within the text.
    pub fn apply_attribute(
        &mut self,
        range: Range<usize>,
        attribute: Attr,
    ) -> Result<(), ApplyAttributeError> {
        let text_len = self.text.len();
        if range.start > text_len || range.end > text_len {
            return Err(ApplyAttributeError::InvalidBounds);
        }
        self.attributes.push((range, attribute));
        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::{ApplyAttributeError, AttributedText};
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

        assert_eq!(at.apply_attribute(1..3, TestAttribute::Keep), Ok(()));
        assert_eq!(at.apply_attribute(2..5, TestAttribute::Remove), Ok(()));

        assert!(at.attributes_at(0).collect::<Vec<_>>().is_empty());
    }

    #[test]
    fn bad_range_for_apply_attribute() {
        let t = "Hello!";
        let mut at = AttributedText::new(t);

        assert_eq!(at.apply_attribute(0..3, TestAttribute::Keep), Ok(()));
        assert_eq!(at.apply_attribute(0..6, TestAttribute::Keep), Ok(()));
        assert_eq!(
            at.apply_attribute(0..7, TestAttribute::Keep),
            Err(ApplyAttributeError::InvalidBounds)
        );
        assert_eq!(
            at.apply_attribute(7..8, TestAttribute::Keep),
            Err(ApplyAttributeError::InvalidBounds)
        );
    }
}
