// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;
use core::fmt;

use crate::GenericFamily;

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
    /// use styled_text::FontFamily::{self, *};
    /// use styled_text::GenericFamily::*;
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
    /// use styled_text::FontFamily::{self, *};
    /// use styled_text::GenericFamily::*;
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

impl fmt::Display for FontFamily<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Named(name) => write!(f, "{:?}", name),
            Self::Generic(family) => write!(f, "{}", family),
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
