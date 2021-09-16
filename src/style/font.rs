use core::fmt;

pub use fount::GenericFamily;
pub use swash::{ObliqueAngle, Stretch as FontStretch, Style as FontStyle, Weight as FontWeight};

/// Setting for a font variation.
pub type FontVariation = swash::Setting<f32>;

/// Setting for a font feature.
pub type FontFeature = swash::Setting<u16>;

/// Prioritized sequence of font families.
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/font-family>
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FontStack<'a> {
    /// Font family list in CSS format.
    Source(&'a str),
    /// Single font family.
    Single(FontFamily<'a>),
    /// Ordered list of font families.
    List(&'a [FontFamily<'a>]),
}

/// Named or generic font family.
///
/// <https://developer.mozilla.org/en-US/docs/Web/CSS/font-family>
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FontFamily<'a> {
    /// Named font family.
    Named(&'a str),
    /// Generic font family.
    Generic(GenericFamily),
}

impl<'a> FontFamily<'a> {
    /// Parses a font family containing a name or a generic family.
    ///
    /// # Example
    /// ```
    /// use font_types::FontFamily::{self, *};
    /// use font_types::GenericFamily::*;
    ///
    /// assert_eq!(FontFamily::parse("Palatino Linotype"), Some(Named("Palatino Linotype")));
    /// assert_eq!(FontFamily::parse("monospace"), Some(Generic(Monospace)));
    ///
    /// // Note that you can quote a generic family to capture it as a named family:
    ///
    /// assert_eq!(FontFamily::parse("'monospace'"), Some(Named("monospace")));
    /// ```    
    pub fn parse(s: &'a str) -> Option<Self> {
        Self::parse_list(s).next()
    }

    /// Parses a comma separated list of font families.
    ///
    /// # Example
    /// ```
    /// use font_types::FontFamily::{self, *};
    /// use font_types::GenericFamily::*;
    ///
    /// let source = "Arial, 'Times New Roman', serif";
    ///
    /// let parsed_families = FontFamily::parse_list(source).collect::<Vec<_>>();
    /// let families = vec![Named("Arial"), Named("Times New Roman"), Generic(Serif)];
    ///
    /// assert_eq!(parsed_families, families);
    /// ```
    pub fn parse_list(s: &'a str) -> impl Iterator<Item = FontFamily<'a>> + 'a + Clone {
        ParseList {
            source: s.as_bytes(),
            len: s.len(),
            pos: 0,
        }
    }
}

impl fmt::Display for FontFamily<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Named(name) => write!(f, "{:?}", name),
            Self::Generic(family) => write!(f, "{}", family),
        }
    }
}

#[derive(Clone)]
struct ParseList<'a> {
    source: &'a [u8],
    len: usize,
    pos: usize,
}

impl<'a> Iterator for ParseList<'a> {
    type Item = FontFamily<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut quote = None;
        let mut pos = self.pos;
        while pos < self.len && {
            let ch = self.source[pos];
            ch.is_ascii_whitespace() || ch == b','
        } {
            pos += 1;
        }
        self.pos = pos;
        if pos >= self.len {
            return None;
        }
        let first = self.source[pos];
        let mut start = pos;
        match first {
            b'"' | b'\'' => {
                quote = Some(first);
                pos += 1;
                start += 1;
            }
            _ => {}
        }
        if let Some(quote) = quote {
            while pos < self.len {
                if self.source[pos] == quote {
                    self.pos = pos + 1;
                    return Some(FontFamily::Named(
                        core::str::from_utf8(self.source.get(start..pos)?)
                            .ok()?
                            .trim(),
                    ));
                }
                pos += 1;
            }
            self.pos = pos;
            return Some(FontFamily::Named(
                core::str::from_utf8(self.source.get(start..pos)?)
                    .ok()?
                    .trim(),
            ));
        }
        let mut end = start;
        while pos < self.len {
            if self.source[pos] == b',' {
                pos += 1;
                break;
            }
            pos += 1;
            end += 1;
        }
        self.pos = pos;
        let name = core::str::from_utf8(self.source.get(start..end)?)
            .ok()?
            .trim();
        Some(match GenericFamily::parse(name) {
            Some(family) => FontFamily::Generic(family),
            _ => FontFamily::Named(name),
        })
    }
}

/// Font settings that can be supplied as a raw source string or
/// a parsed slice.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FontSettings<'a, T> {
    /// Setting source in CSS format.
    Source(&'a str),
    /// List of settings.
    List(&'a [T]),
}

impl<'a, T> From<&'a str> for FontSettings<'a, T> {
    fn from(value: &'a str) -> Self {
        Self::Source(value)
    }
}

impl<'a, T> From<&'a [T]> for FontSettings<'a, T> {
    fn from(value: &'a [T]) -> Self {
        Self::List(value)
    }
}
