// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parsing for CSS-like OpenType settings.
//!
//! This module supports a small, CSS-inspired subset for:
//! - `font-variation-settings` (tag + `f32` value required)
//! - `font-feature-settings` (tag + optional `u16` value)
//!
//! ## Supported syntax
//!
//! Settings are comma-separated. Each entry starts with a quoted 4-byte tag:
//!
//! - `"wght" 700, "wdth" 120`
//! - `'liga' on, "kern" 0, "calt"'`
//!
//! Whitespace is ignored around separators.
//!
//! - Variation values are required and parse as `f32`.
//! - Feature values are optional:
//!   - omitted value defaults to `1`
//!   - `on`/`off` map to `1`/`0`
//!   - otherwise the value parses as an integer `u16`
//!
//! Tags must be exactly 4 characters.

use alloc::vec::Vec;
use core::fmt;

use crate::settings::{Setting, Tag};

/// Errors that can occur while parsing CSS-like OpenType settings.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ParseSettingsError {
    /// The input contained an invalid tag.
    InvalidTag,
    /// The input had invalid syntax.
    InvalidSyntax,
    /// A numeric value failed to parse.
    InvalidNumber,
    /// A numeric value was out of range.
    OutOfRange,
}

impl fmt::Display for ParseSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTag => f.write_str("invalid OpenType tag"),
            Self::InvalidSyntax => f.write_str("invalid settings syntax"),
            Self::InvalidNumber => f.write_str("invalid numeric value"),
            Self::OutOfRange => f.write_str("numeric value out of range"),
        }
    }
}

impl core::error::Error for ParseSettingsError {}

pub(crate) fn parse_variation_settings(
    source: &str,
) -> Result<Vec<Setting<f32>>, ParseSettingsError> {
    parse_settings(source, parse_f32_required)
}

pub(crate) fn parse_feature_settings(
    source: &str,
) -> Result<Vec<Setting<u16>>, ParseSettingsError> {
    parse_settings(source, parse_feature_value_optional)
}

fn parse_settings<T>(
    source: &str,
    mut parse_value: impl FnMut(Option<&str>) -> Result<T, ParseSettingsError>,
) -> Result<Vec<Setting<T>>, ParseSettingsError> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // skip whitespace and commas
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        // tag must be quoted string: "wght" or 'wght'
        let quote = bytes[i];
        if quote != b'"' && quote != b'\'' {
            return Err(ParseSettingsError::InvalidSyntax);
        }
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        if i >= bytes.len() {
            return Err(ParseSettingsError::InvalidSyntax);
        }
        let tag_str =
            core::str::from_utf8(&bytes[start..i]).map_err(|_| ParseSettingsError::InvalidTag)?;
        i += 1; // consume quote

        let tag = Tag::parse(tag_str).ok_or(ParseSettingsError::InvalidTag)?;

        // skip whitespace
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        // value is an unquoted token until comma or whitespace separation rules:
        // take a token until comma, but trim trailing whitespace.
        let value_start = i;
        while i < bytes.len() && bytes[i] != b',' {
            i += 1;
        }
        let value_token = core::str::from_utf8(&bytes[value_start..i])
            .map_err(|_| ParseSettingsError::InvalidSyntax)?
            .trim();
        let value_token = if value_token.is_empty() {
            None
        } else {
            Some(value_token)
        };

        out.push(Setting::new(tag, parse_value(value_token)?));
        // next loop consumes comma/whitespace
    }
    Ok(out)
}

fn parse_f32_required(token: Option<&str>) -> Result<f32, ParseSettingsError> {
    let token = token.ok_or(ParseSettingsError::InvalidSyntax)?;
    token
        .parse::<f32>()
        .map_err(|_| ParseSettingsError::InvalidNumber)
}

fn parse_feature_value_optional(token: Option<&str>) -> Result<u16, ParseSettingsError> {
    let Some(token) = token else {
        return Ok(1);
    };
    match token {
        "on" => Ok(1),
        "off" => Ok(0),
        other => other
            .parse::<u32>()
            .map_err(|_| ParseSettingsError::InvalidNumber)
            .and_then(|v| u16::try_from(v).map_err(|_| ParseSettingsError::OutOfRange)),
    }
}
