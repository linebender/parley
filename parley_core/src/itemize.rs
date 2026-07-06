// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Itemization breaks text into individually-shapeable items.

use core::{ops::Range, str::CharIndices};

use icu_properties::props::Script as IcuScript;
use parlance::Script;

use crate::{Analysis, CharInfo, bidi::BidiLevel};

/// A range of text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRange {
    /// The range of byte offsets.
    pub byte_range: Range<usize>,

    /// The range of character indices.
    pub char_range: Range<usize>,
}

/// An item produced by [`Analysis::itemize`].
#[derive(Clone, Debug)]
pub struct Item {
    /// The text range of this item.
    pub range: TextRange,

    /// The bidi level of characters in this item.
    pub bidi_level: BidiLevel,

    /// The script of characters in this item.
    ///
    /// Characters in the source text that do not have a particular script (i.e., they are one of
    /// [`Script::COMMON`], [`Script::UNKNOWN`] or [`Script::INHERITED`]) get their script from
    /// surrounding context. Currently, these just inherit the script of the preceding characters.
    /// Leading characters without a particular script inherit the script of the first character
    /// *with* a particular script. These heuristics may change in the future. For more, see UAX 24
    /// (<https://www.unicode.org/reports/tr24/>) Section 5.
    pub script: Script,
}

/// An iterator over items in text, produced by [`Analysis::itemize`].
pub struct Itemizer<'a, F> {
    /// Our underlying iterator over the input text.
    char_indices: CharIndices<'a>,
    /// The per-char info, parallel to [`Self::char_indices`].
    char_info: &'a [CharInfo],
    /// The per-char bidi level, parallel to [`Self::char_indices`].
    bidi_levels: &'a [BidiLevel],

    /// The paragraph's base bidi level.
    ///
    /// If [`Self::bidi_levels`] is empty as a special case, this is the bidi level of each
    /// character.
    paragraph_bidi_level: BidiLevel,

    /// User-provided itemization split predicate (e.g., if the font size changes).
    split_after: F,

    /// The running character offset of the last-processed item.
    current_char_offset: usize,
    /// The running script of the last-processed item.
    current_script: IcuScript,
}

impl<F> core::fmt::Debug for Itemizer<'_, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Itemizer")
            .field("char_indices", &self.char_indices)
            .field("char_info", &self.char_info)
            .field("bidi_levels", &self.bidi_levels)
            .field("paragraph_bidi_level", &self.paragraph_bidi_level)
            .field("current_char_offset", &self.current_char_offset)
            .field("current_script", &self.current_script)
            .finish_non_exhaustive()
    }
}

impl Analysis {
    /// Itemize the `text` into individually-shapeable runs.
    ///
    /// The `text` passed in must be the same as used for producing the `self` analysis.
    ///
    /// The text is itemized into items of constant bidi level and script. For consecutive
    /// characters where the bidi level and script are unchanging, the `split_after` predicate is
    /// called with the growing item range, and can be used to split on additional properties like
    /// shaping-relevant style changes (e.g., font size) or properties like language.
    ///
    /// Characters that don't have a particular script have their script resolved based on
    /// surrounding context (see [`Item::script`]).
    pub fn itemize<'a, F: FnMut(TextRange) -> bool>(
        &'a self,
        text: &'a str,
        split_after: F,
    ) -> Itemizer<'a, F> {
        let first_real_script = self
            .char_info()
            .iter()
            .map(|x| x.script)
            .find(|&script| real_script(script))
            .unwrap_or(IcuScript::Latin);

        Itemizer {
            char_indices: text.char_indices(),
            char_info: self.char_info(),
            bidi_levels: self.bidi_levels(),
            paragraph_bidi_level: self.paragraph_level(),
            split_after,

            current_char_offset: 0,
            current_script: first_real_script,
        }
    }
}

