// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Affinity, Cursor, FontContext, FontStack, Layout, LayoutContext};

/// Helper struct for creating cursors and checking their values.
///
/// This type implements multiple assertion methods which, on failure, will
/// print the input text with cursor's expected and actual positions highlighted.
/// This should make test failures more readable than printing the cursor's byte index.
///
/// The following are not supported:
///
/// - RTL text.
/// - Multi-line text.
/// - Any character that doesn't span a single terminal tile.
/// - Multi-bytes characters.
///
/// Some of these limitations are inherent to visually displaying a text layout in the
/// terminal.
///
/// Others will be fixed in the future.
pub struct CursorTest {
    text: String,
    layout: Layout<()>,
}

impl CursorTest {
    pub fn single_line(text: &str, lcx: &mut LayoutContext<()>, fcx: &mut FontContext) -> Self {
        let mut builder = lcx.ranged_builder(fcx, text, 1.0);
        builder.push(
            FontStack::Single(crate::FontFamily::Generic(
                fontique::GenericFamily::Monospace,
            )),
            ..,
        );
        let mut layout = builder.build(text);
        layout.break_all_lines(None);

        Self {
            text: text.to_string(),
            layout,
        }
    }

    /// Returns the text that was used to create the layout.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the layout that was created from the text.
    pub fn layout(&self) -> &Layout<()> {
        &self.layout
    }

    #[track_caller]
    fn get_unique_index(&self, method_name: &str, needle: &str) -> usize {
        let Some(index) = self.text.find(needle) else {
            panic!(
                "Error in {method_name}: needle '{needle}' not found in text '{}'",
                self.text
            );
        };
        dbg!(index);
        if self.text[index + needle.len()..].find(needle).is_some() {
            panic!(
                "Error in {method_name}: needle '{needle}' found multiple times in text '{}'",
                self.text
            );
        }
        index
    }

    /// Returns a cursor that points to the first character of the needle, with
    /// [`Affinity::Downstream`].
    ///
    /// The needle must be unique in the text to avoid ambiguity.
    ///
    /// ### Panics
    ///
    /// - If the needle is not found in the text.
    /// - If the needle is found multiple times in the text.
    #[track_caller]
    pub fn cursor_before(&self, needle: &str) -> Cursor {
        let index = self.get_unique_index("cursor_before", needle);
        Cursor::from_byte_index(&self.layout, index, Affinity::Downstream)
    }

    /// Returns a cursor that points to the first character after the needle, with
    /// [`Affinity::Upstream`].
    ///
    /// The needle must be unique in the text to avoid ambiguity.
    ///
    /// ### Panics
    ///
    /// - If the needle is not found in the text.
    /// - If the needle is found multiple times in the text.
    #[track_caller]
    pub fn cursor_after(&self, needle: &str) -> Cursor {
        let index = self.get_unique_index("cursor_after", needle);
        let index = index + needle.len();
        Cursor::from_byte_index(&self.layout, index, Affinity::Upstream)
    }

    fn cursor_to_monospace(&self, cursor: Cursor, is_correct: bool) -> String {
        fn check_no_color() -> bool {
            let Some(env_var) = std::env::var_os("NO_COLOR") else {
                return false;
            };
            let env_var = env_var.to_str().unwrap_or_default().trim();

            if env_var == "0" {
                return false;
            }
            if env_var.to_ascii_lowercase() == "false" {
                return false;
            }
            true
        }

        // NOTE: The background color doesn't carry important information,
        // so we do a simple implementation, without worrying about
        // color-blindness and platform issues.
        let ansi_bg_color = if cfg!(not(unix)) || check_no_color() {
            ""
        } else if is_correct {
            // Green background
            "\x1b[48;5;70m"
        } else {
            // Red background
            "\x1b[48;5;160m"
        };
        let ansi_reset = if cfg!(not(unix)) { "" } else { "\x1b[0m" };
        let index = cursor.index();
        let affinity = cursor.affinity();

        let cursor_str = if affinity == Affinity::Upstream {
            // - ANSI code for 'Set background color'
            // - Unicode sequence for '▕' character
            // - ANSI code for 'Reset all attributes'
            format!("{ansi_bg_color}\u{2595}{ansi_reset}")
        } else {
            // - 1 space
            // - ANSI code for 'Set background color'
            // - Unicode sequence for '▏' character
            // - ANSI code for 'Reset all attributes'
            format!(" {ansi_bg_color}\u{258F}{ansi_reset}")
        };

        " ".repeat(index) + &cursor_str
    }

