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

    // Collect the style runs' word breaks, and the runs which allow wrapping after each preserved
    // space or tab. Word break gaps are `WordBreak::Normal`, so only non-`Normal`s need an entry.
    // Adjacent `break-spaces` runs are merged, so that an opportunity is not created before the
    // first space of a sequence spanning a style boundary.
    lcx.word_break.clear();
    lcx.break_spaces.clear();
    for style_run in lcx.style_runs.iter() {
        let style = &lcx.style_table[style_run.style_index as usize];
        if style.word_break != WordBreak::Normal {
            lcx.word_break
                .push((style_run.range.clone(), style.word_break));
        }
        if style.white_space_collapse == WhiteSpaceCollapse::BreakSpaces {
            match lcx.break_spaces.last_mut() {
                Some(last) if last.end == style_run.range.start => last.end = style_run.range.end,
                _ => lcx.break_spaces.push(style_run.range.clone()),
            }
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
