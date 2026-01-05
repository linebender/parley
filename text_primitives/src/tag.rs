// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// A 4-byte OpenType tag (for example `wght`, `liga`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct Tag([u8; 4]);

impl Tag {
    /// Creates a tag from a 4-byte array reference.
    pub const fn new(bytes: &[u8; 4]) -> Self {
        Self::from_bytes(*bytes)
    }

    /// Creates a tag from 4 bytes.
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Returns this tag as 4 bytes.
    pub const fn to_bytes(self) -> [u8; 4] {
        self.0
    }

    /// Parses a tag from a 4-character ASCII string.
    pub fn parse(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 4 {
            return None;
        }
        if !bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
            return None;
        }
        Some(Self::from_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.to_bytes();
        let s = core::str::from_utf8(&bytes).unwrap_or("????");
        f.write_str(s)
    }
}

/// Kinds of errors that can occur when parsing OpenType settings source strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseSettingsErrorKind {
    /// The source string does not conform to the supported syntax.
    InvalidSyntax,
    /// A quoted tag was invalid.
    InvalidTag,
    /// A numeric value was out of range for the target type.
    OutOfRange,
}

/// Error returned when parsing OpenType settings source strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseSettingsError {
    kind: ParseSettingsErrorKind,
    at: usize,
    span: Option<(usize, usize)>,
}

impl ParseSettingsError {
    fn new(kind: ParseSettingsErrorKind, at: usize) -> Self {
        Self {
            kind,
            at,
            span: None,
        }
    }

    fn with_span(mut self, span: (usize, usize)) -> Self {
        self.span = Some(span);
        self
    }

    /// Returns the error kind.
    pub const fn kind(self) -> ParseSettingsErrorKind {
        self.kind
    }

    /// Returns the byte offset into the source where the error was detected.
    pub const fn byte_offset(self) -> usize {
        self.at
    }

    /// Returns the byte span (start, end) for the token associated with this error, if available.
    pub const fn byte_span(self) -> Option<(usize, usize)> {
        self.span
    }
}

impl fmt::Display for ParseSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self.kind {
            ParseSettingsErrorKind::InvalidSyntax => "invalid settings syntax",
            ParseSettingsErrorKind::InvalidTag => "invalid OpenType tag",
            ParseSettingsErrorKind::OutOfRange => "value out of range",
        };
        write!(f, "{msg} at byte {}", self.at)
    }
}

impl core::error::Error for ParseSettingsError {}

/// A single OpenType setting (tag + value).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Setting<T> {
    /// The OpenType tag for this setting.
    pub tag: Tag,
    /// The setting value.
    pub value: T,
}

impl<T> Setting<T> {
    /// Creates a new setting.
    pub const fn new(tag: Tag, value: T) -> Self {
        Self { tag, value }
    }
}

impl Setting<u16> {
    /// Parses a comma-separated list of feature settings according to the CSS grammar.
    ///
    /// On success, yields a sequence of settings. On failure, yields a [`ParseSettingsError`].
    ///
    /// Supported syntax is a comma-separated list of entries:
    /// - tags are required and must be quoted: `"liga" on` or `'liga' on`
    /// - values are optional:
    ///   - `on`/omitted => `1`
    ///   - `off` => `0`
    ///   - a numeric value is parsed as `u16`
    ///
    /// Whitespace is ignored and a trailing comma is permitted.
    pub fn parse_css_list(
        s: &str,
    ) -> impl Iterator<Item = Result<Self, ParseSettingsError>> + '_ + Clone {
        ParseCssList::new(s).map(|parsed| {
            let (tag, value_str, value_at) = parsed?;
            let span = (value_at, value_at + value_str.len());
            let value = match value_str {
                "" | "on" => 1,
                "off" => 0,
                _ => value_str.parse::<u16>().map_err(|_| {
                    ParseSettingsError::new(ParseSettingsErrorKind::OutOfRange, value_at)
                        .with_span(span)
                })?,
            };
            Ok(Self { tag, value })
        })
    }
}

