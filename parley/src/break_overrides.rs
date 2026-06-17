// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Overrides for line-break opportunities.

use alloc::boxed::Box;

/// Line break opportunity override.
///
/// Called for each adjacent pair of Unicode code points.
///
/// Returning:
///    - `Some(true)`  : forces a line break opportunity between the pair
///    - `Some(false)` : suppresses any opportunity
///    - `None`        : defers to the default (ICU) behavior
///
/// Mandatory breaks are unaffected (e.g. `\n`).
///
/// This is typically used to force preferential line breaking decisions when it
/// comes to ASCII punctuation like `/`, `-`, etc. For example, to prevent break
/// opportunities within "1/2".
///
/// A separate use case is to match the line breaking behavior of existing systems
/// such as web browsers. See [`CHROMIUM_LINE_BREAK_TABLE`] for a ready-made
/// table that mirrors Chromium's behavior.
pub type LineBreakOverrideFn = dyn Fn(char, char) -> Option<bool> + Send + Sync;

/// A static table mirroring Chromium's preferred line breaking behavior.
///
/// See: <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/text/character_property_data_generator.cc;l=449-495>
///
/// # Differences from other browsers
///
/// ## Compared to Safari
///
/// Chromium (and this table) differs from Safari in 9 cases:
///  - A "-" followed by one of "!|$|)|/|:|;|?|]|}" is suppressed in Chromium, but broken in Safari.
///
/// ## Compared to Firefox
///
/// Firefox always defers to the default ICU behavior.
pub static CHROMIUM_LINE_BREAK_TABLE: AsciiLineBreakTable = AsciiLineBreakTable::chromium();

/// A line break override table for ASCII character pairs.
///
/// See [`LineBreakOverrideFn`] for more details.
///
/// All table operations are `const`, which means that [`CHROMIUM_LINE_BREAK_TABLE`]
/// needs no explicit construction step.
///
/// # Example
///
/// ```
/// # use parley::{AsciiLineBreakTable, CHROMIUM_LINE_BREAK_TABLE, FontContext, LayoutContext};
/// # let mut font_cx = FontContext::default();
/// # let mut layout_cx: LayoutContext<[u8; 4]> = LayoutContext::new();
/// let text = "Hello there!";
/// let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, true);
/// // Emulate Chromium:
/// builder.set_line_break_override(Some(CHROMIUM_LINE_BREAK_TABLE.as_override()));
/// let layout = builder.build(text);
/// # let _ = layout;
/// ```
#[derive(Clone)]
pub struct AsciiLineBreakTable {
    /// Bit `after` set in row `before` means the pair has an explicit override.
    overridden: [u128; 128],
    /// Bit `after` set in row `before` means a break is allowed for that pair.
    allow: [u128; 128],
}

impl AsciiLineBreakTable {
    /// A table that defers every pair to the default ICU behavior.
    pub const fn new() -> Self {
        Self {
            overridden: [0; 128],
            allow: [0; 128],
        }
    }

    /// Override every pair in the cartesian product of the two inclusive ASCII
    /// ranges.
    pub const fn with_pairs(
        mut self,
        before_lo: char,
        before_hi: char,
        after_lo: char,
        after_hi: char,
        allow_break: bool,
    ) -> Self {
        let mut before = before_lo as u32;
        while before <= before_hi as u32 {
            let mut after = after_lo as u32;
            while after <= after_hi as u32 {
                if before < 128 && after < 128 {
                    let bit = 1_u128 << after;
                    self.overridden[before as usize] |= bit;
                    if allow_break {
                        self.allow[before as usize] |= bit;
                    } else {
                        self.allow[before as usize] &= !bit;
                    }
                }
                after += 1;
            }
            before += 1;
        }
        self
    }

    /// Look up the override for a `(before, after)` pair.
    ///
    /// Return semantics copied from [`LineBreakOverrideFn`].
    pub const fn lookup(&self, before: char, after: char) -> Option<bool> {
        let (b, a) = (before as u32, after as u32);
        if b >= 128 || a >= 128 || self.overridden[b as usize] & (1_u128 << a) == 0 {
            return None;
        }
        Some(self.allow[b as usize] & (1_u128 << a) != 0)
    }

    /// Convert into a [`LineBreakOverrideFn`] closure.
    ///
    /// Prefer to use [`AsciiLineBreakTable::as_override`] for static tables.
    pub fn into_override(self) -> Box<LineBreakOverrideFn> {
        Box::new(move |before, after| self.lookup(before, after))
    }

    /// Convert into a [`LineBreakOverrideFn`] closure.
    ///
    /// Prefer to use this for static tables, like [`CHROMIUM_LINE_BREAK_TABLE`] to avoid the ~4 kB
    /// allocation overhead for each boxing.
    pub fn as_override(&'static self) -> Box<LineBreakOverrideFn> {
        Box::new(move |before, after| self.lookup(before, after))
    }

