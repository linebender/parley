pub mod adapter;
pub mod font;

mod builder;
mod context;
mod itemize;
mod layout;
mod session;
mod shape;

pub use context::LayoutBuilder;
pub use context::LayoutContext;
use font::*;
pub use layout::Glyph;

pub use piet;
pub use swash;

use core::ops::Range;

#[derive(Clone)]
pub struct Layout<F: FontHandle> {
    pub glyphs: Vec<Glyph>,
    pub runs: Vec<Run<F>>,
}

#[derive(Clone)]
pub struct Run<F: FontHandle> {
    pub font: F,
    pub text_range: Range<usize>,
    pub glyph_range: Range<usize>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Attribute<'a, B: Brush = ()> {
    FontFamily(FontFamily<'a>),
    FontSize(f32),
    FontStretch(FontStretch),
    FontStyle(FontStyle),
    FontWeight(FontWeight),
    Color(Color<B>),
    Underline(bool),
    Strikethrough(bool),
}

impl<'a> From<FontFamily<'a>> for Attribute<'a> {
    fn from(family: FontFamily<'a>) -> Self {
        Self::FontFamily(family)
    }
}

impl From<FontStyle> for Attribute<'_> {
    fn from(style: FontStyle) -> Self {
        Self::FontStyle(style)
    }
}

impl From<FontWeight> for Attribute<'_> {
    fn from(weight: FontWeight) -> Self {
        Self::FontWeight(weight)
    }
}

impl From<FontStretch> for Attribute<'_> {
    fn from(stretch: FontStretch) -> Self {
        Self::FontStretch(stretch)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Alignment {
    Start,
    End,
    Center,
    Justified,
}

/// Rendering style for glyphs and text decorations.
#[derive(Clone, PartialEq, Debug)]
pub enum Color<B: Brush = ()> {
    /// 32-bit color in RGBA order.
    Solid([u8; 4]),
    /// Custom brush.
    Brush(B)
}

/// Trait for types that represent custom rendering styles.
pub trait Brush: Clone + PartialEq + core::fmt::Debug {}

impl Brush for () {}
