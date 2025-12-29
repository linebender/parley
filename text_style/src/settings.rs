// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec::Vec;

pub use text_primitives::{Setting, Tag};

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
