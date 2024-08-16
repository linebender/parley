// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use parley::{layout::cursor::Selection, layout::PositionedLayoutItem, FontContext};
use peniko::{
    kurbo::{Affine, Stroke},
    Color, Fill,
};
use vello::Scene;
use winit::{
    event::{Modifiers, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

type LayoutContext = parley::LayoutContext<Color>;
type Layout = parley::Layout<Color>;

const INSET: f32 = 32.0;

#[derive(Copy, Clone, Debug)]
pub enum ActiveText<'a> {
    FocusedCluster(&'a str),
    Selection(&'a str)
}

#[derive(Default)]
pub struct Text {
    font_cx: FontContext,
    layout_cx: LayoutContext,
    buffer: String,
    layout: Layout,
    selection: Selection,
    pointer_down: bool,
    cursor_pos: (f32, f32),
    modifiers: Option<Modifiers>,
    width: f32,
}

impl Text {
    pub fn set_text(&mut self, text: &str) {
        self.buffer.clear();
        self.buffer.push_str(text);
    }

    pub fn update_layout(&mut self, width: f32, scale: f32) {
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, &self.buffer, scale);
        builder.push_default(&parley::style::StyleProperty::FontSize(32.0));
        builder.push_default(&parley::style::StyleProperty::LineHeight(1.2));
        builder.push_default(&parley::style::StyleProperty::FontStack(parley::style::FontStack::Source("system-ui")));
        builder.build_into(&mut self.layout);
        self.layout
            .break_all_lines(Some(width - INSET * 2.0), parley::layout::Alignment::Start);
        self.width = width;
    }

    pub fn active_text(&self) -> ActiveText {
        if self.selection.is_collapsed() {
            let range =  self.selection.focus().text_start..self.selection.focus().text_end;
            ActiveText::FocusedCluster(&self.buffer[range])
        } else {
            ActiveText::Selection(&self.buffer[self.selection.text_range()])
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = Some(*modifiers);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.state.is_pressed() {
                    return;
                }
                let shift = self
                    .modifiers
                    .map(|mods| mods.state().shift_key())
                    .unwrap_or_default();
                match event.physical_key {
                    PhysicalKey::Code(code) => match code {
                        KeyCode::ArrowLeft => {
                            self.selection = self.selection.prev_logical(&self.layout, shift);
                        }
                        KeyCode::ArrowRight => {
                            self.selection = self.selection.next_logical(&self.layout, shift);
                        }
                        KeyCode::ArrowUp => {
                            self.selection = self.selection.prev_line(&self.layout, shift);
                        }
                        KeyCode::ArrowDown => {
                            self.selection = self.selection.next_line(&self.layout, shift);
                        }
                        KeyCode::Delete => {
                            let range = if self.selection.is_collapsed() {
                                self.selection.focus().text_start..self.selection.focus().text_end
                            } else {
                                self.selection.text_range()
                            };
                            let start = range.start;
                            self.buffer.replace_range(range, "");
                            self.selection = self.selection.collapse();
                            self.update_layout(self.width, 1.0);
                            self.selection = Selection::from_byte_index(&self.layout, start);
                        }
                        _ => {}
                    },
                    _ => {}
                }
                println!("Active text: {:?}", self.active_text());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == winit::event::MouseButton::Left {
                    self.pointer_down = state.is_pressed();
                    if self.pointer_down {
                        self.selection = Selection::from_point(
                            &self.layout,
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                        );
                        println!("Active text: {:?}", self.active_text());
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = (position.x as f32 - INSET, position.y as f32 - INSET);
                if self.pointer_down {
                    self.selection = self.selection.extend_to_point(
                        &self.layout,
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                    );
                    println!("Active text: {:?}", self.active_text());
                }
            }
            _ => {}
        }
    }

    pub fn draw(&self, scene: &mut Scene) {
        let transform = Affine::translate((INSET as f64, INSET as f64));
        self.selection.visual_regions_with(&self.layout, |rect| {
            scene.fill(Fill::NonZero, transform, Color::STEEL_BLUE, None, &rect);
        });
        if let Some(cursor) = self.selection.visual_caret(&self.layout) {
            scene.stroke(&Stroke::new(1.5), transform, Color::WHITE, None, &cursor);
        };
        for line in self.layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let synthesis = run.synthesis();
                let glyph_xform = synthesis
                    .skew()
                    .map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));
                let style = glyph_run.style();
                let coords = run
                    .normalized_coords()
                    .iter()
                    .map(|coord| vello::skrifa::instance::NormalizedCoord::from_bits(*coord))
                    .collect::<Vec<_>>();
                scene
                    .draw_glyphs(font)
                    .brush(Color::WHITE)
                    .hint(true)
                    .transform(transform)
                    .glyph_transform(glyph_xform)
                    .font_size(font_size)
                    .normalized_coords(&coords)
                    .draw(
                        Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let gx = x + glyph.x;
                            let gy = y - glyph.y;
                            x += glyph.advance;
                            vello::glyph::Glyph {
                                id: glyph.id as _,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
            }
        }
    }
}

pub const LOREM: &str = r"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi cursus mi sed euismod euismod. Orci varius natoque penatibus et magnis dis parturient montes, nascetur ridiculus mus. Nullam placerat efficitur tellus at semper. Morbi ac risus magna. Donec ut cursus ex. Etiam quis posuere tellus. Mauris posuere dui et turpis mollis, vitae luctus tellus consectetur. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Curabitur eu facilisis nisl.

Phasellus in viverra dolor, vitae facilisis est. Maecenas malesuada massa vel ultricies feugiat. Vivamus venenatis et התעשייה בנושא האינטרנט nibh nec pharetra. Phasellus vestibulum elit enim, nec scelerisque orci faucibus id. Vivamus consequat purus sit amet orci egestas, non iaculis massa porttitor. Vestibulum ut eros leo. In fermentum convallis magna in finibus. Donec justo leo, maximus ac laoreet id, volutpat ut elit. Mauris sed leo non neque laoreet faucibus. Aliquam orci arcu, faucibus in molestie eget, ornare non dui. Donec volutpat nulla in fringilla elementum. Aliquam vitae ante egestas ligula tempus vestibulum sit amet sed ante. ";
