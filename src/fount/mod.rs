#![allow(dead_code, unused_variables)]

#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
mod platform;

#[cfg(target_os = "windows")]
#[path = "platform/windows.rs"]
mod platform;

mod context;
mod data;
mod font;
mod id;
mod library;
mod scan;
mod script_tags;

pub use context::FontContext;
pub use font::FontData;
pub use id::{FamilyId, FontId, SourceId};
pub use library::Library;

pub use swash::text::Language as Locale;

use data::*;
use std::sync::Arc;
use swash::{Attributes, CacheKey, Stretch, Style, Weight};

use core::fmt;

/// Describes a generic font family.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum GenericFamily {
    Serif = 0,
    SansSerif = 1,
    Monospace = 2,
    SystemUi = 3,
    Cursive = 4,
    Emoji = 5,
}

impl GenericFamily {
    /// Parses a generic family from a CSS generic family name.
    ///
    /// # Example
    /// ```
    /// use parley::style::GenericFamily;
    ///
    /// assert_eq!(GenericFamily::parse("sans-serif"), Some(GenericFamily::SansSerif));
    /// assert_eq!(GenericFamily::parse("Arial"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "serif" => Self::Serif,
            "sans-serif" => Self::SansSerif,
            "monospace" => Self::Monospace,
            "cursive" => Self::Cursive,
            "system-ui" => Self::SystemUi,
            "emoji" => Self::Emoji,
            _ => return None,
        })
    }
}

impl fmt::Display for GenericFamily {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Self::Serif => "serif",
            Self::SansSerif => "sans-serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::SystemUi => "system-ui",
            Self::Emoji => "emoji",
        };
        write!(f, "{}", name)
    }
}

/// Entry for a font family in a font library.
#[derive(Clone)]
pub struct FamilyEntry {
    id: FamilyId,
    has_stretch: bool,
    kind: FontFamilyKind,
}

impl FamilyEntry {
    /// Returns the identifier for the font family.
    pub fn id(&self) -> FamilyId {
        self.id
    }

    /// Returns the name of the font family.
    pub fn name(&self) -> &str {
        match &self.kind {
            FontFamilyKind::Static(name, _) => name,
            FontFamilyKind::Dynamic(data) => &data.name,
        }
    }

    /// Returns an iterator over the fonts that are members of the family.
    pub fn fonts<'a>(&'a self) -> impl Iterator<Item = FontId> + Clone + 'a {
        self.fonts_with_attrs().map(|font| font.0)
    }

    /// Returns the font that most closely matches the specified attributes.
    pub fn query(&self, attributes: Attributes) -> Option<FontId> {
        let style = attributes.style();
        let weight = attributes.weight();
        let stretch = attributes.stretch();
        let mut min_stretch_dist = i32::MAX;
        let mut matching_stretch = Stretch::NORMAL;
        if self.has_stretch {
            if stretch <= Stretch::NORMAL {
                for font in self.fonts_with_attrs() {
                    let val = font.1;
                    let font_stretch = if val > Stretch::NORMAL {
                        val.raw() as i32 - Stretch::NORMAL.raw() as i32
                            + Stretch::ULTRA_EXPANDED.raw() as i32
                    } else {
                        val.raw() as i32
                    };
                    let offset = (font_stretch - stretch.raw() as i32).abs();
                    if offset < min_stretch_dist {
                        min_stretch_dist = offset;
                        matching_stretch = val;
                    }
                }
            } else {
                for font in self.fonts_with_attrs() {
                    let val = font.1;
                    let font_stretch = if val < Stretch::NORMAL {
                        val.raw() as i32 - Stretch::NORMAL.raw() as i32
                            + Stretch::ULTRA_EXPANDED.raw() as i32
                    } else {
                        val.raw() as i32
                    };
                    let offset = (font_stretch - stretch.raw() as i32).abs();
                    if offset < min_stretch_dist {
                        min_stretch_dist = offset;
                        matching_stretch = val;
                    }
                }
            }
        }
        let mut matching_style;
        match style {
            Style::Normal => {
                matching_style = Style::Italic;
                for font in self.fonts_with_attrs().filter(|f| f.1 == matching_stretch) {
                    let val = font.3;
                    match val {
                        Style::Normal => {
                            matching_style = style;
                            break;
                        }
                        Style::Oblique(_) => {
                            matching_style = val;
                        }
                        _ => {}
                    }
                }
            }
            Style::Oblique(_) => {
                matching_style = Style::Normal;
                for font in self.fonts_with_attrs().filter(|f| f.1 == matching_stretch) {
                    let val = font.3;
                    match val {
                        Style::Oblique(_) => {
                            matching_style = style;
                            break;
                        }
                        Style::Italic => {
                            matching_style = val;
                        }
                        _ => {}
                    }
                }
            }
            Style::Italic => {
                matching_style = Style::Normal;
                for font in self.fonts_with_attrs().filter(|f| f.1 == matching_stretch) {
                    let val = font.3;
                    match val {
                        Style::Italic => {
                            matching_style = style;
                            break;
                        }
                        Style::Oblique(_) => {
                            matching_style = val;
                        }
                        _ => {}
                    }
                }
            }
        }
        // If the desired weight is inclusively between 400 and 500
        if weight >= Weight(400) && weight <= Weight(500) {
            // weights greater than or equal to the target weight are checked
            // in ascending order until 500 is hit and checked
            for font in self.fonts_with_attrs().filter(|f| {
                f.1 == matching_stretch
                    && f.3 == matching_style
                    && f.2 >= weight
                    && f.2 <= Weight(500)
            }) {
                return Some(font.0);
            }
            // followed by weights less than the target weight in descending
            // order
            for font in self
                .fonts_with_attrs()
                .rev()
                .filter(|f| f.1 == matching_stretch && f.3 == matching_style && f.2 < weight)
            {
                return Some(font.0);
            }
            // followed by weights greater than 500, until a match is found
            return self
                .fonts_with_attrs()
                .filter(|f| f.1 == matching_stretch && f.3 == matching_style && f.2 > Weight(500))
                .map(|f| f.0)
                .next();
        // If the desired weight is less than 400
        } else if weight < Weight(400) {
            // weights less than or equal to the desired weight are checked in
            // descending order
            for font in self
                .fonts_with_attrs()
                .rev()
                .filter(|f| f.1 == matching_stretch && f.3 == matching_style && f.2 <= weight)
            {
                return Some(font.0);
            }
            // followed by weights above the desired weight in ascending order
            // until a match is found
            return self
                .fonts_with_attrs()
                .filter(|f| f.1 == matching_stretch && f.3 == matching_style && f.2 > weight)
                .map(|f| f.0)
                .next();
        // If the desired weight is greater than 500
        } else {
            // weights greater than or equal to the desired weight are checked
            // in ascending order
            for font in self
                .fonts_with_attrs()
                .filter(|f| f.1 == matching_stretch && f.3 == matching_style && f.2 >= weight)
            {
                return Some(font.0);
            }
            // followed by weights below the desired weight in descending order
            // until a match is found
            return self
                .fonts_with_attrs()
                .rev()
                .filter(|f| f.1 == matching_stretch && f.3 == matching_style && f.2 < weight)
                .map(|f| f.0)
                .next();
        }
    }

    fn fonts_with_attrs<'a>(
        &'a self,
    ) -> impl Iterator<Item = &(FontId, Stretch, Weight, Style)> + DoubleEndedIterator + Clone + 'a
    {
        let fonts = match &self.kind {
            FontFamilyKind::Static(_, fonts) => *fonts,
            FontFamilyKind::Dynamic(data) => &data.fonts,
        };
        fonts.iter()
    }
}

