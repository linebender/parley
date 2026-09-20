// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Brush, LayoutContext, WhiteSpaceCollapse};

use parley_engine::break_overrides::LineBreakOverrideFn;

use parley_engine::AnalysisOptions;

use parlance::{BaseDirection, WordBreak};

pub(crate) fn analyze_text<B: Brush>(
    lcx: &mut LayoutContext<B>,
    text: &str,
    base_direction: BaseDirection,
    line_break_override: Option<&LineBreakOverrideFn>,
) {
    let text = if text.is_empty() { " " } else { text };

    // Collect the style runs' word breaks. Gaps are `WordBreak::Normal`, so only non-`Normal`s need
    // an entry.
    lcx.word_break.clear();
    lcx.word_break
        .extend(lcx.style_runs.iter().filter_map(|sr| {
            let word_break = lcx.style_table[sr.style_index as usize].word_break;
            (word_break != WordBreak::Normal).then(|| (sr.range.clone(), word_break))
        }));

    // Collect the style runs which allow wrapping after each preserved space or tab. Adjacent
    // runs are merged, so that an opportunity is not created before the first space of a sequence
    // spanning a style boundary.
    lcx.break_spaces.clear();
    for style_run in lcx.style_runs.iter().filter(|sr| {
        lcx.style_table[sr.style_index as usize].white_space_collapse
            == WhiteSpaceCollapse::BreakSpaces
    }) {
        match lcx.break_spaces.last_mut() {
            Some(last) if last.end == style_run.range.start => last.end = style_run.range.end,
            _ => lcx.break_spaces.push(style_run.range.clone()),
        }
    }

    let options = AnalysisOptions {
        base_direction,
        word_break: &lcx.word_break,
        break_spaces: &lcx.break_spaces,
        line_break_override,
    };
    lcx.analyzer.analyze(text, &options, &mut lcx.analysis);
}
