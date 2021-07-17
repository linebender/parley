mod builder;
mod context;
mod font;
mod font_cache;
mod itemize;
mod layout;
mod shape;

pub use builder::LayoutBuilder;
pub use context::LayoutContext;
pub use font::Font;
pub use layout::Glyph;

pub use piet;
pub use swash;

use core::ops::Range;
use piet::TextStorage;
use std::rc::Rc;

#[derive(Clone)]
pub struct Layout {
    pub text: Rc<dyn TextStorage>,
    pub glyphs: Vec<Glyph>,
    pub runs: Vec<Run>,
}

#[derive(Clone)]
pub struct Run {
    pub font: Font,
    pub text_range: Range<usize>,
    pub glyph_range: Range<usize>,
}
