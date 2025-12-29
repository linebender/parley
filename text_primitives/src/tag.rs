// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// A 4-byte OpenType tag (for example `wght`, `liga`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(transparent)]
pub struct Tag(u32);

impl Tag {
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
