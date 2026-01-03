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
/// Any additional subtags (variants, extensions, private use) are ignored, but the input must
/// still be well-formed enough that `script`/`region` aren't silently dropped (for example, a
/// region subtag after a variant is treated as an error).
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
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len as usize])
            .expect("only ASCII")
    }

    /// Returns the primary language subtag (lowercase).
    #[inline(always)]
    pub fn language(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.language_len as usize])
            .expect("only ASCII")
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
                .expect("only ASCII"),
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
                .expect("only ASCII"),
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
    /// The script subtag was malformed or appeared in an invalid position.
    InvalidScript,
    /// The region subtag was malformed or appeared in an invalid position.
    InvalidRegion,
    /// The tag contained an invalid or unsupported subtag sequence.
    InvalidSubtag,
}

impl fmt::Display for ParseLanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLanguage => f.write_str("invalid primary language subtag"),
            Self::InvalidScript => f.write_str("invalid script subtag"),
            Self::InvalidRegion => f.write_str("invalid region subtag"),
            Self::InvalidSubtag => f.write_str("invalid language subtag sequence"),
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
            #[allow(
                clippy::cast_possible_truncation,
                reason = "language subtag length is 2 or 3"
            )]
            language_len: language_bytes.len() as u8,
            script_len: 0,
            region_len: 0,
        };

        // language: lower
        for (i, b) in language_bytes.iter().enumerate() {
            out.bytes[i] = b.to_ascii_lowercase();
        }
        out.len = out.language_len;

        let mut seen_variant = false;
        let mut seen_extension_or_private = false;

        for part in parts {
            if seen_extension_or_private {
                break;
            }
            let b = part.as_bytes();

            // Extension or private use: we stop validating at this point.
            if b.len() == 1 && b[0].is_ascii_alphanumeric() {
                seen_extension_or_private = true;
                continue;
            }

            if is_variant(b) {
                seen_variant = true;
                continue;
            }

            // Script (BCP 47): 4 alpha.
            if b.len() == 4 {
                if !b.iter().all(|c| c.is_ascii_alphabetic()) {
                    return Err(ParseLanguageError::InvalidScript);
                }
                if out.script_len != 0 || out.region_len != 0 || seen_variant {
                    return Err(ParseLanguageError::InvalidScript);
                }
                out.bytes[out.len as usize] = b'-';
                out.len += 1;
                out.script_len = 4;
                // titlecase script
                out.bytes[out.len as usize] = b[0].to_ascii_uppercase();
                out.bytes[out.len as usize + 1] = b[1].to_ascii_lowercase();
                out.bytes[out.len as usize + 2] = b[2].to_ascii_lowercase();
                out.bytes[out.len as usize + 3] = b[3].to_ascii_lowercase();
                out.len += 4;
                continue;
            }

            // Region (BCP 47): 2 alpha or 3 digit.
            if b.len() == 2 || b.len() == 3 {
                let is_alpha2 = b.len() == 2 && b.iter().all(|c| c.is_ascii_alphabetic());
                let is_digit3 = b.len() == 3 && b.iter().all(|c| c.is_ascii_digit());
                if !(is_alpha2 || is_digit3) {
                    return Err(ParseLanguageError::InvalidRegion);
                }
                if out.region_len != 0 || seen_variant {
                    return Err(ParseLanguageError::InvalidRegion);
                }
                out.bytes[out.len as usize] = b'-';
                out.len += 1;
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "region subtag length is 2 or 3"
                )]
                {
                    out.region_len = b.len() as u8;
                }
                if is_alpha2 {
                    out.bytes[out.len as usize] = b[0].to_ascii_uppercase();
                    out.bytes[out.len as usize + 1] = b[1].to_ascii_uppercase();
                } else {
                    out.bytes[out.len as usize] = b[0];
                    out.bytes[out.len as usize + 1] = b[1];
                    out.bytes[out.len as usize + 2] = b[2];
                }
                out.len += out.region_len;
                continue;
            }

            return Err(ParseLanguageError::InvalidSubtag);
        }

        Ok(out)
    }
}

fn is_variant(bytes: &[u8]) -> bool {
    // BCP 47 variant: 5-8 alphanum or 4 alphanum starting with a digit.
    if bytes.len() == 4 {
        bytes[0].is_ascii_digit() && bytes.iter().all(|b| b.is_ascii_alphanumeric())
    } else if (5..=8).contains(&bytes.len()) {
        bytes.iter().all(|b| b.is_ascii_alphanumeric())
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Language, ParseLanguageError};

    #[test]
    fn parse_language_only() {
        let lang = Language::parse("EN").unwrap();
        assert_eq!(lang.as_str(), "en");
        assert_eq!(lang.language(), "en");
        assert_eq!(lang.script(), None);
        assert_eq!(lang.region(), None);
    }

    #[test]
    fn parse_with_script_and_region() {
        let lang = Language::parse("zh-Hans-CN").unwrap();
        assert_eq!(lang.as_str(), "zh-Hans-CN");
        assert_eq!(lang.language(), "zh");
        assert_eq!(lang.script(), Some("Hans"));
        assert_eq!(lang.region(), Some("CN"));
    }

    #[test]
    fn parse_with_region_only() {
        let lang = Language::parse("es_419").unwrap();
        assert_eq!(lang.as_str(), "es-419");
        assert_eq!(lang.region(), Some("419"));
    }

    #[test]
    fn parse_ignores_variants_after_region() {
        let lang = Language::parse("en-Latn-US-posix").unwrap();
        assert_eq!(lang.as_str(), "en-Latn-US");
    }

    #[test]
    fn invalid_language_errors() {
        assert_eq!(
            Language::parse("").unwrap_err(),
            ParseLanguageError::InvalidLanguage
        );
        assert_eq!(
            Language::parse("e").unwrap_err(),
            ParseLanguageError::InvalidLanguage
        );
        assert_eq!(
            Language::parse("en1").unwrap_err(),
            ParseLanguageError::InvalidLanguage
        );
    }

    #[test]
    fn invalid_script_errors() {
        assert_eq!(
            Language::parse("en-La1n").unwrap_err(),
            ParseLanguageError::InvalidScript
        );
        assert_eq!(
            Language::parse("en-Latn-US-Latn").unwrap_err(),
            ParseLanguageError::InvalidScript
        );
    }

    #[test]
    fn invalid_region_errors() {
        assert_eq!(
            Language::parse("en-U1").unwrap_err(),
            ParseLanguageError::InvalidRegion
        );
        assert_eq!(
            Language::parse("en-Latn-US-CA").unwrap_err(),
            ParseLanguageError::InvalidRegion
        );
        assert_eq!(
            Language::parse("en-Latin-US").unwrap_err(),
            ParseLanguageError::InvalidRegion
        );
    }

    #[test]
    fn invalid_subtag_errors() {
        assert_eq!(
            Language::parse("en-abc$e").unwrap_err(),
            ParseLanguageError::InvalidSubtag
        );
    }
}
