// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use super::utils::{
    ColorBrush,
    fonts::{FONT_FAMILY_LIST, create_font_context},
};
use crate::{Alignment, AlignmentOptions, FontFamily, IndentOptions, Layout, LayoutContext};

#[test]
fn clear_resets_to_new() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();

    static TEXT: &str = "Some text that will wrap across several lines.";

    let mut builder = lcx.ranged_builder(&mut fcx, TEXT, 1., false);
    builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
    let mut layout = builder.build(TEXT);
    layout.set_text_indent(10., IndentOptions::default());
    layout.break_all_lines(Some(50.));
    layout.align(Alignment::Center, AlignmentOptions::default());
    assert!(layout.lines().len() > 1);
    assert!(layout.width() > 0.);
    assert!(layout.height() > 0.);

    layout.clear();

    assert_eq!(layout.data, Layout::new().data);
}

/// A layout of `text` with tab stops every 40 layout units.
fn tabbed_layout(text: &str) -> Layout<ColorBrush> {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.ranged_builder(&mut fcx, text, 1., false);
    builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
    builder.push_default(crate::StyleProperty::TabSize(crate::TabSize::new(40.)));
    builder.build(text)
}

/// The tab advances recorded for each line, as `(cluster, advance)`.
fn tab_advances_per_line(layout: &Layout<ColorBrush>) -> Vec<Vec<(u32, f32)>> {
    layout
        .data
        .lines
        .iter()
        .map(|line| {
            layout.data.tab_advances
                [line.tab_advances.start as usize..line.tab_advances.end as usize]
                .iter()
                .map(|tab| (tab.cluster, tab.advance))
                .collect()
        })
        .collect()
}

/// Guards against a tab's advance surviving the backtrack that moves the tab to the next line.
/// Source: an invariant of the side table, each line owns exactly the entries of its own tabs.
#[test]
fn tab_advances_dropped_when_backtracking_before_a_tab() {
    // "aaa aaa" fits and the tab after it hangs, so the breaker places the tab before it finds
    // that "!!!" doesn't fit and backtracks to the break after the space (there's no break
    // opportunity before "!", so none right after the tab). The tab's first advance must go with
    // it; only its advance on the second line remains, and each line owns its own.
    let text = "aaa aaa\t!!!";
    let mut layout = tabbed_layout(text);
    let full_width = {
        layout.break_all_lines(None);
        assert_eq!(tab_advances_per_line(&layout).len(), 1);
        assert_eq!(tab_advances_per_line(&layout)[0].len(), 1);
        layout.full_width()
    };
    layout.break_all_lines(Some(full_width - 1.));
    assert_eq!(layout.len(), 2);
    assert_eq!(&text[layout.get(1).unwrap().text_range()], "aaa\t!!!");
    let per_line = tab_advances_per_line(&layout);
    assert!(per_line[0].is_empty(), "{per_line:?}");
    assert_eq!(per_line[1].len(), 1, "{per_line:?}");
    assert_eq!(layout.data.tab_advances.len(), 1);
    // The tab starts after "aaa" at the line start, so it reaches the first stop: the second
    // line is one tab size plus "!!!" wide.
    let bangs = tabbed_layout("!!!").calculate_content_widths().max;
    let second_line = layout.get(1).unwrap().metrics().advance;
    assert!(
        (second_line - bangs - 40.).abs() < 1e-3,
        "{second_line} - {bangs}"
    );
}

/// Guards against `BreakLines::revert` keeping the reverted line's tab advances. Source: the
/// same invariant.
#[test]
fn tab_advances_dropped_when_reverting_a_line() {
    let text = "a\tb\nc\td\ne\tf";
    let mut layout = tabbed_layout(text);
    {
        let mut breaker = layout.break_lines();
        // `break_lines` starts with no width at all, so give the lines room: without this, the
        // breaker wraps after every tab (a tab is a break opportunity) and the lines come out as
        // "a\t", "b\n", "c\t", ... rather than one line per newline.
        let state = breaker.state_mut();
        state.set_layout_max_advance(f32::MAX);
        state.set_line_max_advance(f32::MAX);
        breaker.break_next().unwrap();
        breaker.break_next().unwrap();
        assert!(breaker.revert());
        breaker.break_next().unwrap();
        breaker.break_next().unwrap();
        breaker.finish();
    }
    assert_eq!(layout.len(), 3);
    let per_line = tab_advances_per_line(&layout);
    assert_eq!(per_line.iter().map(Vec::len).collect::<Vec<_>>(), [1, 1, 1]);
    assert_eq!(layout.data.tab_advances.len(), 3);
    let clusters: Vec<_> = layout
        .data
        .tab_advances
        .iter()
        .map(|tab| tab.cluster)
        .collect();
    assert!(
        clusters.windows(2).all(|pair| pair[0] < pair[1]),
        "{clusters:?}"
    );
}

/// Guards against tabs with no advance filling the side table. Source: `push_tab_advance`'s
/// contract, a missing entry reads as zero.
#[test]
fn tabs_without_an_advance_get_no_entry() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let text = "a\tb\tc";
    let mut builder = lcx.ranged_builder(&mut fcx, text, 1., false);
    builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    assert!(layout.data.tab_advances.is_empty());
    assert!(layout.data.lines[0].tab_advances.is_empty());
}

/// Guards against `TabSize` not being scaled like other lengths, or the default skip threshold
/// being resolved before scaling. Source: `FontSize` and `LetterSpacing`, which are scaled.
#[test]
fn tab_size_scales_like_other_lengths() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let text = "a\tb";
    for scale in [1., 2.] {
        let mut builder = lcx.ranged_builder(&mut fcx, text, scale, false);
        builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
        builder.push_default(crate::StyleProperty::TabSize(crate::TabSize::new(40.)));
        let layout = builder.build(text);
        assert_eq!(layout.data.styles[0].tab_size, 40. * scale);
        // The default skip threshold is half the root font size (16), which is already scaled.
        assert_eq!(layout.data.styles[0].tab_skip_threshold, 8. * scale);

        let mut builder = lcx.ranged_builder(&mut fcx, text, scale, false);
        builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
        builder.push_default(crate::StyleProperty::TabSize(
            crate::TabSize::new(40.).with_skip_threshold(3.),
        ));
        let layout = builder.build(text);
        assert_eq!(layout.data.styles[0].tab_skip_threshold, 3. * scale);
    }
}
