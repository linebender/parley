// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The analyzer API.

use core::ops::Range;

use alloc::vec::Vec;
use icu_properties::props::{BidiClass, BidiMirroringGlyph};
use parlance::WordBreak;

use crate::{bidi::BidiResolver, break_overrides::LineBreakOverrideFn};

use crate::analysis::{Analysis, analyze_text};

/// Reusable scratch for [`Analyzer::analyze`].
#[derive(Default)]
pub struct Analyzer {
    pub(crate) bidi: BidiResolver,
    pub(crate) bidi_props: Vec<(BidiClass, BidiMirroringGlyph)>,
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

    /// Analyze `text`, overwriting `analysis`.
    ///
    /// This reuses the allocations of `analysis`.
    pub fn analyze(&mut self, text: &str, options: &AnalysisOptions<'_>, analysis: &mut Analysis) {
        analysis.clear();
        analyze_text(self, text, options, analysis);
    }
}

/// Options controlling [`Analyzer::analyze`].
#[derive(Clone, Copy)]
pub struct AnalysisOptions<'a> {
    /// Word break configuration for ranges of the source text.
    ///
    /// Ranges must be sorted and non-overlapping. Gaps use [`WordBreak::Normal`].
    pub word_break: &'a [(Range<usize>, WordBreak)],

    /// The callback which will be called as a first provider of line breaking decisions.
    ///
    /// See [`LineBreakOverrideFn`] for more details.
    pub line_break_override: Option<&'a LineBreakOverrideFn>,
}

impl core::fmt::Debug for AnalysisOptions<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnalysisOptions").finish_non_exhaustive()
    }
}
