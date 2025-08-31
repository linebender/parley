// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Model for a font.

use crate::CharmapIndex;

use super::attributes::{FontStyle, FontWeight, FontWidth};
use super::source::{SourceInfo, SourceKind};
use super::{Blob, source_cache::SourceCache};
use core::fmt;
use read_fonts::{FontRef, TableProvider as _, types::Tag};
use smallvec::SmallVec;

type AxisVec = SmallVec<[AxisInfo; 1]>;

/// Representation of a single font in a family.
#[derive(Clone, Debug)]
pub struct FontInfo {
    source: SourceInfo,
    index: u32,
    width: FontWidth,
    style: FontStyle,
    weight: FontWeight,
    axes: AxisVec,
    attr_axes: u8,
    charmap_index: CharmapIndex,
}

impl FontInfo {
    /// Creates a new font object from the given source and index.
    pub fn from_source(source: SourceInfo, index: u32) -> Option<Self> {
        match &source.kind {
            #[cfg(feature = "std")]
            SourceKind::Path(path) => {
                let file = std::fs::File::open(&**path).ok()?;
                let mapped = unsafe { memmap2::Mmap::map(&file).ok()? };
                let font = FontRef::from_index(&mapped, index).ok()?;
                Self::from_font_ref(&font, source.clone(), index)
            }
            SourceKind::Memory(memory) => {
                let font = FontRef::from_index(memory.as_ref(), index).ok()?;
                Self::from_font_ref(&font, source.clone(), index)
            }
        }
    }

    /// Returns an object describing how to locate the data containing this
    /// font.
    pub fn source(&self) -> &SourceInfo {
        &self.source
    }

    /// Returns the index of the font in a collection.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Attempts to load the font, optionally from a source cache.
    pub fn load(&self, source_cache: Option<&mut SourceCache>) -> Option<Blob<u8>> {
        if let Some(source_cache) = source_cache {
            source_cache.get(&self.source)
        } else {
            match &self.source.kind {
                SourceKind::Memory(blob) => Some(blob.clone()),
                #[cfg(feature = "std")]
                SourceKind::Path(path) => super::source_cache::load_blob(path),
            }
        }
    }

    /// Returns the visual width of the font-- a relative change from the normal
    /// aspect ratio, typically in the range `0.5` to `2.0`.
    pub fn width(&self) -> FontWidth {
        self.width
    }

    /// Returns the visual style or 'slope' of the font.
    pub fn style(&self) -> FontStyle {
        self.style
    }

    /// Returns the visual weight class of the font, typically on a scale
    /// from `1.0` to `1000.0`.
    pub fn weight(&self) -> FontWeight {
        self.weight
    }

    /// Returns synthesis suggestions for this font with the given attributes.
    pub fn synthesis(&self, width: FontWidth, style: FontStyle, weight: FontWeight) -> Synthesis {
        let mut synth = Synthesis::default();
        let mut len = 0_usize;
        if self.has_width_axis() && self.width != width {
            synth.vars[len] = (Tag::new(b"wdth"), width.percentage());
            len += 1;
        }
        if self.weight != weight {
            if self.has_weight_axis() {
                synth.vars[len] = (Tag::new(b"wght"), weight.value());
                len += 1;
            } else if weight.value() > self.weight.value() {
                synth.embolden = true;
            }
        }
        if self.style != style {
            match style {
                FontStyle::Normal => {}
                FontStyle::Italic => {
                    if self.style == FontStyle::Normal {
                        if self.has_italic_axis() {
                            synth.vars[len] = (Tag::new(b"ital"), 1.0);
                            len += 1;
                        } else if self.has_slant_axis() {
                            synth.vars[len] = (Tag::new(b"slnt"), 14.0);
                            len += 1;
                        } else {
                            synth.skew = 14;
                        }
                    }
                }
                FontStyle::Oblique(angle) => {
                    if self.style == FontStyle::Normal {
                        let degrees = angle.unwrap_or(14.0);
                        if self.has_slant_axis() {
                            synth.vars[len] = (Tag::new(b"slnt"), degrees);
                            len += 1;
                        } else if self.has_italic_axis() && degrees > 0. {
                            synth.vars[len] = (Tag::new(b"ital"), 1.0);
                            len += 1;
                        } else {
                            synth.skew = degrees as i8;
                        }
                    }
                }
            }
        }
        synth.len = len as u8;
        synth
    }