    #[track_caller]
    fn cursor_assertion(&self, expected: Cursor, actual: Cursor) {
        if expected == actual {
            return;
        }

        // TODO - Check that the tested string doesn't include difficult
        // characters (newlines, tabs, RTL text, etc.)
        // If it does, we should still print the text on a best effort basis, but
        // without visual cursors and with a warning that the text may not be accurate.

        panic!(
            concat!(
                "cursor assertion failed\n",
                "  expected: '{text}' - ({expected_index}, {expected_affinity})\n",
                "            {expected_cursor}\n",
                "       got: '{text}' - ({actual_index}, {actual_affinity})\n",
                "            {actual_cursor}\n",
            ),
            text = self.text,
            expected_index = expected.index(),
            expected_affinity = expected.affinity(),
            actual_index = actual.index(),
            actual_affinity = actual.affinity(),
            expected_cursor = self.cursor_to_monospace(expected, true),
            actual_cursor = self.cursor_to_monospace(actual, false),
        );
    }

    /// Asserts that the cursor is before the needle.
    ///
    /// The needle must be unique in the text to avoid ambiguity.
    ///
    /// ### Panics
    ///
    /// - If the needle is not found in the text.
    /// - If the needle is found multiple times in the text.
    /// - If the cursor has the wrong position.
    /// - If the cursor doesn't have [`Affinity::Downstream`].
    #[track_caller]
    pub fn assert_cursor_is_before(&self, needle: &str, cursor: Cursor) {
        let index = self.get_unique_index("assert_cursor_is_before", needle);

        let expected_cursor = Cursor::from_byte_index(&self.layout, index, Affinity::Downstream);
        self.cursor_assertion(expected_cursor, cursor);
    }

    /// Asserts that the cursor is after the needle.
    ///
    /// The needle must be unique in the text to avoid ambiguity.
    ///
    /// ### Panics
    ///
    /// - If the needle is not found in the text.
    /// - If the needle is found multiple times in the text.
    /// - If the cursor has the wrong position.
    /// - If the cursor doesn't have [`Affinity::Upstream`].
    #[track_caller]
    pub fn assert_cursor_is_after(&self, needle: &str, cursor: Cursor) {
        let index = self.get_unique_index("assert_cursor_is_after", needle);
        let index = index + needle.len();

        let expected_cursor = Cursor::from_byte_index(&self.layout, index, Affinity::Upstream);
        self.cursor_assertion(expected_cursor, cursor);
    }

    /// Compares two cursors and asserts that they are the same.
    ///
    /// ### Panics
    ///
    /// - If the cursors don't have the same index.
    /// - If the cursors don't have the same affinity.
    #[track_caller]
    pub fn assert_cursor_is(&self, expected: Cursor, cursor: Cursor) {
        self.cursor_assertion(expected, cursor);
    }

    /// Prints the TestLayout's text, with the cursor highlighted.
    ///
    /// Uses the same format as assertion failures.
    #[track_caller]
    pub fn print_cursor(&self, cursor: Cursor) {
        eprintln!(
            concat!(
                "dumping test layout value\n",
                "text: '{text}' - ({actual_index}, {actual_affinity})\n",
                "      {actual_cursor}\n",
            ),
            text = self.text,
            actual_index = cursor.index(),
            actual_affinity = cursor.affinity(),
            actual_cursor = self.cursor_to_monospace(cursor, true),
        );
    }
}

// ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_next_visual() {
        let (mut lcx, mut fcx) = (LayoutContext::new(), FontContext::new());
        let text = "Lorem ipsum dolor sit amet";
        let layout = CursorTest::single_line(text, &mut lcx, &mut fcx);

        let mut cursor: Cursor = layout.cursor_before("dolor");
        layout.print_cursor(cursor);
        cursor = cursor.next_visual(&layout.layout);

        layout.assert_cursor_is_after("ipsum d", cursor);
    }
}