impl Setting<f32> {
    /// Parses a comma-separated list of variation settings according to the CSS grammar.
    ///
    /// On success, yields a sequence of settings. On failure, yields a [`ParseSettingsError`].
    ///
    /// Supported syntax is a comma-separated list of entries:
    /// - tags are required and must be quoted: `"wght" 700` or `'wght' 700`
    /// - values are required and are parsed as `f32`
    ///
    /// Whitespace is ignored and a trailing comma is permitted.
    pub fn parse_css_list(
        s: &str,
    ) -> impl Iterator<Item = Result<Self, ParseSettingsError>> + '_ + Clone {
        ParseCssList::new(s).map(|parsed| {
            let (tag, value_str, value_at) = parsed?;
            let span = (value_at, value_at + value_str.len());
            if value_str.is_empty() {
                return Err(ParseSettingsError::new(
                    ParseSettingsErrorKind::InvalidSyntax,
                    value_at,
                ));
            }
            let value = value_str.parse::<f32>().map_err(|_| {
                ParseSettingsError::new(ParseSettingsErrorKind::InvalidSyntax, value_at)
                    .with_span(span)
            })?;
            Ok(Self { tag, value })
        })
    }
}

fn trim_ascii_whitespace(bytes: &[u8], mut start: usize, mut end: usize) -> (usize, usize) {
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    (start, end)
}

#[derive(Clone)]
struct ParseCssList<'a> {
    source: &'a [u8],
    len: usize,
    pos: usize,
    done: bool,
}

impl<'a> ParseCssList<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            len: source.len(),
            pos: 0,
            done: false,
        }
    }
}

impl<'a> Iterator for ParseCssList<'a> {
    type Item = Result<(Tag, &'a str, usize), ParseSettingsError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut pos = self.pos;
        while pos < self.len {
            let ch = self.source[pos];
            if ch.is_ascii_whitespace() || ch == b',' {
                pos += 1;
            } else {
                break;
            }
        }
        self.pos = pos;
        if pos >= self.len {
            self.done = true;
            return None;
        }

        let first = self.source[pos];
        let mut start = pos;
        let quote = match first {
            b'"' | b'\'' => {
                pos += 1;
                start += 1;
                first
            }
            _ => {
                self.done = true;
                return Some(Err(ParseSettingsError::new(
                    ParseSettingsErrorKind::InvalidSyntax,
                    pos,
                )));
            }
        };

        let mut tag_str = None;
        while pos < self.len {
            if self.source[pos] == quote {
                tag_str = Some(pos);
                pos += 1;
                break;
            }
            pos += 1;
        }
        if tag_str.is_none() {
            self.done = true;
            return Some(Err(ParseSettingsError::new(
                ParseSettingsErrorKind::InvalidSyntax,
                start.saturating_sub(1),
            )));
        }
        self.pos = pos;

        let end = tag_str.unwrap();
        let tag_bytes = match self.source.get(start..end) {
            Some(bytes) => bytes,
            None => {
                self.done = true;
                return Some(Err(ParseSettingsError::new(
                    ParseSettingsErrorKind::InvalidSyntax,
                    start,
                )));
            }
        };
        let tag_str = match core::str::from_utf8(tag_bytes) {
            Ok(s) => s,
            Err(_) => {
                self.done = true;
                return Some(Err(ParseSettingsError::new(
                    ParseSettingsErrorKind::InvalidSyntax,
                    start,
                )));
            }
        };
        let tag = match Tag::parse(tag_str) {
            Some(tag) => tag,
            None => {
                self.done = true;
                return Some(Err(ParseSettingsError::new(
                    ParseSettingsErrorKind::InvalidTag,
                    start,
                )
                .with_span((start, end))));
            }
        };

        while pos < self.len && self.source[pos].is_ascii_whitespace() {
            pos += 1;
        }
        start = pos;
        let mut value_end = start;
        while pos < self.len {
            if self.source[pos] == b',' {
                pos += 1;
                break;
            }
            pos += 1;
            value_end += 1;
        }
        self.pos = pos;

        let (trim_start, trim_end) = trim_ascii_whitespace(self.source, start, value_end);
        let value_slice = match self.source.get(trim_start..trim_end) {
            Some(slice) => slice,
            None => {
                self.done = true;
                return Some(Err(ParseSettingsError::new(
                    ParseSettingsErrorKind::InvalidSyntax,
                    start,
                )));
            }
        };
        let value_str = match core::str::from_utf8(value_slice) {
            Ok(s) => s,
            Err(_) => {
                self.done = true;
                return Some(Err(ParseSettingsError::new(
                    ParseSettingsErrorKind::InvalidSyntax,
                    start,
                )));
            }
        };

        Some(Ok((tag, value_str, trim_start)))
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseSettingsErrorKind, Setting, Tag};
    extern crate alloc;
    use alloc::vec::Vec;