    /// See [`CHROMIUM_LINE_BREAK_TABLE`] for more details.
    const fn chromium() -> Self {
        // The printable ASCII range `'!'..=0x7F`.
        const ALL_LO: char = '!';
        const ALL_HI: char = '\u{7f}';

        Self::new()
            .with_pairs(ALL_LO, ALL_HI, ALL_LO, ALL_HI, false)
            .with_pairs(ALL_LO, ALL_HI, '(', '(', true)
            .with_pairs(ALL_LO, ALL_HI, '<', '<', true)
            .with_pairs(ALL_LO, ALL_HI, '[', '[', true)
            .with_pairs(ALL_LO, ALL_HI, '{', '{', true)
            .with_pairs('-', '-', ALL_LO, ALL_HI, true)
            .with_pairs('?', '?', ALL_LO, ALL_HI, true)
            .with_pairs('-', '-', '$', '$', false)
            .with_pairs(ALL_LO, ALL_HI, '!', '!', false)
            .with_pairs('?', '?', '"', '"', false)
            .with_pairs('?', '?', '\'', '\'', false)
            .with_pairs(ALL_LO, ALL_HI, ')', ')', false)
            .with_pairs(ALL_LO, ALL_HI, ',', ',', false)
            .with_pairs(ALL_LO, ALL_HI, '.', '.', false)
            .with_pairs(ALL_LO, ALL_HI, '/', '/', false)
            .with_pairs('-', '-', '0', '9', false)
            .with_pairs(ALL_LO, ALL_HI, ':', ':', false)
            .with_pairs(ALL_LO, ALL_HI, ';', ';', false)
            .with_pairs(ALL_LO, ALL_HI, '?', '?', false)
            .with_pairs(ALL_LO, ALL_HI, ']', ']', false)
            .with_pairs(ALL_LO, ALL_HI, '}', '}', false)
            .with_pairs('$', '$', ALL_LO, ALL_HI, false)
            .with_pairs('\'', '\'', ALL_LO, ALL_HI, false)
            .with_pairs('(', '(', ALL_LO, ALL_HI, false)
            .with_pairs('/', '/', ALL_LO, ALL_HI, false)
            .with_pairs('0', '9', ALL_LO, ALL_HI, false)
            .with_pairs('<', '<', ALL_LO, ALL_HI, false)
            .with_pairs('@', '@', ALL_LO, ALL_HI, false)
            .with_pairs('A', 'Z', ALL_LO, ALL_HI, false)
            .with_pairs('[', '[', ALL_LO, ALL_HI, false)
            .with_pairs('^', '`', ALL_LO, ALL_HI, false)
            .with_pairs('a', 'z', ALL_LO, ALL_HI, false)
            .with_pairs('{', '{', ALL_LO, ALL_HI, false)
            .with_pairs('\u{7f}', '\u{7f}', ALL_LO, ALL_HI, false)
    }
}

impl Default for AsciiLineBreakTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::AsciiLineBreakTable;
    use super::CHROMIUM_LINE_BREAK_TABLE;

    #[test]
    fn chromium_suppresses_ascii_punctuation_breaks() {
        let t = &CHROMIUM_LINE_BREAK_TABLE;
        // No break before or after a slash.
        assert_eq!(t.lookup('a', '/'), Some(false));
        assert_eq!(t.lookup('/', 'b'), Some(false));
        // No break around other punctuation.
        assert_eq!(t.lookup('a', '.'), Some(false));
        assert_eq!(t.lookup('a', ':'), Some(false));
        // No break between letters.
        assert_eq!(t.lookup('a', 'b'), Some(false));
    }

    #[test]
    fn chromium_allows_some_breaks() {
        let t = &CHROMIUM_LINE_BREAK_TABLE;
        // Break allowed before opening punctuation, but only when the preceding
        // character's row was not later suppressed.
        assert_eq!(t.lookup(')', '('), Some(true));
        assert_eq!(t.lookup(')', '<'), Some(true));
        assert_eq!(t.lookup('a', '('), Some(false));
        // Break allowed after '-' and '?' except when...
        assert_eq!(t.lookup('-', 'b'), Some(true));
        assert_eq!(t.lookup('?', 'b'), Some(true));
        // ...break not allowed after '-' before a digit, or '?' before a quote.
        assert_eq!(t.lookup('-', '5'), Some(false));
        assert_eq!(t.lookup('?', '"'), Some(false));
    }

    #[test]
    fn non_ascii_pairs_defer_to_icu() {
        let t = &CHROMIUM_LINE_BREAK_TABLE;
        assert_eq!(t.lookup('a', 'é'), None);
        assert_eq!(t.lookup('é', 'a'), None);
        // Space (0x20) is below the printable range, so it defers too.
        assert_eq!(t.lookup('a', ' '), None);
    }

    #[test]
    fn empty_table_defers_to_icu() {
        let t = AsciiLineBreakTable::new();
        assert_eq!(t.lookup('a', '/'), None);
        assert_eq!(t.lookup('a', '('), None);
    }
}
