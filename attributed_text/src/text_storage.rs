// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::sync::Arc;

/// A block of text that will be wrapped by an [`AttributedText`].
///
/// [`AttributedText`]: crate::AttributedText
pub trait TextStorage {
    /// The length of the underlying text.
    fn len(&self) -> usize;

    /// Return `true` if the underlying text is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return whether `index` is a UTF-8 character boundary in the text.
    ///
    /// Implementors may compute this by any means appropriate for the
    /// underlying representation (e.g. [`str::is_char_boundary`] for contiguous
    /// strings, or by inspecting rope chunk boundaries).
    fn is_char_boundary(&self, index: usize) -> bool;
}

impl TextStorage for String {
    fn len(&self) -> usize {
        Self::len(self)
    }

    fn is_char_boundary(&self, index: usize) -> bool {
        self.as_str().is_char_boundary(index)
    }
}

impl TextStorage for &str {
    fn len(&self) -> usize {
        str::len(self)
    }

    fn is_char_boundary(&self, index: usize) -> bool {
        str::is_char_boundary(self, index)
    }
}

impl TextStorage for Arc<str> {
    fn len(&self) -> usize {
        str::len(self)
    }

    fn is_char_boundary(&self, index: usize) -> bool {
        str::is_char_boundary(self, index)
    }
}

#[cfg(test)]
mod tests {
    use super::TextStorage;
    use alloc::string::ToString;
    use alloc::sync::Arc;

    fn assert_boundaries<T: TextStorage>(t: &T, trues: &[usize], falses: &[usize]) {
        for &i in trues {
            assert!(t.is_char_boundary(i), "index {i} should be boundary");
        }
        for &i in falses {
            assert!(!t.is_char_boundary(i), "index {i} should not be boundary");
        }
    }

    #[test]
    fn is_char_boundary_ascii() {
        let s = "abc";
        // All byte positions 0..=len are char boundaries in pure ASCII
        assert_boundaries(&s, &[0, 1, 2, 3], &[4]);
    }

    #[test]
    fn is_char_boundary_multibyte() {
        let s = "éclair"; // first codepoint is 2 bytes
        assert_boundaries(&s, &[0, 2, s.len()], &[1]);

        let owned = s.to_string();
        assert_boundaries(&owned, &[0, 2, owned.len()], &[1]);

        let arc: Arc<str> = Arc::from(s);
        assert_boundaries(&arc, &[0, 2, arc.len()], &[1]);
    }

    #[test]
    fn is_char_boundary_emoji_flag() {
        let s = "🇯🇵"; // two 4-byte codepoints
        assert_eq!(s.len(), 8);
        // Boundaries at 0, 4, 8
        assert_boundaries(&s, &[0, 4, 8], &[1, 2, 3, 5, 6, 7]);
    }
}