    /// Returns the variation [axes] for the font.
    ///
    /// [axes]: crate::AxisInfo
    pub fn axes(&self) -> &[AxisInfo] {
        &self.axes
    }

    /// Returns `true` if the font has a `wght` [axis].
    ///
    /// This is a quicker check than scanning the [axes].
    ///
    /// [axes]: Self::axes
    /// [axis]: crate::AxisInfo
    pub fn has_weight_axis(&self) -> bool {
        self.attr_axes & WEIGHT_AXIS != 0
    }

    /// Returns `true` if the font has a `wdth` [axis].
    ///
    /// This is a quicker check than scanning the [axes].
    ///
    /// [axes]: Self::axes
    /// [axis]: crate::AxisInfo
    pub fn has_width_axis(&self) -> bool {
        self.attr_axes & WIDTH_AXIS != 0
    }

    /// Returns `true` if the font has a `slnt` [axis].
    ///
    /// This is a quicker check than scanning the [axes].
    ///
    /// [axes]: Self::axes
    /// [axis]: crate::AxisInfo
    pub fn has_slant_axis(&self) -> bool {
        self.attr_axes & SLANT_AXIS != 0
    }

    /// Returns `true` if the font has an `ital` [axis].
    ///
    /// This is a quicker check than scanning the [axes].
    ///
    /// [axes]: Self::axes
    /// [axis]: crate::AxisInfo
    pub fn has_italic_axis(&self) -> bool {
        self.attr_axes & ITALIC_AXIS != 0
    }

    /// Returns `true` if the font as an `opsz` [axis].
    ///
    /// This is a quicker check than scanning the [axes].
    ///
    /// [axes]: Self::axes
    /// [axis]: crate::AxisInfo
    pub fn has_optical_size_axis(&self) -> bool {
        self.attr_axes & OPTICAL_SIZE_AXIS != 0
    }

    /// Returns the index used for constructing a [Charmap] for this font.
    ///
    /// [Charmap]: crate::Charmap
    pub fn charmap_index(&self) -> CharmapIndex {
        self.charmap_index
    }
}

impl FontInfo {
    pub(crate) fn from_font_ref(
        font: &FontRef<'_>,
        source: SourceInfo,
        index: u32,
    ) -> Option<Self> {
        // It's probably not useful to retain fonts that don't have
        // a valid cmap so just bail here if we fail.
        let charmap_index = CharmapIndex::new(font)?;
        let (width, style, weight) = read_attributes(font);
        let (axes, attr_axes) = if let Ok(fvar_axes) = font.fvar().and_then(|fvar| fvar.axes()) {
            let mut axes = SmallVec::<[AxisInfo; 1]>::with_capacity(fvar_axes.len());
            let mut attrs_axes = 0_u8;
            for fvar_axis in fvar_axes {
                let axis = AxisInfo {
                    tag: fvar_axis.axis_tag(),
                    min: fvar_axis.min_value().to_f32(),
                    max: fvar_axis.max_value().to_f32(),
                    default: fvar_axis.default_value().to_f32(),
                };
                axes.push(axis);
                match &axis.tag.to_be_bytes() {
                    b"wght" => attrs_axes |= WEIGHT_AXIS,
                    b"wdth" => attrs_axes |= WIDTH_AXIS,
                    b"slnt" => attrs_axes |= SLANT_AXIS,
                    b"ital" => attrs_axes |= ITALIC_AXIS,
                    b"opsz" => attrs_axes |= OPTICAL_SIZE_AXIS,
                    _ => {}
                }
            }
            (axes, attrs_axes)
        } else {
            (Default::default(), Default::default())
        };
        Some(Self {
            source,
            index,
            width,
            style,
            weight,
            axes,
            attr_axes,
            charmap_index,
        })
    }

