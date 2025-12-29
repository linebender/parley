// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec::Vec;

use text_style::{Setting, Tag};

/// Errors that can occur when parsing OpenType settings source strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseSettingsError {
    /// The source string does not conform to the supported syntax.
    InvalidSyntax,
    /// A quoted tag was invalid.
    InvalidTag,
    /// A numeric value was out of range for the target type.
    OutOfRange,
}

impl core::fmt::Display for ParseSettingsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidSyntax => f.write_str("invalid settings syntax"),
            Self::InvalidTag => f.write_str("invalid OpenType tag"),
            Self::OutOfRange => f.write_str("value out of range"),
        }
    }
}

impl core::error::Error for ParseSettingsError {}

fn skip_ws(s: &str, mut i: usize) -> usize {
    while i < s.len() && s.as_bytes()[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn parse_quoted_tag(s: &str, mut i: usize) -> Result<(Tag, usize), ParseSettingsError> {
    i = skip_ws(s, i);
    if s.as_bytes().get(i) != Some(&b'"') {
        return Err(ParseSettingsError::InvalidSyntax);
    }
    i += 1;
    let start = i;
    while i < s.len() && !s.as_bytes()[i].eq(&b'"') {
        i += 1;
    }
    if i >= s.len() {
        return Err(ParseSettingsError::InvalidSyntax);
    }
    let tag_str = &s[start..i];
    i += 1;
    let tag = Tag::parse(tag_str).ok_or(ParseSettingsError::InvalidTag)?;
    Ok((tag, i))
}

fn parse_ident(s: &str, mut i: usize) -> (Option<&str>, usize) {
    i = skip_ws(s, i);
    let start = i;
    while i < s.len() {
        let b = s.as_bytes()[i];
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' {
            i += 1;
        } else {
            break;
        }
    }
    if i == start {
        (None, start)
    } else {
        (Some(&s[start..i]), i)
    }
}

fn parse_number_token(s: &str, mut i: usize) -> (Option<&str>, usize) {
    i = skip_ws(s, i);
    let start = i;
    if s.as_bytes()
        .get(i)
        .is_some_and(|b| *b == b'+' || *b == b'-')
    {
        i += 1;
    }
    let mut saw_digit = false;
    while i < s.len() && s.as_bytes()[i].is_ascii_digit() {
        saw_digit = true;
        i += 1;
    }
    if i < s.len() && s.as_bytes()[i] == b'.' {
        i += 1;
        while i < s.len() && s.as_bytes()[i].is_ascii_digit() {
            saw_digit = true;
            i += 1;
        }
    }
    if !saw_digit {
        return (None, start);
    }
    (Some(&s[start..i]), i)
}

fn parse_comma(s: &str, mut i: usize) -> Result<usize, ParseSettingsError> {
    i = skip_ws(s, i);
    if i >= s.len() {
        return Ok(i);
    }
    if s.as_bytes()[i] == b',' {
        Ok(i + 1)
    } else {
        Err(ParseSettingsError::InvalidSyntax)
    }
}

/// Parses a CSS-like `font-variation-settings` value into a list of settings.
///
/// Supported syntax is a comma-separated list of entries:
/// - tags are required and must be double-quoted: `"wght" 700`
/// - values are required and are parsed as `f32`
///
/// Whitespace is ignored and a trailing comma is permitted.
pub(crate) fn parse_variation_settings(
    source: &Arc<str>,
) -> Result<Vec<Setting<f32>>, ParseSettingsError> {
    parse_settings_impl(source.as_ref(), |token| {
        token
            .parse::<f32>()
            .map_err(|_| ParseSettingsError::InvalidSyntax)
    })
}

/// Parses a CSS-like `font-feature-settings` value into a list of settings.
///
/// Supported syntax is a comma-separated list of entries:
/// - tags are required and must be double-quoted: `"liga" on`
/// - values are optional:
///   - `on`/omitted => `1`
///   - `off` => `0`
///   - a numeric value is parsed as `u16`
///
/// Whitespace is ignored and a trailing comma is permitted.
pub(crate) fn parse_feature_settings(
    source: &Arc<str>,
) -> Result<Vec<Setting<u16>>, ParseSettingsError> {
    parse_settings_impl(source.as_ref(), |token| match token {
        "on" => Ok(1),
        "off" => Ok(0),
        _ => token
            .parse::<u16>()
            .map_err(|_| ParseSettingsError::OutOfRange),
    })
}

fn parse_settings_impl<T>(
    s: &str,
    mut parse_value: impl FnMut(&str) -> Result<T, ParseSettingsError>,
) -> Result<Vec<Setting<T>>, ParseSettingsError> {
    let mut i = 0;
    let mut out = Vec::new();

    loop {
        i = skip_ws(s, i);
        if i >= s.len() {
            break;
        }

        let (tag, next) = parse_quoted_tag(s, i)?;
        i = next;

        let (value_token, next) = parse_number_token(s, i);
        let (ident_token, next_ident) = parse_ident(s, i);

        let (value, next_i) = match (value_token, ident_token) {
            (Some(num), _) => (parse_value(num)?, next),
            (None, Some(ident)) => (parse_value(ident)?, next_ident),
            (None, None) => {
                // Default behavior: for features, omitted means "on"; for variations this will
                // fail in the value parser.
                (parse_value("on")?, i)
            }
        };
        i = next_i;

        out.push(Setting::new(tag, value));

        i = skip_ws(s, i);
        if i >= s.len() {
            break;
        }
        i = parse_comma(s, i)?;
    }

    Ok(out)
}
