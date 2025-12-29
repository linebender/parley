// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec::Vec;

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

/// Settings that can be supplied as raw CSS-like source or a parsed list.
///
/// This enables accepting CSS-like input without making strings the primary representation. When
/// resolution occurs, `Source` values are parsed into `List` values.
#[derive(Clone, PartialEq, Debug)]
pub enum Settings<T> {
    /// A raw source string (CSS-like syntax).
    Source(Arc<str>),
    /// A parsed list of settings.
    List(Vec<Setting<T>>),
}

impl<T> Settings<T> {
    /// Creates settings from a raw source string.
    pub fn source(source: impl Into<Arc<str>>) -> Self {
        Self::Source(source.into())
    }

    /// Creates settings from a parsed list.
    pub fn list(list: Vec<Setting<T>>) -> Self {
        Self::List(list)
    }
}
