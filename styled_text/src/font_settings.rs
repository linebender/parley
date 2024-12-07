// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use core::fmt;

/// Setting for a font variation.
// FIXME(style): We should copy the Setting definition from swash instead of having a dep
pub type FontVariation = swash::Setting<f32>;

/// Setting for a font feature.
// FIXME(style): We should copy the Setting definition from swash instead of having a dep
pub type FontFeature = swash::Setting<u16>;

/// Font settings that can be supplied as a raw source string or
/// a parsed slice.
#[derive(Clone, PartialEq, Debug)]
pub enum FontSettings<'a, T>
where
    [T]: ToOwned,
    <[T] as ToOwned>::Owned: fmt::Debug + PartialEq + Clone,
{
    /// Setting source in CSS format.
    Source(Cow<'a, str>),
    /// List of settings.
    List(Cow<'a, [T]>),
}

impl<'a, T> From<&'a str> for FontSettings<'a, T>
where
    [T]: ToOwned,
    <[T] as ToOwned>::Owned: fmt::Debug + PartialEq + Clone,
{
    fn from(value: &'a str) -> Self {
        Self::Source(Cow::Borrowed(value))
    }
}

impl<'a, T> From<&'a [T]> for FontSettings<'a, T>
where
    [T]: ToOwned,
    <[T] as ToOwned>::Owned: fmt::Debug + PartialEq + Clone,
{
    fn from(value: &'a [T]) -> Self {
        Self::List(Cow::Borrowed(value))
    }
}
