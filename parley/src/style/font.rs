// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use core::fmt;

pub use styled_text::{FontFamily, GenericFamily, Stretch as FontStretch, Style as FontStyle, Weight as FontWeight};

/// Setting for a font variation.
pub type FontVariation = swash::Setting<f32>;

/// Setting for a font feature.
pub type FontFeature = swash::Setting<u16>;

/// Prioritized sequence of font families.
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/font-family>
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FontStack<'a> {
    /// Font family list in CSS format.
    Source(Cow<'a, str>),
    /// Single font family.
    Single(FontFamily<'a>),
    /// Ordered list of font families.
    List(Cow<'a, [FontFamily<'a>]>),
}

impl From<GenericFamily> for FontStack<'_> {
    fn from(f: GenericFamily) -> Self {
        FontStack::Single(f.into())
    }
}

impl<'a> From<FontFamily<'a>> for FontStack<'a> {
    fn from(f: FontFamily<'a>) -> Self {
        FontStack::Single(f)
    }
}

impl<'a> From<&'a str> for FontStack<'a> {
    fn from(s: &'a str) -> Self {
        FontStack::Source(Cow::Borrowed(s))
    }
}

impl<'a> From<&'a [FontFamily<'a>]> for FontStack<'a> {
    fn from(fs: &'a [FontFamily<'a>]) -> Self {
        FontStack::List(Cow::Borrowed(fs))
    }
}

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
