// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// A 4-byte OpenType tag (for example `wght`, `liga`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct Tag(u32);

impl Tag {
    /// Creates a tag from a 4-byte array reference.
    pub const fn new(bytes: &[u8; 4]) -> Self {
        Self::from_bytes(*bytes)
    }

    /// Creates a tag from 4 bytes.
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(u32::from_be_bytes(bytes))
    }

    /// Returns this tag as 4 bytes.
    pub const fn to_bytes(self) -> [u8; 4] {
        self.0.to_be_bytes()
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
    pub fn parse_list(s: &str) -> impl Iterator<Item = Self> + '_ + Clone {
        ParseList::new(s)
            .map(|(_, tag, value_str)| {
                let (ok, value) = match value_str {
                    "on" | "" => (true, 1),
                    "off" => (true, 0),
                    _ => match value_str.parse::<u16>() {
                        Ok(value) => (true, value),
                        _ => (false, 0),
                    },
                };
                (ok, tag, value)
            })
            .take_while(|(ok, _, _)| *ok)
            .map(|(_, tag, value)| Self { tag, value })
    }
}

impl Setting<f32> {
    /// Parses a comma-separated list of variation settings according to the CSS grammar.
    pub fn parse_list(s: &str) -> impl Iterator<Item = Self> + '_ + Clone {
        ParseList::new(s)
            .map(|(_, tag, value_str)| {
                let (ok, value) = match value_str.parse::<f32>() {
                    Ok(value) => (true, value),
                    _ => (false, 0.0),
                };
                (ok, tag, value)
            })
            .take_while(|(ok, _, _)| *ok)
            .map(|(_, tag, value)| Self { tag, value })
    }
}

#[derive(Clone)]
struct ParseList<'a> {
    source: &'a [u8],
    len: usize,
    pos: usize,
}

impl<'a> ParseList<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            len: source.len(),
            pos: 0,
        }
    }
}

impl<'a> Iterator for ParseList<'a> {
    type Item = (usize, Tag, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
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
        let quote = match first {
            b'"' | b'\'' => {
                pos += 1;
                start += 1;
                first
            }
            _ => return None,
        };
        let mut tag_str = None;
        while pos < self.len {
            if self.source[pos] == quote {
                tag_str = core::str::from_utf8(self.source.get(start..pos)?).ok();
                pos += 1;
                break;
            }
            pos += 1;
        }
        self.pos = pos;
        let tag_str = tag_str?;
        if !tag_str.is_ascii() {
            return None;
        }
        let tag = Tag::new(&tag_str.as_bytes().try_into().ok()?);
        while pos < self.len {
            if !self.source[pos].is_ascii_whitespace() {
                break;
            }
            pos += 1;
        }
        self.pos = pos;
        start = pos;
        let mut end = start;
        while pos < self.len {
            if self.source[pos] == b',' {
                pos += 1;
                break;
            }
            pos += 1;
            end += 1;
        }
        let value = core::str::from_utf8(self.source.get(start..end)?)
            .ok()?
            .trim();
        self.pos = pos;
        Some((pos, tag, value))
    }
}
