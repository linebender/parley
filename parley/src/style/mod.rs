// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rich styling support.

mod styleset;

pub use styled_text::{
    Brush, FontFamily, FontSettings, FontStack, GenericFamily, Stretch as FontStretch,
    Style as FontStyle, StyleProperty, TextStyle, Weight as FontWeight,
};

pub use styleset::StyleSet;

/// Setting for a font variation.
pub type FontVariation = swash::Setting<f32>;

/// Setting for a font feature.
pub type FontFeature = swash::Setting<u16>;

#[derive(Debug, Clone, Copy)]
pub enum WhiteSpaceCollapse {
    Collapse,
    Preserve,
}
