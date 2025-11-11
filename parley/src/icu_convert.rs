// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::analysis::AnalysisDataSources;

use icu_locale_core::LanguageIdentifier;
use icu_properties::props::Script;

pub(crate) fn script_to_fontique(
    script: Script,
    analysis_data_sources: &AnalysisDataSources,
) -> fontique::Script {
    let short_name: [u8; 4] = analysis_data_sources
        .script_short_name()
        .get(script)
        .unwrap_or("Zzzz")
        .as_bytes()
        .try_into()
        .expect("exactly 4 bytes");
    fontique::Script(short_name)
}

pub(crate) fn locale_to_fontique(locale: LanguageIdentifier) -> Option<fontique::Language> {
    fontique::Language::try_from_utf8(locale.to_string().as_bytes()).ok()
}

pub(crate) fn script_to_harfrust(script: fontique::Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&script.0))
        .unwrap_or(harfrust::script::UNKNOWN)
}
