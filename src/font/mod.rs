use fount::Locale;
use swash::text::cluster::CharCluster;
use swash::text::Script;
use swash::{Attributes, FontRef, Synthesis};

use core::fmt::Debug;

pub use fount::GenericFamily as GenericFontFamily;
pub use swash::{Stretch as FontStretch, Style as FontStyle, Weight as FontWeight};

pub mod system;

pub trait FontInstance: Clone + PartialEq {
    fn as_font_ref(&self) -> FontRef;

    fn synthesis(&self) -> Option<Synthesis> {
        None
    }
}

pub trait FontCollection {
    type Family: Clone + PartialEq + Debug;
    type Font: FontInstance;

    /// Begins a layout sesion with this collection.
    fn begin_session(&mut self);

    /// Ends a layout session with this collection.
    fn end_session(&mut self);

    /// Returns a handle for the font family in the collection with the specified family and attributes. Handles
    /// returned by this function are only guaranteed to be valid between calls to `begin_session` and
    /// `end_session`.
    fn query_family(&mut self, name: &str) -> Option<Self::Family>;

    /// Uses the specified family, attributes and fallbacks to select an appropriate font for a character cluster.
    fn map(
        &mut self,
        family: &FontFamilyHandle<Self::Family>,
        attributes: impl Into<Attributes>,
        fallbacks: &FontFallbacks,
        cluster: &mut CharCluster,
    ) -> Option<Self::Font>;
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct FontFallbacks {
    pub script: Script,
    pub locale: Option<Locale>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FontFamilyHandle<F: Clone + PartialEq + Debug> {
    Default,
    Named(F),
    Generic(GenericFontFamily),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FontFamily<'a> {
    Named(&'a str),
    Generic(GenericFontFamily),
}
