// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

type Tag = u32;
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct Setting<T> {
    /// The tag that identifies the setting.
    pub tag: Tag,
    /// The value for the setting.
    pub value: T,
}

/// Creates a tag from four bytes.
pub(crate) const fn tag_from_bytes(bytes: [u8; 4]) -> Tag {
    (bytes[0] as u32) << 24 | (bytes[1] as u32) << 16 | (bytes[2] as u32) << 8 | bytes[3] as u32
}

/// Creates a tag from the first four bytes of a string, inserting
/// spaces for any missing bytes.
fn tag_from_str_lossy(s: &str) -> Tag {
    let mut bytes = [b' '; 4];
    for (i, b) in s.as_bytes().iter().enumerate().take(4) {
        bytes[i] = *b;
    }
    tag_from_bytes(bytes)
}

impl Setting<u16> {
    /// Parses a comma separated list of feature settings according to the CSS
    /// grammar.
    pub(crate) fn parse_list(s: &str) -> impl Iterator<Item = Self> + '_ + Clone {
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
    /// Parses a comma separated list of variation settings according to the
    /// CSS grammar.
    pub(crate) fn parse_list(s: &str) -> impl Iterator<Item = Self> + '_ + Clone {
        ParseList::new(s)
            .map(|(_, tag, value_str)| {
                let (ok, value) = match value_str.parse::<f32>() {
                    Ok(value) => (true, value),
                    _ => (false, 0.),
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
        if tag_str.len() != 4 || !tag_str.is_ascii() {
            return None;
        }
        let tag = tag_from_str_lossy(tag_str);
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
