// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt::Debug;

use attributed_text::TextStorage;

use crate::text::StyledText;

/// The kind of a [`Block`] in a [`StyledDocument`](crate::StyledDocument).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// A normal paragraph of text.
    Paragraph,
    /// A heading block with the given level.
    Heading {
        /// Heading level, typically `1..=6`.
        level: u8,
    },
    /// A list item block.
    ListItem {
        /// Whether this list item belongs to an ordered list.
        ordered: bool,
    },
    /// A block quote.
    BlockQuote,
    /// A code block.
    CodeBlock,
}

/// A semantic block in a [`StyledDocument`](crate::StyledDocument).
#[derive(Debug)]
pub struct Block<T: Debug + TextStorage, A: Debug> {
    /// The semantic kind for this block.
    pub kind: BlockKind,
    /// Nesting depth (for list items, quotes, etc.).
    pub nesting: u16,
    /// The block's styled text content.
    pub text: StyledText<T, A>,
}
