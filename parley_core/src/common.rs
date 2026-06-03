// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Common types used across analysis and shaping.

/// A single normalized variation coordinate, in the range `[-1, 1]` mapped to `i16` fixed point
/// (`[-0x4000, 0x4000]`), as produced by `skrifa`/`harfrust`.
///
/// A run carries one coordinate per variation axis of its font; see
/// [`crate::Run::normalized_coords`].
pub type NormalizedCoord = i16;

/// How a paragraph's text is written: horizontal lines, or vertical lines with a given glyph
/// orientation.
///
/// This is similar to CSS's `writing-mode` property, see
/// <https://www.w3.org/TR/css-writing-modes-3/#block-flow>. It captures the inline axis and, when
/// vertical, the [`TextOrientation`]. It does not capture block flow (`vertical-rl` vs
/// `vertical-lr`), which determines how lines stack and is a layout concern.
///
/// This is resolved to a per-run [`RunOrientation`], which the shaper acts on (and a renderer
/// should also act on).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Hash)]
pub enum WritingMode {
    /// Horizontal lines with glyphs upright, advancing along the horizontal axis.
    #[default]
    Horizontal,
    /// Vertical lines; the orientation of glyphs within each line is given by [`TextOrientation`].
    Vertical(TextOrientation),
}

impl WritingMode {
    /// Returns whether this writing mode is [`WritingMode::Vertical`].
    #[inline]
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Vertical(_))
    }

    /// Resolves to a single [`RunOrientation`] for all except
    /// [Vertical(Mixed)](`TextOrientation::Mixed`).
    ///
    /// `Vertical(Mixed)` is resolved per-character using the Unicode `Vertical_Orientation`
    /// property, see <https://www.unicode.org/reports/tr50/tr50-15.html>.
    #[inline]
    pub fn uniform_orientation(self) -> Option<RunOrientation> {
        match self {
            Self::Horizontal => Some(RunOrientation::Horizontal),
            Self::Vertical(TextOrientation::Upright) => Some(RunOrientation::Upright),
            Self::Vertical(TextOrientation::Sideways) => Some(RunOrientation::Sideways),
            Self::Vertical(TextOrientation::Mixed) => None,
        }
    }

    /// Whether this writing mode forces a left-to-right inline direction and suppresses bidi
    /// reordering.
    ///
    /// This is the case for [`TextOrientation::Upright`], see
    /// <https://www.w3.org/TR/css-writing-modes-4/#valdef-text-orientation-upright>.
    #[inline]
    pub fn suppresses_bidi(self) -> bool {
        matches!(self, Self::Vertical(TextOrientation::Upright))
    }
}

/// How glyphs are oriented within [`WritingMode::Vertical`].
///
/// Mirrors the CSS property of the same name. See
/// <https://www.w3.org/TR/css-writing-modes-4/#text-orientation>.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Hash)]
pub enum TextOrientation {
    /// Vertical scripts are set upright; horizontal scripts (Latin, digits, ...) are rotated 90°
    /// clockwise.
    ///
    /// Resolved via UTR #50, see <https://www.unicode.org/reports/tr50/tr50-15.html>.
    #[default]
    Mixed,
    /// All glyphs are set upright. Forces the inline direction to left-to-right, suppressing bidi
    /// reordering.
    Upright,
    /// All glyphs are shaped horizontally and the whole line is then rotated 90° clockwise.
    Sideways,
}

/// How a single run's glyphs sit on the line.
///
/// This is the per-run resolution of a [`WritingMode`].
///
/// For [`TextOrientation::Mixed`], the text is segmented into [`Self::Upright`] and
/// [`Self::Sideways`] during analysis.
///
/// Glyph [`x`](crate::Glyph::x)/[`y`](crate::Glyph::y) offsets are relative to the pen position
/// regardless of orientation.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Hash)]
pub enum RunOrientation {
    /// Upright, advancing along the horizontal axis. The only orientation in a horizontal writing
    /// mode.
    #[default]
    Horizontal,
    /// Upright, advancing along the vertical axis (vertical metrics and the font's vertical
    /// typesetting features apply).
    Upright,
    /// Shaped in the horizontal frame, then rotated 90° into a vertical line by the layout layer.
    Sideways,
}

