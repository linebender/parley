// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub(crate) mod cluster;

use crate::{Brush, LayoutContext};

use icu_properties::props::{GeneralCategory, Script};
use parley_core::break_overrides::LineBreakOverrideFn;

use parley_core::{AnalysisDataSources, AnalysisOptions};

pub(crate) fn analyze_text<B: Brush>(
    lcx: &mut LayoutContext<B>,
    text: &str,
    line_break_override: Option<&LineBreakOverrideFn>,
) {
    let text = if text.is_empty() { " " } else { text };

    // Collect the style runs word breaks. Gaps are `WordBreak::Normal`, so only non-`Normal`s need
    // an entry.
    lcx.word_break.clear();
    lcx.word_break.extend(lcx.style_runs.iter().map(|sr| {
        (
            sr.range.clone(),
            lcx.style_table[sr.style_index as usize].word_break,
        )
    }));

    let options = AnalysisOptions {
        word_break: &lcx.word_break,
        line_break_override,
    };
    lcx.analyzer.analyze(text, &options, &mut lcx.analysis);

    // Pair the char infos with style indices (which we set later in the builder).
    lcx.info.clear();
    lcx.info
        .extend(lcx.analysis.char_info().iter().map(|&ci| (ci, 0)));
}

/// All characters contribute to shaping except:
/// - Control characters
/// - Format characters, unless they use the "Inherited" script
// TODO: this function is duplicated at the time of writing, as it also exists in `parley_core`.
// Once more of `parley` is moved to `parley_core`, this function should be removable.
#[inline(always)]
pub(crate) fn contributes_to_shaping(general_category: GeneralCategory, script: Script) -> bool {
    if matches!(
        general_category,
        GeneralCategory::Control
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    ) {
        return false;
    }

    !(general_category == GeneralCategory::Format && script != Script::Inherited)
}