#[derive(Clone)]
enum FontFamilyKind {
    Static(&'static str, &'static [(FontId, Stretch, Weight, Style)]),
    Dynamic(Arc<FamilyData>),
}

/// Iterator over the font families in a font library.
#[derive(Clone)]
pub struct Families {
    user: Arc<(u64, CollectionData)>,
    library: Library,
    pos: usize,
    stage: u8,
}

impl Iterator for Families {
    type Item = FamilyEntry;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.stage == 0 {
                let len = self.user.1.families.len();
                if self.pos >= len {
                    self.stage = 1;
                    continue;
                }
                let pos = self.pos;
                self.pos += 1;
                return self.user.1.family(FamilyId::new_user(pos as u32));
            } else {
                let pos = self.pos;
                self.pos += 1;
                return self.library.inner.system.family(FamilyId::new(pos as u32));
            }
        }
    }
}

/// Entry for a font in a font library.
#[derive(Copy, Clone)]
pub struct FontEntry {
    id: FontId,
    family: FamilyId,
    source: SourceId,
    index: u32,
    attributes: Attributes,
    cache_key: CacheKey,
}

impl FontEntry {
    /// Returns the identifier for the font.
    pub fn id(&self) -> FontId {
        self.id
    }

    /// Returns the identifier for the family that contains the font.
    pub fn family(&self) -> FamilyId {
        self.family
    }

    /// Returns the identifier for the source that contains the font.
    pub fn source(&self) -> SourceId {
        self.source
    }

    /// Returns the index of the font within the corresponding source.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Returns the primary font attributes.
    pub fn attributes(&self) -> Attributes {
        self.attributes
    }

    /// Returns the cache key for the font.
    pub fn cache_key(&self) -> CacheKey {
        self.cache_key
    }
}

/// Entry for a font source in a font library.
#[derive(Clone)]
pub struct SourceEntry {
    id: SourceId,
    kind: SourceKind,
}

impl SourceEntry {
    /// Returns the identifier for the font source.
    pub fn id(&self) -> SourceId {
        self.id
    }

    /// Returns the kind of the font source.
    pub fn kind(&self) -> &SourceKind {
        &self.kind
    }
}

/// The kind of a font source.
#[derive(Clone)]
pub enum SourceKind {
    /// File name of the source. Pair with [`SourcePaths`] to locate the file.
    FileName(&'static str),
    /// Full path to a font file.
    Path(Arc<str>),
    /// Shared buffer containing font data.
    Data(FontData),
}

/// Context that describes the result of font registration.
#[derive(Clone, Default)]
pub struct Registration {
    /// List of font families that were registered.
    pub families: Vec<FamilyId>,
    /// List of fonts that were registered.
    pub fonts: Vec<FontId>,
}
