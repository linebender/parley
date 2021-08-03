mod collection;
mod font;

use super::FontInstance;
pub use collection::SystemFontCollection;
pub use font::Font;
use swash::{FontRef, Synthesis};

impl FontInstance for Font {
    fn as_font_ref(&self) -> FontRef {
        self.as_ref()
    }

    fn synthesis(&self) -> Option<Synthesis> {
        Some(self.synthesis)
    }
}