    #[allow(unused)]
    pub(crate) fn maybe_override_attributes(
        &mut self,
        width: FontWidth,
        style: FontStyle,
        weight: FontWeight,
    ) {
        if self.width == FontWidth::default() {
            self.width = width;
        }
        if self.style == FontStyle::default() {
            self.style = style;
        }
        if self.weight == FontWeight::default() {
            self.weight = weight;
        }
    }

    pub(crate) fn apply_override(&mut self, info_override: &FontInfoOverride<'_>) {
        if let Some(width) = info_override.width {
            self.width = width;
        }
        if let Some(style) = info_override.style {
            self.style = style;
        }
        if let Some(weight) = info_override.weight {
            self.weight = weight;
        }
        if let Some(axes) = info_override.axes {
            // This is O(n^2) but no font should have enough axes for that to
            // matter (read-fonts does the same thing)
            for (tag, value) in axes {
                if let Some(axis) = self.axes.iter_mut().find(|axis| axis.tag == *tag) {
                    axis.default = *value;
                }
            }
        }
    }
}

const WEIGHT_AXIS: u8 = 0x01;
const WIDTH_AXIS: u8 = 0x02;
const SLANT_AXIS: u8 = 0x04;
const ITALIC_AXIS: u8 = 0x08;
const OPTICAL_SIZE_AXIS: u8 = 0x10;

/// An axis of variation for a variable font.
///
/// Instances of this can be obtained from [`FontInfo::axes`].
///
/// These give the [`Tag`] and range of valid values for a given font
/// variation. In `parley`, these values are used to create a
/// `FontVariation`.
///
/// OpenType defines some common axes:
///
/// * [Italic](https://fonts.google.com/knowledge/glossary/italic_axis) or `ital`
/// * [Optical Size](https://fonts.google.com/knowledge/glossary/optical_size_axis) or `opsz`
/// * [Slant](https://fonts.google.com/knowledge/glossary/slant_axis) or `slnt`
/// * [FontWeight](https://fonts.google.com/knowledge/glossary/weight_axis) or `wght`
/// * [Width](https://fonts.google.com/knowledge/glossary/width_axis) or `wdth`
///
/// For a broader explanation of this, see
/// [Axis in Variable Fonts](https://fonts.google.com/knowledge/glossary/axis_in_variable_fonts)
/// from Google Fonts.
#[derive(Copy, Clone, Default, Debug)]
pub struct AxisInfo {
    /// The tag that identifies the axis.
    pub tag: Tag,
    /// The inclusive minimum value of the axis.
    pub min: f32,
    /// The inclusive maximum value of the axis.
    pub max: f32,
    /// The default value of the axis.
    pub default: f32,
}

/// Suggestions for synthesizing a set of font attributes for a given
/// font.
///
/// Instances of this can be obtained from [`FontInfo::synthesis`]
/// as well as [`QueryFont::synthesis`].
///
/// [`QueryFont::synthesis`]: crate::QueryFont::synthesis
#[derive(Copy, Clone, Default, PartialEq)]
pub struct Synthesis {
    vars: [(Tag, f32); 3],
    len: u8,
    embolden: bool,
    skew: i8,
}

impl Synthesis {
    /// Returns `true` if any synthesis suggestions are available.
    pub fn any(&self) -> bool {
        self.len != 0 || self.embolden || self.skew != 0
    }

    /// Returns the variation settings that should be applied to match the
    /// requested attributes.
    ///
    /// When using `parley`, these can be used to create `FontVariation`
    /// settings.
    pub fn variation_settings(&self) -> &[(Tag, f32)] {
        &self.vars[..self.len as usize]
    }

    /// Returns `true` if the scaler should apply a faux bold.
    pub fn embolden(&self) -> bool {
        self.embolden
    }

