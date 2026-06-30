// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use icu_properties::props::Script;
use parley_core::AnalysisDataSources;

pub(crate) fn script_to_fontique(
    script: Script,
    analysis_data_sources: &AnalysisDataSources,
) -> fontique::Script {
    analysis_data_sources
        .script_short_name()
        .get(script)
        .unwrap_or("Zzzz")
        .parse()
        .unwrap_or(fontique::Script::UNKNOWN)
}

pub(crate) fn script_to_harfrust(script: fontique::Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&script.to_bytes()))
        .unwrap_or(harfrust::script::UNKNOWN)
}