impl<F: FnMut(TextRange) -> bool> Iterator for Itemizer<'_, F> {
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.char_info.is_empty() {
            // We're already finished.
            debug_assert!(
                self.char_indices.next().is_none() && self.bidi_levels.is_empty(),
                "`char_info`, `bidi_levels`, and `char_indices` should now all be empty \
                (though note `bidi_levels` may already have been empty as a special-case)"
            );
            return None;
        }

        let mut item_bidi_level = 0; // Initialized in the loop.

        let start_byte_offset = self.char_indices.offset();
        let mut item_char_len = 0;
        loop {
            let byte_offset = self.char_indices.offset();

            let bidi_level = if self.bidi_levels.is_empty() {
                self.paragraph_bidi_level
            } else {
                self.bidi_levels[0]
            };
            let mut script = self.char_info[0].script;

            if !real_script(script) {
                // This is a very simple heuristic, where if a character does not have a "real
                // script," it inherits the script of the preceding character. UAX 24 paragraph
                // 5.1 says this "works well in many cases", but also suggests performing, e.g.,
                // bracket matching (for example, the parentheses in `hello (αβγ)` should ideally
                // both be marked as being `Latin`). At that point, `Itemizer` would probably like
                // to have reusable scratch for the bracket stack.
                script = self.current_script;
            }

            // First iteration of the loop, initialize item properties.
            if item_char_len == 0 {
                item_bidi_level = bidi_level;
                self.current_script = script;
            }

            if bidi_level != item_bidi_level || script != self.current_script {
                break;
            }

            if item_char_len > 0
                && (self.split_after)(TextRange {
                    byte_range: start_byte_offset..byte_offset,
                    char_range: self.current_char_offset..self.current_char_offset + item_char_len,
                })
            {
                break;
            }

            self.char_indices.next().expect("The passed in `text` was not of the same length as the text used to generate `Analysis`");
            self.char_info = &self.char_info[1..];
            if !self.bidi_levels.is_empty() {
                self.bidi_levels = &self.bidi_levels[1..];
            }

            item_char_len += 1;

            if self.char_info.is_empty() {
                // The text is now empty, so we're finished.
                break;
            }
        }

        let start_char_offset = self.current_char_offset;
        self.current_char_offset += item_char_len;

        Some(Item {
            range: TextRange {
                byte_range: start_byte_offset..self.char_indices.offset(),
                char_range: start_char_offset..self.current_char_offset,
            },
            script: icu_script_to_parlance_script(self.current_script),
            bidi_level: item_bidi_level,
        })
    }
}

fn real_script(script: IcuScript) -> bool {
    script != IcuScript::Common && script != IcuScript::Unknown && script != IcuScript::Inherited
}

/// Convert an ICU script into a [`Script`].
#[inline]
fn icu_script_to_parlance_script(script: IcuScript) -> Script {
    static SHORT_NAMES: icu_properties::PropertyNamesShortBorrowed<'static, IcuScript> =
        icu_properties::PropertyNamesShort::new();

    SHORT_NAMES
        .get(script)
        .and_then(|name| Script::parse(name).ok())
        .unwrap_or(Script::UNKNOWN)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use parlance::Script;

    use crate::{Analysis, AnalysisOptions, Analyzer};

    use super::Item;

    const LATN: Script = Script::from_str_unchecked("Latn");
    const GREK: Script = Script::from_str_unchecked("Grek");
    const ARAB: Script = Script::from_str_unchecked("Arab");

    fn analyze(text: &str) -> Analysis {
        let mut analyzer = Analyzer::new();
        let mut analysis = Analysis::new();
        let options = AnalysisOptions {
            word_break: &[],
            line_break_override: None,
        };
        analyzer.analyze(text, &options, &mut analysis);
        analysis
    }

    fn items(text: &str) -> Vec<Item> {
        analyze(text).itemize(text, |_| false).collect()
    }

    #[test]
    fn empty() {
        assert!(items("").is_empty());
    }

    #[test]
    fn mixed_direction() {
        let text = "hello مرحبا";
        let items = items(text);
        assert!(items.len() >= 2);
        assert_eq!(items[0].script, LATN);
        assert!(items[0].bidi_level.is_multiple_of(2));
        assert!(
            items
                .iter()
                .any(|item| item.script == ARAB && !item.bidi_level.is_multiple_of(2))
        );

        // Items tile the text contiguously.
        let mut cursor = 0;
        for item in &items {
            assert_eq!(item.range.byte_range.start, cursor);
            cursor = item.range.byte_range.end;
        }
        assert_eq!(cursor, text.len());
    }

    #[test]
    fn predicate() {
        let text = "abcdef";
        let analysis = analyze(text);
        let items: Vec<_> = analysis
            .itemize(text, |range| range.char_range.end == 3)
            .collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].range.byte_range, 0..3);
        assert_eq!(items[0].range.char_range, 0..3);
        assert_eq!(items[1].range.byte_range, 3..6);
        assert_eq!(items[1].range.char_range, 3..6);
    }

    #[test]
    fn neutral_backward() {
        // Latin, a space (`Common`), then Greek.
        let text = "abc αβγ";
        let items = items(text);
        assert_eq!(items.len(), 2);
        // The space attaches to the Latin run.
        assert_eq!(&text[items[0].range.byte_range.clone()], "abc ");
        assert_eq!(items[0].script, LATN);
        assert_eq!(&text[items[1].range.byte_range.clone()], "αβγ");
        assert_eq!(items[1].script, GREK);
    }
}
