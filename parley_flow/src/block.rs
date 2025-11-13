// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text blocks and a simple adapter for Parley [`parley::layout::Layout`].

use alloc::string::String;
use core::ops::Range;

use parley::layout::Layout;
use parley::style::Brush;

/// A uniform facade for anything that behaves like a text layout block.
///
/// Implementers must provide:
/// - A stable identifier (`id`).
/// - A layout reference for geometry and hit-testing.
/// - Text access for extraction via [`TextBlock::text_slice`] and/or
///   [`TextBlock::read_text`].
///
/// Geometry and ordering are defined by `FlowItem` rectangles in a `TextFlow`; this
/// trait does not carry positional information.
pub trait TextBlock<B: Brush> {
    /// Identifier type used by this surface set.
    type Id: Copy + Ord + Eq + core::fmt::Debug;

    /// Stable identifier for the surface.
    fn id(&self) -> Self::Id;

    /// The underlying Parley layout for this surface.
    ///
    /// This is typically a [`parley::layout::Layout`] built by your code.
    fn layout(&self) -> &Layout<B>;

    /// Return a borrowed text slice for a local byte `range`, if valid and contiguous.
    ///
    /// This is the fast path used when the underlying storage is contiguous. Implementers that
    /// do not have contiguous storage (e.g., ropes) can return `None` and instead implement
    /// [`TextBlock::read_text`]. The `range` must be on UTF‑8 character boundaries.
    fn text_slice(&self, _range: Range<usize>) -> Option<&str> {
        None
    }

    /// Append the text in the local byte `range` into `out` and return `true` if successful.
    ///
    /// This is the fallback used by helpers like [`crate::copy_text`] to support non‑contiguous
    /// storage. The default implementation tries [`TextBlock::text_slice`] and, if present,
    /// pushes it into `out`.
    fn read_text(&self, range: Range<usize>, out: &mut String) -> bool {
        if let Some(s) = self.text_slice(range) {
            out.push_str(s);
            true
        } else {
            false
        }
    }
}

/// A simple adapter turning a Parley [`parley::layout::Layout`] and source text into a
/// [`TextBlock`].
///
/// Construct this when you already have a `Layout` and the `&str` it was built from, then
/// pass it to helpers like [`crate::hit_test`], [`crate::selection_geometry`], and
/// [`crate::copy_text`].
#[derive(Copy, Clone)]
pub struct LayoutBlock<'a, B: Brush, Id> {
    /// Stable identifier for the surface.
    pub id: Id,
    /// The layout to expose.
    pub layout: &'a Layout<B>,
    /// The source text used to build `layout`.
    pub text: &'a str,
}

impl<'a, B: Brush, Id: Copy + Ord + Eq + core::fmt::Debug> TextBlock<B> for LayoutBlock<'a, B, Id> {
    type Id = Id;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn layout(&self) -> &Layout<B> {
        self.layout
    }

    fn text_slice(&self, range: Range<usize>) -> Option<&str> {
        self.text.get(range)
    }

    fn read_text(&self, range: Range<usize>, out: &mut String) -> bool {
        if let Some(slice) = self.text.get(range) {
            out.push_str(slice);
            true
        } else {
            false
        }
    }
}

impl<'a, B: Brush, Id: Copy + Ord + Eq + core::fmt::Debug> core::fmt::Debug
    for LayoutBlock<'a, B, Id>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LayoutBlock")
            .field("id", &self.id)
            .field("text_len", &self.text.len())
            .finish_non_exhaustive()
    }
}
