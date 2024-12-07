// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;

use crate::{FontFamily, GenericFamily};

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
