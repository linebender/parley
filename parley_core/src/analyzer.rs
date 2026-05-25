// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The analysis API.

use alloc::vec::Vec;
use core::ops::Range;

use parlance::{BaseDirection, WordBreak};

use crate::analysis::{CharInfo, analyze_text};
use crate::bidi::BidiResolver;

/// Reusable scratch for [`Analyzer::analyze`].
#[derive(Default)]
pub struct Analyzer {
    pub(crate) bidi: BidiResolver,
}

impl core::fmt::Debug for Analyzer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Analyzer").finish_non_exhaustive()
    }
}

impl Analyzer {
    /// Creates a new analyzer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze `text`, appending the result to `out`.
    pub fn analyze(&mut self, text: &str, options: &AnalysisOptions<'_>, out: &mut Analysis) {
        analyze_text(self, text, options, out);
    }
}

/// Options controlling [`Analyzer::analyze`].
#[derive(Clone, Copy, Debug)]
pub struct AnalysisOptions<'a> {
    /// The paragraph's base direction.
    ///
    /// [`BaseDirection::Auto`] applies UAX #9's "first-strong" heuristic.
    pub base_direction: BaseDirection,
    /// Per-range word break configuration.
    ///
    /// Ranges must be sorted and non-overlapping. Gaps use [`WordBreak::Normal`].
    pub line_break_overrides: &'a [(Range<usize>, WordBreak)],
}

impl Default for AnalysisOptions<'_> {
    fn default() -> Self {
        Self {
            base_direction: BaseDirection::Auto,
            line_break_overrides: &[],
        }
    }
}

/// The result of [`Analyzer::analyze`].
///
/// Indexed by character (one [`CharInfo`] per `char` of the source text).
#[derive(Debug, Default)]
pub struct Analysis {
    pub(crate) infos: Vec<CharInfo>,
    /// Resolved bidi levels, parallel to `infos`. Empty if the text is all LTR.
    pub(crate) levels: Vec<u8>,
    pub(crate) paragraph_level: u8,
}

impl Analysis {
    /// Creates an empty analysis result.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears the result while retaining capacity.
    pub fn clear(&mut self) {
        self.infos.clear();
        self.levels.clear();
        self.paragraph_level = 0;
    }

    /// Returns per-character info, in source order.
    pub fn char_infos(&self) -> &[CharInfo] {
        &self.infos
    }

    /// Returns resolved bidi levels, parallel to [`Self::char_infos`].
    ///
    /// Empty when the whole paragraph is left-to-right.
    pub fn bidi_levels(&self) -> &[u8] {
        &self.levels
    }

    /// Returns the paragraph's resolved base bidi level.
    pub fn paragraph_level(&self) -> u8 {
        self.paragraph_level
    }
}