impl RunOrientation {
    /// Whether the shaper lays this run out along the vertical axis.
    ///
    /// Only [`Self::Upright`] shapes vertically; [`Horizontal`](Self::Horizontal) and
    /// [`Sideways`](Self::Sideways) are shaped horizontally, with a `Sideways` run rotated
    /// afterwards.
    #[inline]
    pub fn is_vertical_shaping(self) -> bool {
        matches!(self, Self::Upright)
    }
}

/// The resolved inline direction of a run, derived from its bidi level.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Direction {
    /// Left-to-right (even bidi level).
    LeftToRight,
    /// Right-to-left (odd bidi level).
    RightToLeft,
}

impl Direction {
    /// Returns the direction implied by a bidi embedding `level`, even levels being LTR, odd levels
    /// RTL.
    #[inline]
    pub fn from_bidi_level(level: u8) -> Self {
        if level & 1 == 0 {
            Self::LeftToRight
        } else {
            Self::RightToLeft
        }
    }

    /// Returns `true` for [`Direction::RightToLeft`].
    #[inline]
    pub fn is_rtl(self) -> bool {
        self == Self::RightToLeft
    }
}

/// The strongest segmentation boundary occurring immediately *before* a character or cluster.
///
/// Boundaries are derived during [analysis](crate::Analyzer) from the Unicode segmentation
/// algorithms (UAX #14 line breaking, UAX #29 word boundaries) and copied onto each
/// [`Cluster`](crate::Cluster) so that line breakers operate on the shaped output alone. The
/// variants are ordered by strength.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Boundary {
    /// Not a boundary.
    None = 0,
    /// A word boundary (UAX #29), but not a permitted line break.
    Word = 1,
    /// A permitted (optional) line break opportunity (UAX #14).
    Line = 2,
    /// A mandatory line break (e.g. after `U+000A`, or other forced breaks).
    Mandatory = 3,
}

impl Boundary {
    /// Returns `true` if a line may break before this cluster.
    ///
    /// If [`Self::is_mandatory`] is also `true`, a line *must* break before this cluster.
    #[inline]
    pub fn is_line_break(self) -> bool {
        matches!(self, Self::Line | Self::Mandatory)
    }

    /// Returns `true` if a line *must* break before this cluster.
    #[inline]
    pub fn is_mandatory(self) -> bool {
        self == Self::Mandatory
    }
}

/// A coarse classification of a cluster's whitespace.
///
/// This is sufficient for line breaking, trailing-whitespace handling, and justification.
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Whitespace {
    /// Not a space.
    None = 0,
    /// Standard space (`U+0020`).
    ///
    /// This is a candidate for justification expansion.
    Space = 1,
    /// A non-breaking space (`U+00A0`): whitespace that does not allow a break.
    NoBreakSpace = 2,
    /// Horizontal tab (`U+0009`).
    Tab = 3,
    /// Newline (CR, LF, CRLF, LS, or PS).
    Newline = 4,
}

impl Whitespace {
    /// Returns true for space or non-breaking space.
    #[inline]
    pub fn is_space_or_nbsp(self) -> bool {
        matches!(self, Self::Space | Self::NoBreakSpace)
    }

    /// Returns `true` if this whitespace may be expanded during justification.
    #[inline]
    pub fn is_stretchable(self) -> bool {
        self == Self::Space
    }
}

/// Metrics and decoration geometry for a run.
#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct RunMetrics {
    /// Typographic ascent: distance from the baseline to the top of the line.
    pub ascent: f32,
    /// Typographic descent: distance from the baseline to the bottom of the line.
    pub descent: f32,
    /// Typographic leading: recommended extra spacing between lines.
    pub leading: f32,
    /// Offset of the top of underline decoration from the baseline.
    pub underline_offset: f32,
    /// Thickness of the underline decoration.
    pub underline_size: f32,
    /// Offset of the top of strikethrough decoration from the baseline.
    pub strikethrough_offset: f32,
    /// Thickness of the strikethrough decoration.
    pub strikethrough_size: f32,
    /// Distance from the baseline to the top of short lowercase letters.
    pub x_height: Option<f32>,
    /// Distance from the baseline to the top of capital letters.
    pub cap_height: Option<f32>,
}
