// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec::Vec;

use text_style::Setting;

pub use text_primitives::ParseSettingsError;

/// Parses a CSS-like `font-variation-settings` value into a list of settings.
pub(crate) fn parse_variation_settings(
    source: &Arc<str>,
) -> Result<Vec<Setting<f32>>, ParseSettingsError> {
    Setting::<f32>::parse_list(source.as_ref()).collect()
}

/// Parses a CSS-like `font-feature-settings` value into a list of settings.
pub(crate) fn parse_feature_settings(
    source: &Arc<str>,
) -> Result<Vec<Setting<u16>>, ParseSettingsError> {
    Setting::<u16>::parse_list(source.as_ref()).collect()
}
