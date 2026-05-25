// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text analysis.

use alloc::vec::Vec;
use core::ops::Range;

use crate::{Brush, LayoutContext, WordBreak};

use parlance::BaseDirection;
use parley_core::AnalysisOptions;

pub(crate) use parley_core::{Boundary, CharInfo};

/// Analyzes `text` into [`LayoutContext::analysis`], translating the layout's word-break
/// configuration into [`parley_core`] line-break overrides.
///
/// Empty text is analyzed as a single space so that a cursor can be sized from the resulting
/// metrics.
pub(crate) fn analyze_text<B: Brush>(lcx: &mut LayoutContext<B>, text: &str) {
    let text = if text.is_empty() { " " } else { text };

    // Translate each style run's word-break into a line-break override. The analyzer treats gaps
    // as `Normal`, so only non-default strengths need an entry.
    lcx.line_break_overrides.clear();
    for style_run in &lcx.style_runs {
        let word_break = lcx.style_table[style_run.style_index as usize].word_break;
        if word_break != WordBreak::Normal {
            lcx.line_break_overrides
                .push((style_run.range.clone(), word_break));
        }
    }

    let options = AnalysisOptions {
        base_direction: BaseDirection::Auto,
        line_break_overrides: &lcx.line_break_overrides,
    };
    lcx.analyzer.analyze(text, &options, &mut lcx.analysis);
}

/// The per-style line-break overrides handed to [`parley_core::Analyzer`].
pub(crate) type LineBreakOverrides = Vec<(Range<usize>, WordBreak)>;
