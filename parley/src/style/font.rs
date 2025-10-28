// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use core::fmt;

use crate::setting::Setting;

pub use fontique::{FontStyle, FontWeight, FontWidth, GenericFamily};

/// Setting for a font variation.
pub type FontVariation = Setting<f32>;

/// Setting for a font feature.
pub type FontFeature = Setting<u16>;

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

/// Named or generic font family.
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/font-family>
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FontFamily<'a> {
    /// Named font family.
    Named(Cow<'a, str>),
    /// Generic font family.
    Generic(GenericFamily),
}

impl<'a> FontFamily<'a> {
    /// Parses a font family containing a name or a generic family.
    ///
    /// # Example
    /// ```
    /// # extern crate alloc;
    /// use alloc::borrow::Cow;
    /// use parley::style::FontFamily::{self, *};
    /// use parley::style::GenericFamily::*;
    ///
    /// assert_eq!(FontFamily::parse("Palatino Linotype"), Some(Named(Cow::Borrowed("Palatino Linotype"))));
    /// assert_eq!(FontFamily::parse("monospace"), Some(Generic(Monospace)));
    ///
    /// // Note that you can quote a generic family to capture it as a named family:
    ///
    /// assert_eq!(FontFamily::parse("'monospace'"), Some(Named(Cow::Borrowed("monospace"))));
    /// ```
    pub fn parse(s: &'a str) -> Option<Self> {
        Self::parse_list(s).next()
    }

    /// Parses a comma separated list of font families.
    ///
    /// # Example
    /// ```
    /// # extern crate alloc;
    /// use alloc::borrow::Cow;
    /// use parley::style::FontFamily::{self, *};
    /// use parley::style::GenericFamily::*;
    ///
    /// let source = "Arial, 'Times New Roman', serif";
    ///
    /// let parsed_families = FontFamily::parse_list(source).collect::<Vec<_>>();
    /// let families = vec![Named(Cow::Borrowed("Arial")), Named(Cow::Borrowed("Times New Roman")), Generic(Serif)];
    ///
    /// assert_eq!(parsed_families, families);
    /// ```
    pub fn parse_list(s: &'a str) -> impl Iterator<Item = FontFamily<'a>> + 'a + Clone {
        ParseList {
            source: s.as_bytes(),
            len: s.len(),
            pos: 0,
        }
    }
}

impl From<GenericFamily> for FontFamily<'_> {
    fn from(f: GenericFamily) -> Self {
        FontFamily::Generic(f)
    }
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

impl fmt::Display for FontFamily<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(name) => write!(f, "{name:?}"),
            Self::Generic(family) => write!(f, "{family}"),
        }
    }
}

#[derive(Clone)]
struct ParseList<'a> {
    source: &'a [u8],
    len: usize,
    pos: usize,
}

impl<'a> Iterator for ParseList<'a> {
    type Item = FontFamily<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut quote = None;
        let mut pos = self.pos;
        while pos < self.len && {
            let ch = self.source[pos];
            ch.is_ascii_whitespace() || ch == b','
        } {
            pos += 1;
        }
        self.pos = pos;
        if pos >= self.len {
            return None;
        }
        let first = self.source[pos];
        let mut start = pos;
        match first {
            b'"' | b'\'' => {
                quote = Some(first);
                pos += 1;
                start += 1;
            }
            _ => {}
        }
        if let Some(quote) = quote {
            while pos < self.len {
                if self.source[pos] == quote {
                    self.pos = pos + 1;
                    return Some(FontFamily::Named(Cow::Borrowed(
                        core::str::from_utf8(self.source.get(start..pos)?)
                            .ok()?
                            .trim(),
                    )));
                }
                pos += 1;
            }
            self.pos = pos;
            return Some(FontFamily::Named(Cow::Borrowed(
                core::str::from_utf8(self.source.get(start..pos)?)
                    .ok()?
                    .trim(),
            )));
        }
        let mut end = start;
        while pos < self.len {
            if self.source[pos] == b',' {
                pos += 1;
                break;
            }
            pos += 1;
            end += 1;
        }
        self.pos = pos;
        let name = core::str::from_utf8(self.source.get(start..end)?)
            .ok()?
            .trim();
        Some(match GenericFamily::parse(name) {
            Some(family) => FontFamily::Generic(family),
            _ => FontFamily::Named(Cow::Borrowed(name)),
        })
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