    #[test]
    fn parse_feature_settings_css_list_ok() {
        let parsed: Result<Vec<_>, _> =
            Setting::<u16>::parse_css_list(r#""liga" on, 'kern', "dlig" off, "salt" 3,"#).collect();
        let settings = parsed.unwrap();

        assert_eq!(settings.len(), 4);
        assert_eq!(settings[0].tag, Tag::parse("liga").unwrap());
        assert_eq!(settings[0].value, 1);
        assert_eq!(settings[1].tag, Tag::parse("kern").unwrap());
        assert_eq!(settings[1].value, 1);
        assert_eq!(settings[2].tag, Tag::parse("dlig").unwrap());
        assert_eq!(settings[2].value, 0);
        assert_eq!(settings[3].tag, Tag::parse("salt").unwrap());
        assert_eq!(settings[3].value, 3);
    }

    #[test]
    fn parse_feature_settings_css_list_errors_include_offset_and_span() {
        let err = Setting::<u16>::parse_css_list(r#""liga" 70000"#)
            .next()
            .unwrap()
            .unwrap_err();
        assert_eq!(err.kind(), ParseSettingsErrorKind::OutOfRange);
        assert_eq!(err.byte_offset(), 7);
        assert_eq!(err.byte_span(), Some((7, 12)));
    }

    #[test]
    fn parse_feature_settings_css_list_requires_quotes() {
        let err = Setting::<u16>::parse_css_list("liga on")
            .next()
            .unwrap()
            .unwrap_err();
        assert_eq!(err.kind(), ParseSettingsErrorKind::InvalidSyntax);
        assert_eq!(err.byte_offset(), 0);
        assert_eq!(err.byte_span(), None);
    }

    #[test]
    fn parse_feature_settings_css_list_invalid_tag_reports_span() {
        let err = Setting::<u16>::parse_css_list(r#""lig" on"#)
            .next()
            .unwrap()
            .unwrap_err();
        assert_eq!(err.kind(), ParseSettingsErrorKind::InvalidTag);
        assert_eq!(err.byte_offset(), 1);
        assert_eq!(err.byte_span(), Some((1, 4)));
    }

    #[test]
    fn parse_variation_settings_css_list_ok() {
        let parsed: Result<Vec<_>, _> =
            Setting::<f32>::parse_css_list(r#""wght" 700, "wdth" 125.5,"#).collect();
        let settings = parsed.unwrap();
        assert_eq!(settings.len(), 2);
        assert_eq!(settings[0].tag, Tag::parse("wght").unwrap());
        assert_eq!(settings[0].value, 700.0);
        assert_eq!(settings[1].tag, Tag::parse("wdth").unwrap());
        assert_eq!(settings[1].value, 125.5);
    }

    #[test]
    fn parse_variation_settings_css_list_requires_value() {
        let err = Setting::<f32>::parse_css_list(r#""wght""#)
            .next()
            .unwrap()
            .unwrap_err();
        assert_eq!(err.kind(), ParseSettingsErrorKind::InvalidSyntax);
        assert_eq!(err.byte_offset(), 6);
    }

    #[test]
    fn parse_variation_settings_css_list_invalid_number_reports_span() {
        let err = Setting::<f32>::parse_css_list(r#""wght" nope"#)
            .next()
            .unwrap()
            .unwrap_err();
        assert_eq!(err.kind(), ParseSettingsErrorKind::InvalidSyntax);
        assert_eq!(err.byte_offset(), 7);
        assert_eq!(err.byte_span(), Some((7, 11)));
    }
}
