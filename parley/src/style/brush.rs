// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Trait for types that represent the color of glyphs or decorations.
pub trait Brush: Clone + PartialEq + Default + core::fmt::Debug {
    #[cfg(feature = "accesskit")]
    /// Converts the brush to an AccessKit color.
    fn to_accesskit_color(&self) -> Option<accesskit::Color> {
        None
    }
}

impl Brush for () {}

impl Brush for [u8; 4] {}

impl Brush for u32 {}

#[cfg(feature = "peniko")]
impl Brush for peniko::Brush {
    #[cfg(feature = "accesskit")]
    fn to_accesskit_color(&self) -> Option<accesskit::Color> {
        if let Self::Solid(color) = self {
            let rgba = color.to_rgba8();
            Some(accesskit::Color {
                red: rgba.r,
                green: rgba.g,
                blue: rgba.b,
                alpha: rgba.a,
            })
        } else {
            None
        }
    }
}