    /// Returns a skew angle for faux italic/oblique, if requested.
    pub fn skew(&self) -> Option<f32> {
        if self.skew != 0 {
            Some(self.skew as f32)
        } else {
            None
        }
    }
}

#[allow(clippy::missing_fields_in_debug)]
impl fmt::Debug for Synthesis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Synthesis")
            .field("vars", &self.variation_settings())
            .field("embolden", &self.embolden)
            .field("skew", &self.skew)
            .finish()
    }
}

fn read_attributes(font: &FontRef<'_>) -> (FontWidth, FontStyle, FontWeight) {
    use read_fonts::{
        TableProvider,
        tables::{
            head::{Head, MacStyle},
            os2::{Os2, SelectionFlags},
            post::Post,
        },
    };

    fn width_from_width_class(width_class: u16) -> FontWidth {
        FontWidth::from_ratio(match width_class {
            0..=1 => 0.5,
            2 => 0.625,
            3 => 0.75,
            4 => 0.875,
            5 => 1.0,
            6 => 1.125,
            7 => 1.25,
            8 => 1.5,
            _ => 2.0,
        })
    }

    fn from_os2_post(os2: Os2<'_>, post: Option<Post<'_>>) -> (FontWidth, FontStyle, FontWeight) {
        let width = width_from_width_class(os2.us_width_class());
        // Bits 1 and 9 of the fsSelection field signify italic and
        // oblique, respectively.
        // See: <https://learn.microsoft.com/en-us/typography/opentype/spec/os2#fsselection>
        let fs_selection = os2.fs_selection();
        let style = if fs_selection.contains(SelectionFlags::ITALIC) {
            FontStyle::Italic
        } else if fs_selection.contains(SelectionFlags::OBLIQUE) {
            let angle = post.map(|post| post.italic_angle().to_f64() as f32);
            FontStyle::Oblique(angle)
        } else {
            FontStyle::Normal
        };
        // The usFontWeightClass field is specified with a 1-1000 range, but
        // we don't clamp here because variable fonts could potentially
        // have a value outside of that range.
        // See <https://learn.microsoft.com/en-us/typography/opentype/spec/os2#usweightclass>
        let weight = FontWeight::new(os2.us_weight_class() as f32);
        (width, style, weight)
    }

    fn from_head(head: Head<'_>) -> (FontWidth, FontStyle, FontWeight) {
        let mac_style = head.mac_style();
        let style = mac_style
            .contains(MacStyle::ITALIC)
            .then_some(FontStyle::Italic)
            .unwrap_or_default();
        let weight = mac_style
            .contains(MacStyle::BOLD)
            .then_some(700.0)
            .unwrap_or_default();
        (FontWidth::default(), style, FontWeight::new(weight))
    }

    if let Ok(os2) = font.os2() {
        // Prefer values from the OS/2 table if it exists. We also use
        // the post table to extract the angle for oblique styles.
        from_os2_post(os2, font.post().ok())
    } else if let Ok(head) = font.head() {
        // Otherwise, fall back to the macStyle field of the head table.
        from_head(head)
    } else {
        (
            FontWidth::default(),
            FontStyle::Normal,
            FontWeight::default(),
        )
    }
}

/// Helper for specifying aspects of a font's metadata to be overridden when the
/// font is registered. Helpful when implementing a `@font-face`-like API, which
/// allows those defining the fonts to specify certain font properties manually.
#[derive(Copy, Clone, Debug, Default)]
pub struct FontInfoOverride<'a> {
    /// Font family name to be used instead of the one specified in the font
    /// itself.
    pub family_name: Option<&'a str>,
    /// Font width to be used instead of the one specified in the font itself.
    pub width: Option<FontWidth>,
    /// Font's visual style / "slope" to be used instead of the one specified in
    /// the font itself.
    pub style: Option<FontStyle>,
    /// Font weight to be used instead of the one specified in the font itself.
    pub weight: Option<FontWeight>,
    /// Default values for the font's variation axes. Axes not included within
    /// the font will be ignored.
    pub axes: Option<&'a [(Tag, f32)]>,
}
