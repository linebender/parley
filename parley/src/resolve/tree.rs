// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hierarchical tree based style application.
use alloc::borrow::Cow;
use alloc::{string::String, vec::Vec};

use crate::style::WhiteSpaceCollapse;

use super::{Brush, RangedStyle, ResolvedProperty, ResolvedStyle};

#[derive(Debug, Clone)]
struct StyleTreeNode<B: Brush> {
    parent: Option<usize>,
    style: ResolvedStyle<B>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ItemKind {
    None,
    InlineBox,
    TextRun,
}

/// Builder for constructing a tree of styles
#[derive(Clone)]
pub(crate) struct TreeStyleBuilder<B: Brush> {
    tree: Vec<StyleTreeNode<B>>,
    flatted_styles: Vec<RangedStyle<B>>,
    white_space_collapse: WhiteSpaceCollapse,
    text: String,
    uncommitted_text: String,
    current_span: usize,
    is_span_first: bool,
    last_item_kind: ItemKind,
}

impl<B: Brush> TreeStyleBuilder<B> {
    fn current_style(&self) -> ResolvedStyle<B> {
        self.tree[self.current_span].style.clone()
    }
}

impl<B: Brush> Default for TreeStyleBuilder<B> {
    fn default() -> Self {
        Self {
            tree: Vec::new(),
            flatted_styles: Vec::new(),
            white_space_collapse: WhiteSpaceCollapse::Preserve,
            text: String::new(),
            uncommitted_text: String::new(),
            current_span: usize::MAX,
            is_span_first: false,
            last_item_kind: ItemKind::None,
        }
    }
}

impl<B: Brush> TreeStyleBuilder<B> {
    /// Prepares the builder for accepting a tree of styles and text.
    ///
    /// The provided `root_style` is the default style applied to all text unless overridden.
    pub(crate) fn begin(&mut self, root_style: ResolvedStyle<B>) {
        self.tree.clear();
        self.flatted_styles.clear();
        self.white_space_collapse = WhiteSpaceCollapse::Preserve;
        self.text.clear();
        self.uncommitted_text.clear();

        self.tree.push(StyleTreeNode {
            parent: None,
            style: root_style,
        });
        self.current_span = 0;
        self.is_span_first = true;
    }

    pub(crate) fn set_white_space_mode(&mut self, white_space_collapse: WhiteSpaceCollapse) {
        self.white_space_collapse = white_space_collapse;
    }

    pub(crate) fn set_is_span_first(&mut self, is_span_first: bool) {
        self.is_span_first = is_span_first;
    }

    pub(crate) fn set_last_item_kind(&mut self, item_kind: ItemKind) {
        self.last_item_kind = item_kind;
    }

    pub(crate) fn push_uncommitted_text(&mut self, is_span_last: bool) {
        let span_text: Cow<'_, str> = match self.white_space_collapse {
            WhiteSpaceCollapse::Preserve => Cow::from(&self.uncommitted_text),
            WhiteSpaceCollapse::Collapse => {
                let mut span_text = self.uncommitted_text.as_str();

                if self.is_span_first
                    || (self.last_item_kind == ItemKind::TextRun
                        && self
                            .text
                            .chars()
                            .last()
                            .is_some_and(|c| c.is_ascii_whitespace()))
                {
                    span_text = span_text.trim_start();
                }
                if is_span_last {
                    span_text = span_text.trim_end();
                }

                // Collapse spaces
                let mut last_char_whitespace = false;
                let span_text: String = span_text
                    .chars()
                    .filter_map(|c: char| {
                        let this_char_whitespace = c.is_ascii_whitespace();
                        let prev_char_whitespace = last_char_whitespace;
                        last_char_whitespace = this_char_whitespace;

                        if this_char_whitespace {
                            if prev_char_whitespace {
                                None
                            } else {
                                Some(' ')
                            }
                        } else {
                            Some(c)
                        }
                    })
                    .collect();

                Cow::from(span_text)
            }
        };
        let span_text = span_text.as_ref();

        // Nothing to do if there is no uncommitted text.
        if span_text.is_empty() {
            self.uncommitted_text.clear();
            return;
        }

        let range = self.text.len()..(self.text.len() + span_text.len());
        let style = self.current_style();
        self.flatted_styles.push(RangedStyle { style, range });
        self.text.push_str(span_text);
        self.uncommitted_text.clear();
        self.is_span_first = false;
        self.last_item_kind = ItemKind::TextRun;
    }

    pub(crate) fn current_text_len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn push_style_span(&mut self, style: ResolvedStyle<B>) {
        self.push_uncommitted_text(false);

        self.tree.push(StyleTreeNode {
            parent: Some(self.current_span),
            style,
        });
        self.current_span = self.tree.len() - 1;
        self.is_span_first = true;
    }

    pub(crate) fn push_style_modification_span(
        &mut self,
        properties: impl Iterator<Item = ResolvedProperty<B>>,
    ) {
        let mut style = self.current_style();
        for prop in properties {
            style.apply(prop.clone());
        }
        self.push_style_span(style);
    }

    pub(crate) fn pop_style_span(&mut self) {
        self.push_uncommitted_text(true);

        self.current_span = self.tree[self.current_span]
            .parent
            .expect("Popped root style");
    }

    /// Pushes a property that covers the specified range of text.
    pub(crate) fn push_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.uncommitted_text.push_str(text);
        }
    }

    /// Computes the sequence of ranged styles.
    pub(crate) fn finish(&mut self, styles: &mut Vec<RangedStyle<B>>) -> String {
        while self.tree[self.current_span].parent.is_some() {
            self.pop_style_span();
        }

        self.push_uncommitted_text(true);

        styles.clear();
        styles.extend_from_slice(&self.flatted_styles);

        core::mem::take(&mut self.text)
    }
}
