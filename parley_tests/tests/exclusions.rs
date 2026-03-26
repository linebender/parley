// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod circle {
    use crate::util::TestEnv;
    use crate::{test_name, util::ColorBrush};
    use parley::{Alignment, AlignmentOptions, Layout, LineHeight, StyleProperty, YieldData};
    use peniko::kurbo::Size;

    #[test]
    fn custom_break_lines_circle_layout() {
        let mut env = TestEnv::new(test_name!(), None);
        *env.max_screenshot_size() = Some(20 * 1024);
        let text = "Curving text is easier when Parley lets us steer every line. ".repeat(8);
        let text = &text[..&text.len() - 7];

        let font_size = 10.0;
        let line_height = font_size * 1.2;
        let diameter = 180.0;

        let mut builder = env.ranged_builder(text);
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(line_height)));

        let mut layout = builder.build(text);

        apply_circle_breaking(&mut layout, diameter, line_height);

        layout.align(Alignment::Justify, AlignmentOptions::default());

        env.rendering_config()
            .size
            .replace(Size::new(diameter as f64, diameter as f64));
        env.check_layout_snapshot(&layout);
    }

    const MIN_LINE_WIDTH: f32 = 24.0;

    fn apply_circle_breaking(layout: &mut Layout<ColorBrush>, diameter: f32, line_height: f32) {
        let mut breaker = layout.break_lines();

        let (line_x, line_width) = circle_band_for_y(diameter, 0.0, line_height);

        let state = breaker.state_mut();
        state.set_layout_max_advance(diameter);
        state.set_line_x(line_x);
        state.set_line_max_advance(line_width.max(MIN_LINE_WIDTH));

        while let Some(data) = breaker.break_next() {
            match data {
                YieldData::LineBreak(line_break) => {
                    let (line_x, line_width) =
                        circle_band_for_y(diameter, line_break.line_y_end as f32, line_height);
                    let state = breaker.state_mut();
                    state.set_line_x(line_x);
                    state.set_line_max_advance(line_width.max(MIN_LINE_WIDTH));
                }
                YieldData::InlineBoxBreak(_) => {}
                YieldData::MaxHeightExceeded(data) => {
                    panic!("Unexpected max-height break at {}", data.line_height);
                }
            }
        }
    }

    fn circle_band_for_y(diameter: f32, line_top: f32, line_height: f32) -> (f32, f32) {
        let radius = diameter * 0.5;
        let band_center = line_top + line_height * 0.5;
        let dy = (band_center - radius).abs();

        if line_height <= 0.0 || band_center >= diameter || dy >= radius {
            return (0.0, diameter);
        }

        let half_width = (radius * radius - dy * dy).max(0.0).sqrt();
        let left = radius - half_width;
        let width = (half_width * 2.0).max(MIN_LINE_WIDTH);

        (left, width.min(diameter - left))
    }
}
