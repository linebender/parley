// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A specified value in a style, using CSS-like reset semantics.
///
/// See the module docs for how `inherit` and `initial` interact with resolution: [`super`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Specified<T> {
    /// Use the property's value from the parent style.
    Inherit,
    /// Reset the property to the initial value.
    Initial,
    /// Provide an explicit value.
    Value(T),
}

impl<T> From<T> for Specified<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}
