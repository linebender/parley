// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::analysis::AnalysisDataSources;

use icu_properties::props::Script;

pub(crate) fn script_to_fontique(
    script: Script,
    analysis_data_sources: &AnalysisDataSources,
) -> fontique::Script {
    let short_name: [u8; 4] = analysis_data_sources
        .script_short_name()
        .get_locale_script(script)
        .unwrap_or(icu_locale_core::subtags::script!("Zzzz"))
        .into_raw();
    fontique::Script(short_name)
}

pub(crate) fn script_to_harfrust(script: fontique::Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&script.0))
        .unwrap_or(harfrust::script::UNKNOWN)
}
