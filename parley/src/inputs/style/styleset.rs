// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::mem::Discriminant;

use hashbrown::HashMap;

use crate::inputs::Brush;

type StyleProperty<Brush> = crate::inputs::StyleProperty<'static, Brush>;

/// A long-lived collection of [`StyleProperties`](super::StyleProperty), containing at
/// most one of each property.
///
/// This is used by [`PlainEditor`](crate::editing::PlainEditor) to provide a reasonably ergonomic
/// mutable API for styles applied to all text managed by it.
/// This can be accessed using [`PlainEditor::edit_styles`](crate::editing::PlainEditor::edit_styles).
///
/// These styles do not have a corresponding range, and are generally unsuited for rich text.
#[derive(Clone, Debug)]
pub struct StyleSet<B: Brush>(HashMap<Discriminant<StyleProperty<B>>, StyleProperty<B>>);

impl<B: Brush> StyleSet<B> {
    /// Create a new collection of styles.
    ///
    /// The font size will be `font_size`, and can be overwritten at runtime by
    /// [inserting](Self::insert) a new [`FontSize`](crate::inputs::StyleProperty::FontSize).
    pub fn new(font_size: f32) -> Self {
        let mut this = Self(Default::default());
        this.insert(StyleProperty::FontSize(font_size));
        this
    }

    /// Add `style` to this collection, returning any overwritten value.
    ///
    /// Note: Adding a [font stack](crate::inputs::StyleProperty::FontStack) to this collection is not
    /// additive, and instead overwrites any previously added font stack.
    pub fn insert(&mut self, style: StyleProperty<B>) -> Option<StyleProperty<B>> {
        let discriminant = core::mem::discriminant(&style);
        self.0.insert(discriminant, style)
    }

    /// [Retain](std::vec::Vec::retain) only the styles for which `f` returns true.
    ///
    /// Styles which are removed return to their default values.
    ///
    /// Removing the [font size](crate::inputs::StyleProperty::FontSize) is not recommended, as an unspecified
    /// fallback font size will be used.
    pub fn retain(&mut self, mut f: impl FnMut(&StyleProperty<B>) -> bool) {
        self.0.retain(|_, v| f(v));
    }

    /// Remove the style with the discriminant `property`.
    ///
    /// Styles which are removed return to their default values.
    ///
    /// To get the discriminant requires constructing a valid `StyleProperty` for the
    /// the desired property and passing it to [`core::mem::discriminant`].
    /// Getting this discriminant is usually possible in a `const` context.
    ///
    /// Removing the [font size](crate::inputs::StyleProperty::FontSize) is not recommended, as an unspecified
    /// fallback font size will be used.
    pub fn remove(&mut self, property: Discriminant<StyleProperty<B>>) -> Option<StyleProperty<B>> {
        self.0.remove(&property)
    }

    /// Read the raw underlying storage of this.
    ///
    /// Write access is not provided due to the invariant that keys
    /// are the discriminant of their corresponding value.
    pub fn inner(&self) -> &HashMap<Discriminant<StyleProperty<B>>, StyleProperty<B>> {
        &self.0
    }
}