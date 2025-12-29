// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::InlineStyle;

/// Extracts the inline style declarations from a span attribute.
///
/// This enables wrapper attribute types to carry additional semantic information (links, custom
/// annotations, etc.) while still allowing `styled_text` to resolve inline styles.
pub trait HasInlineStyle {
    /// Returns the inline style attached to this span.
    fn inline_style(&self) -> &InlineStyle;
}

impl HasInlineStyle for InlineStyle {
    fn inline_style(&self) -> &InlineStyle {
        self
    }
}
