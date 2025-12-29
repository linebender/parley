// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

/// A font family name or generic family.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// A named font family (for example `"Inter"`).
    Named(Arc<str>),
    /// A generic font family (for example `sans-serif`).
    Generic(GenericFamily),
}

impl FontFamily {
    /// Creates a named font family.
    pub fn named(name: impl Into<Arc<str>>) -> Self {
        Self::Named(name.into())
    }
}

/// Generic font families, named after CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GenericFamily {
    /// The `serif` generic family.
    Serif,
    /// The `sans-serif` generic family.
    SansSerif,
    /// The `monospace` generic family.
    Monospace,
    /// The `cursive` generic family.
    Cursive,
    /// The `fantasy` generic family.
    Fantasy,
    /// The `system-ui` generic family.
    SystemUi,
    /// The `emoji` generic family.
    Emoji,
    /// The `math` generic family.
    Math,
    /// The `fangsong` generic family.
    Fangsong,
}

/// A prioritized list of font families.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontStack {
    families: Vec<FontFamily>,
}

impl FontStack {
    /// Creates an empty font stack.
    pub fn new() -> Self {
        Self {
            families: Vec::new(),
        }
    }

    /// Creates a font stack with a single family.
    pub fn single(family: FontFamily) -> Self {
        let families = vec![family];
        Self { families }
    }

    /// Appends a family to the end of the stack.
    pub fn push(&mut self, family: FontFamily) {
        self.families.push(family);
    }

    /// Returns an iterator over the families in this stack.
    pub fn iter(&self) -> impl Iterator<Item = &FontFamily> {
        self.families.iter()
    }
}

impl Default for FontStack {
    fn default() -> Self {
        Self::single(FontFamily::Generic(GenericFamily::SansSerif))
    }
}
