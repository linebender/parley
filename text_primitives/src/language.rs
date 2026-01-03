// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;
use core::str::FromStr;

/// A compact, zero-allocation language tag.
///
/// This type captures only the `language` + optional `script` + optional `region` subtags from a
/// BCP 47 language tag, normalized to common casing conventions:
/// - language: lowercase (2–3 letters)
/// - script: titlecase (4 letters)
/// - region: uppercase (2 letters) or digits (3 digits)
///
/// Any additional subtags (variants, extensions, private use) are ignored.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Language {
    bytes: [u8; 12],
    len: u8,
    language_len: u8,
    script_len: u8,
    region_len: u8,
}

impl Language {
    /// The maximum length of the canonical `language[-Script][-REGION]` form.
    pub const MAX_LEN: usize = 12;

    /// The “undefined” language (`und`).
    pub const UND: Self = Self::from_language_bytes(*b"und", 3);

    /// Parses a language tag, keeping only language/script/region.
    pub fn parse(s: &str) -> Result<Self, ParseLanguageError> {
        s.parse()
    }

    /// Returns the canonical string form (`language[-Script][-REGION]`).
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len as usize])
            .expect("Language stores only ASCII bytes")
    }

    /// Returns the primary language subtag (lowercase).
    pub fn language(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.language_len as usize])
            .expect("Language stores only ASCII bytes")
    }

    /// Returns the script subtag (titlecase), if present.
    pub fn script(&self) -> Option<&str> {
        if self.script_len == 0 {
            return None;
        }
        let start = self.language_len as usize + 1;
        let end = start + self.script_len as usize;
        Some(
            core::str::from_utf8(&self.bytes[start..end])
                .expect("Language stores only ASCII bytes"),
        )
    }

    /// Returns the region subtag (uppercase or digits), if present.
    pub fn region(&self) -> Option<&str> {
        if self.region_len == 0 {
            return None;
        }
        let mut start = self.language_len as usize;
        if self.script_len != 0 {
            start += 1 + self.script_len as usize;
        }
        start += 1;
        let end = start + self.region_len as usize;
        Some(
            core::str::from_utf8(&self.bytes[start..end])
                .expect("Language stores only ASCII bytes"),
        )
    }

    const fn from_language_bytes(bytes: [u8; 3], len: u8) -> Self {
        let mut out = Self {
            bytes: [0; 12],
            len: 0,
            language_len: len,
            script_len: 0,
            region_len: 0,
        };
        out.bytes[0] = bytes[0];
        out.bytes[1] = bytes[1];
        out.bytes[2] = bytes[2];
        out.len = len;
        out
    }
}

impl fmt::Debug for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Language").field(&self.as_str()).finish()
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An error returned when parsing a [`Language`] fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseLanguageError {
    /// The input did not contain a valid primary language subtag.
    InvalidLanguage,
}

impl fmt::Display for ParseLanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLanguage => f.write_str("invalid primary language subtag"),
        }
    }
}

impl core::error::Error for ParseLanguageError {}

impl FromStr for Language {
    type Err = ParseLanguageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(['-', '_']).filter(|p| !p.is_empty());

        let language = parts.next().ok_or(ParseLanguageError::InvalidLanguage)?;
        let language_bytes = language.as_bytes();
        if !(2..=3).contains(&language_bytes.len())
            || !language_bytes.iter().all(|b| b.is_ascii_alphabetic())
        {
            return Err(ParseLanguageError::InvalidLanguage);
        }

        let mut out = Self {
            bytes: [0; 12],
            len: 0,
            language_len: u8::try_from(language_bytes.len())
                .expect("language subtag length is at most 3"),
            script_len: 0,
            region_len: 0,
        };

        // language: lower
        for (i, b) in language_bytes.iter().enumerate() {
            out.bytes[i] = b.to_ascii_lowercase();
        }
        out.len = out.language_len;

        let mut maybe_script_or_region = parts.next();
        if let Some(part) = maybe_script_or_region.take() {
            let b = part.as_bytes();
            if b.len() == 4 && b.iter().all(|c| c.is_ascii_alphabetic()) {
                out.bytes[out.len as usize] = b'-';
                out.len += 1;
                out.script_len = 4;
                // titlecase script
                out.bytes[out.len as usize] = b[0].to_ascii_uppercase();
                out.bytes[out.len as usize + 1] = b[1].to_ascii_lowercase();
                out.bytes[out.len as usize + 2] = b[2].to_ascii_lowercase();
                out.bytes[out.len as usize + 3] = b[3].to_ascii_lowercase();
                out.len += 4;
                maybe_script_or_region = parts.next();
            } else {
                maybe_script_or_region = Some(part);
            }
        }

        if let Some(part) = maybe_script_or_region.take() {
            let b = part.as_bytes();
            let is_alpha2 = b.len() == 2 && b.iter().all(|c| c.is_ascii_alphabetic());
            let is_digit3 = b.len() == 3 && b.iter().all(|c| c.is_ascii_digit());
            if is_alpha2 || is_digit3 {
                out.bytes[out.len as usize] = b'-';
                out.len += 1;
                out.region_len = u8::try_from(b.len()).expect("region subtag length is 2 or 3");
                if is_alpha2 {
                    out.bytes[out.len as usize] = b[0].to_ascii_uppercase();
                    out.bytes[out.len as usize + 1] = b[1].to_ascii_uppercase();
                } else {
                    out.bytes[out.len as usize] = b[0];
                    out.bytes[out.len as usize + 1] = b[1];
                    out.bytes[out.len as usize + 2] = b[2];
                }
                out.len += out.region_len;
            }
        }

        Ok(out)
    }
}
