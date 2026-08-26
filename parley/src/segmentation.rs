// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text segmentation using [`icu_segmenter`] for word and line break boundaries.

use alloc::vec::Vec;
use core::ops::Range;

use icu_segmenter::options::{LineBreakOptions, LineBreakWordOption, WordBreakInvariantOptions};
use icu_segmenter::{LineSegmenter, LineSegmenterBorrowed, WordSegmenter};
use parlance::WordBreak;

use crate::{LineBreakContext, LineBreakOverrideFn};

/// Turns the sparse, sorted, non-overlapping `options.word_break` into a contiguous sequence of
/// `(range, word-break)` segments covering all of `text`.
///
/// Any region not covered by an override takes the default `WordBreak::Normal`.
struct DenseWordBreaks<'a> {
    word_break: &'a [(Range<usize>, WordBreak)],
    next: usize,
    cursor: usize,
    text_len: usize,
}

impl<'a> DenseWordBreaks<'a> {
    fn new(word_break: &'a [(Range<usize>, WordBreak)], text_len: usize) -> Self {
        Self {
            word_break,
            next: 0,
            cursor: 0,
            text_len,
        }
    }
}

impl Iterator for DenseWordBreaks<'_> {
    type Item = (Range<usize>, WordBreak);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.text_len {
            return None;
        }
        while self
            .word_break
            .get(self.next)
            .is_some_and(|(range, _)| range.is_empty())
        {
            self.next += 1;
        }
        match self.word_break.get(self.next) {
            Some((range, _)) if self.cursor < range.start => {
                let segment = self.cursor..range.start;
                self.cursor = range.start;
                Some((segment, WordBreak::Normal))
            }
            Some((range, word_break)) => {
                self.cursor = range.end;
                self.next += 1;
                Some((range.clone(), *word_break))
            }
            None => {
                let segment = self.cursor..self.text_len;
                self.cursor = self.text_len;
                Some((segment, WordBreak::Normal))
            }
        }
    }
}

/// Produces overlapping substrings for contiguous runs of one word-break style.
struct WordBreakSegmentIter<'a, I: Iterator> {
    text: &'a str,
    segments: I,
    char_indices: core::str::CharIndices<'a>,
    current_char: (usize, char),
    building_range_start: usize,
    previous_word_break_style: WordBreak,
    done: bool,
}

impl<'a, I> WordBreakSegmentIter<'a, I>
where
    I: Iterator<Item = (Range<usize>, WordBreak)>,
{
    fn new(text: &'a str, segments: I, first_segment: (Range<usize>, WordBreak)) -> Self {
        let mut char_indices = text.char_indices();
        let current_char = char_indices.next().unwrap();
        Self {
            text,
            segments,
            char_indices,
            current_char,
            building_range_start: first_segment.0.start,
            previous_word_break_style: first_segment.1,
            done: false,
        }
    }
}

impl<'a, I> Iterator for WordBreakSegmentIter<'a, I>
where
    I: Iterator<Item = (Range<usize>, WordBreak)>,
{
    type Item = (&'a str, WordBreak, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        for (range, word_break) in self.segments.by_ref() {
            assert!(range.start < range.end, "segments must not be empty");
            let mut previous_char = self.current_char;
            while self.current_char.0 < range.start {
                previous_char = self.current_char;
                self.current_char = self.char_indices.next().unwrap();
            }
            if self.previous_word_break_style == word_break {
                continue;
            }
            let substring =
                &self.text[self.building_range_start..range.start + self.current_char.1.len_utf8()];
            let result_style = self.previous_word_break_style;
            self.building_range_start = range.start - previous_char.1.len_utf8();
            self.previous_word_break_style = word_break;
            return Some((substring, result_style, false));
        }
        self.done = true;
        Some((
            &self.text[self.building_range_start..],
            self.previous_word_break_style,
            true,
        ))
    }
}

#[cfg(feature = "complex-scripts")]
fn line_segmenter(options: LineBreakOptions<'_>) -> LineSegmenterBorrowed<'static> {
    LineSegmenter::new_dictionary(options)
}

#[cfg(not(feature = "complex-scripts"))]
fn line_segmenter(options: LineBreakOptions<'_>) -> LineSegmenterBorrowed<'static> {
    LineSegmenter::new_for_non_complex_scripts(options)
}

fn line_segmenter_for(word_break: WordBreak) -> LineSegmenterBorrowed<'static> {
    let mut options = LineBreakOptions::default();
    options.word_option = Some(match word_break {
        WordBreak::Normal => LineBreakWordOption::Normal,
        WordBreak::BreakAll => LineBreakWordOption::BreakAll,
        WordBreak::KeepAll => LineBreakWordOption::KeepAll,
    });
    line_segmenter(options)
}

/// Computes soft line-break opportunities for `text` into reusable storage.
pub(crate) fn line_break_opportunities(
    text: &str,
    word_break: &[(Range<usize>, WordBreak)],
    line_break_override: Option<&LineBreakOverrideFn>,
    output: &mut Vec<usize>,
) {
    output.clear();
    if text.is_empty() {
        return;
    }

    let mut segments = DenseWordBreaks::new(word_break, text.len());
    let first_segment = segments.next().unwrap();
    let substrings = WordBreakSegmentIter::new(text, segments, first_segment);
    let mut global_offset = 0;

    for (substring_index, (substring, word_break_strength, last)) in substrings.enumerate() {
        if substring_index == 0 && last {
            output.extend(
                line_segmenter_for(word_break_strength)
                    .segment_str(substring)
                    .skip(1),
            );
            output.pop();
            break;
        }

        let mut substring_chars = substring.chars();
        if substring_index != 0 {
            global_offset -= substring_chars.next().unwrap().len_utf8();
        }
        let last_len = substring_chars.next_back().unwrap().len_utf8();
        for (index, position) in line_segmenter_for(word_break_strength)
            .segment_str(substring)
            .enumerate()
        {
            if index == 0 || position == substring.len() {
                continue;
            }
            if !last && position == substring.len() - last_len {
                continue;
            }
            output.push(position + global_offset);
        }
        if !last {
            global_offset += substring.len() - last_len;
        }
    }

    if let Some(line_break_override) = line_break_override {
        let mut previous = None;
        let mut previous_previous = None;
        for (position, character) in text.char_indices() {
            if let Some(before) = previous
                && let Some(forced) = line_break_override(LineBreakContext {
                    before_before: previous_previous,
                    before,
                    after: character,
                })
            {
                match (output.binary_search(&position), forced) {
                    (Err(index), true) => output.insert(index, position),
                    (Ok(index), false) => {
                        output.remove(index);
                    }
                    _ => {}
                }
            }
            previous_previous = previous;
            previous = Some(character);
        }
    }
}

/// Computes UAX-29 word-segment boundaries for `text` into reusable storage.
pub(crate) fn word_boundaries(text: &str, output: &mut Vec<usize>) {
    output.clear();
    #[cfg(feature = "complex-scripts")]
    let segmenter = WordSegmenter::new_dictionary(WordBreakInvariantOptions::default());
    #[cfg(not(feature = "complex-scripts"))]
    let segmenter =
        const { WordSegmenter::new_for_non_complex_scripts(WordBreakInvariantOptions::default()) };
    output.extend(segmenter.segment_str(text));
}
