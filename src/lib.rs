extern crate alloc;

pub use swash;

mod bidi;
pub mod font;
mod resolve;
mod shape;
mod swash_convert;
mod util;

pub mod context;
pub mod fontique;
pub mod layout;
pub mod style;

pub use peniko::Font;

pub use context::LayoutContext;
pub use font::FontContext;
pub use layout::Layout;
