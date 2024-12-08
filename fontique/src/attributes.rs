// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Properties for specifying font weight, stretch and style.

#[cfg(feature = "libm")]
#[allow(unused_imports)]
use core_maths::CoreFloat;

use core::fmt;
use styled_text::{Stretch, Style, Weight};

/// Primary attributes for font matching: [`Stretch`], [`Style`] and [`Weight`].
///
/// These are used to [configure] a [`Query`].
///
/// [configure]: crate::Query::set_attributes
/// [`Query`]: crate::Query
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Attributes {
    pub stretch: Stretch,
    pub style: Style,
    pub weight: Weight,
}

impl Attributes {
    /// Creates new attributes from the given stretch, style and weight.
    pub fn new(stretch: Stretch, style: Style, weight: Weight) -> Self {
        Self {
            stretch,
            style,
            weight,
        }
    }
}

impl fmt::Display for Attributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stretch: {}, style: {}, weight: {}",
            self.stretch, self.style, self.weight
        )
    }
}
