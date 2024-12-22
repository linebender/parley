// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout types.

pub(crate) mod alignment;
pub(crate) mod cluster;
pub(crate) mod layout;
pub(crate) mod line;
pub(crate) mod run;

pub mod cursor;
pub mod editor;

#[cfg(feature = "accesskit")]
mod accessibility;

#[cfg(feature = "accesskit")]
pub use accessibility::LayoutAccessibility;

pub use self::alignment::Alignment;
pub use self::cluster::{Affinity, Cluster, ClusterPath, ClusterSide};
pub use self::cursor::{Cursor, Selection};
pub use self::layout::Layout;
pub use self::line::{GlyphRun, Line, PositionedInlineBox, PositionedLayoutItem};
pub use self::run::{Run, RunMetrics};

pub(crate) use self::alignment::align;
pub(crate) use self::cluster::ClusterData;
pub(crate) use self::layout::LayoutData;
pub(crate) use self::line::{LayoutItem, LayoutItemKind, LineData, LineItemData};
pub(crate) use self::run::RunData;

use crate::inputs::Brush;
use swash::GlyphId;

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum BreakReason {
    #[default]
    None,
    Regular,
    Explicit,
    Emergency,
}

/// Glyph with an offset and advance.
#[derive(Copy, Clone, Default, Debug)]
pub struct Glyph {
    pub id: GlyphId,
    pub style_index: u16,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

impl Glyph {
    /// Returns the index into the layout style collection.
    pub fn style_index(&self) -> usize {
        self.style_index as usize
    }
}

#[allow(clippy::partial_pub_fields)]
/// Style properties.
#[derive(Clone, Debug)]
pub struct Style<B: Brush> {
    /// Brush for drawing glyphs.
    pub brush: B,
    /// Underline decoration.
    pub underline: Option<Decoration<B>>,
    /// Strikethrough decoration.
    pub strikethrough: Option<Decoration<B>>,
    /// Absolute line height in layout units (style line height * font size)
    pub(crate) line_height: f32,
}

/// Underline or strikethrough decoration.
#[derive(Clone, Debug)]
pub struct Decoration<B: Brush> {
    /// Brush used to draw the decoration.
    pub brush: B,
    /// Offset of the decoration from the baseline. If `None`, use the metrics
    /// of the containing run.
    pub offset: Option<f32>,
    /// Thickness of the decoration. If `None`, use the metrics of the
    /// containing run.
    pub size: Option<f32>,
}
