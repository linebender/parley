// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout types.

#[cfg(feature = "accesskit")]
mod accessibility;
mod alignment;
mod cluster;
mod glyph;
mod line;
mod line_break;
mod run;

// TODO - Add to allowed lint set?
#[expect(
    clippy::module_inception,
    reason = "Private inner module for code organisation"
)]
mod layout;

pub(crate) mod data;

#[cfg(feature = "accesskit")]
pub use accessibility::LayoutAccessibility;
pub use alignment::{Alignment, AlignmentOptions};
pub use cluster::{Affinity, Cluster, ClusterPath, ClusterSide};
pub use data::BreakReason;
pub use glyph::Glyph;
pub use layout::Layout;
pub use line::{GlyphRun, Line, LineMetrics, PositionedInlineBox, PositionedLayoutItem};
pub use line_break::{BoxBreakData, BreakLines, BreakerState, LineBreakData, YieldData};
pub use run::{Run, RunMetrics};

pub(crate) use data::{LayoutData, LayoutItem, LayoutItemKind, LineData, LineItemData, RunData};
pub(crate) use line::LineItem;

// TODO - Deprecation not yet active to ease internal code migration.
#[deprecated(since = "TBD", note = "Access from the `editing` module instead.")]
pub use crate::editing::{Cursor, Selection};

// TODO - Move the following to `style` module and submodules.
use crate::OverflowWrap;
use crate::style::Brush;

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum LayoutLineHeight {
    MetricsRelative(f32),
    Absolute(f32),
}

impl LayoutLineHeight {
    pub(crate) fn resolve(self, run: &RunData) -> f32 {
        match self {
            Self::MetricsRelative(value) => {
                (run.metrics.ascent + run.metrics.descent + run.metrics.leading) * value
            }
            Self::Absolute(value) => value,
        }
    }
}

#[allow(clippy::partial_pub_fields)]
/// Style properties.
#[derive(Clone, Debug, PartialEq)]
pub struct Style<B: Brush> {
    /// Brush for drawing glyphs.
    pub brush: B,
    /// Underline decoration.
    pub underline: Option<Decoration<B>>,
    /// Strikethrough decoration.
    pub strikethrough: Option<Decoration<B>>,
    /// Partially resolved line height, either in in layout units or dependent on metrics
    pub(crate) line_height: LayoutLineHeight,
    /// Per-cluster overflow-wrap setting
    pub(crate) overflow_wrap: OverflowWrap,
}

/// Underline or strikethrough decoration.
#[derive(Clone, Debug, PartialEq)]
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

/// Lower and upper bounds on layout width based on its contents.
#[derive(Copy, Clone, Debug)]
pub struct ContentWidths {
    /// The minimum content width. This is the width of the layout if _all_ soft line-breaking
    /// opportunities are taken.
    pub min: f32,
    /// The maximum content width. This is the width of the layout if _no_ soft line-breaking
    /// opportunities are taken.
    pub max: f32,
}
