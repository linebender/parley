// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt::Debug;

use crate::{ComputedInlineStyle, ComputedParagraphStyle};
use attributed_text::TextStorage;

use crate::block::Block;

/// A flat sequence of semantic blocks.
///
/// This is intended to carry semantic structure (headings, lists, etc.) suitable for accessibility,
/// even before layout provides geometry.
#[derive(Debug)]
pub struct StyledDocument<T: Debug + TextStorage, A: Debug> {
    root_inline: ComputedInlineStyle,
    root_paragraph: ComputedParagraphStyle,
    blocks: Vec<Block<T, A>>,
}

impl<T: Debug + TextStorage, A: Debug> Default for StyledDocument<T, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug + TextStorage, A: Debug> StyledDocument<T, A> {
    /// Creates an empty document.
    #[inline]
    pub fn new() -> Self {
        Self {
            root_inline: ComputedInlineStyle::default(),
            root_paragraph: ComputedParagraphStyle::default(),
            blocks: Vec::new(),
        }
    }

    /// Creates an empty document with explicit root styles.
    ///
    /// Root styles are used for root-relative units such as `rem`.
    #[inline]
    pub fn new_with_root(
        root_inline: ComputedInlineStyle,
        root_paragraph: ComputedParagraphStyle,
    ) -> Self {
        Self {
            root_inline,
            root_paragraph,
            blocks: Vec::new(),
        }
    }

    /// Sets the root styles for this document, updating all existing blocks.
    pub fn set_root_styles(
        &mut self,
        root_inline: ComputedInlineStyle,
        root_paragraph: ComputedParagraphStyle,
    ) {
        self.root_inline = root_inline;
        self.root_paragraph = root_paragraph;
        for block in &mut self.blocks {
            block
                .text
                .set_root_styles(self.root_inline.clone(), self.root_paragraph.clone());
        }
    }

    /// Returns the root inline style.
    #[inline]
    pub fn root_inline_style(&self) -> &ComputedInlineStyle {
        &self.root_inline
    }

    /// Returns the root paragraph style.
    #[inline]
    pub fn root_paragraph_style(&self) -> &ComputedParagraphStyle {
        &self.root_paragraph
    }

    /// Appends a block to the document.
    ///
    /// The document's current root styles are applied to the block.
    pub fn push(&mut self, mut block: Block<T, A>) {
        block
            .text
            .set_root_styles(self.root_inline.clone(), self.root_paragraph.clone());
        self.blocks.push(block);
    }

    /// Appends a block to the document without modifying its root styles.
    #[inline]
    pub fn push_raw(&mut self, block: Block<T, A>) {
        self.blocks.push(block);
    }

    /// Returns an iterator over blocks.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Block<T, A>> {
        self.blocks.iter()
    }

    /// Returns the number of blocks.
    #[inline]
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Returns `true` if there are no blocks.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}
