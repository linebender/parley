// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub use styled_text::{
    FontFamily, FontSettings, FontStack, GenericFamily, Stretch as FontStretch, Style as FontStyle,
    Weight as FontWeight,
};

/// Setting for a font variation.
pub type FontVariation = swash::Setting<f32>;

/// Setting for a font feature.
pub type FontFeature = swash::Setting<u16>;
