// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use clipboard::ClipboardProvider;
use parley::layout::cursor::{Selection, VisualMode};
use parley::layout::Affinity;
use parley::{layout::PositionedLayoutItem, FontContext};
use peniko::{kurbo::Affine, Color, Fill};
use std::time::Instant;
use vello::Scene;
use winit::{
    event::{Modifiers, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

type LayoutContext = parley::LayoutContext<Color>;
type Layout = parley::Layout<Color>;

const INSET: f32 = 32.0;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum ActiveText<'a> {
    FocusedCluster(Affinity, &'a str),
    Selection(&'a str),
}

#[derive(Default)]
pub struct Editor {
    font_cx: FontContext,
    layout_cx: LayoutContext,
    buffer: String,
    layout: Layout,
    selection: Selection,
    cursor_mode: VisualMode,
    last_click_time: Option<Instant>,
    click_count: u32,
    pointer_down: bool,
    cursor_pos: (f32, f32),
    modifiers: Option<Modifiers>,
    width: f32,
}

impl Editor {
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
        builder.push_default(&parley::style::StyleProperty::FontStack(
            parley::style::FontStack::Source("system-ui"),
        ));
        builder.build_into(&mut self.layout);
        self.layout.break_all_lines(Some(width - INSET * 2.0));
        self.layout
            .align(Some(width - INSET * 2.0), parley::layout::Alignment::Start);
        self.width = width;
    }

    pub fn active_text(&self) -> ActiveText {
        if self.selection.is_collapsed() {
            let range = self
                .selection
                .focus()
                .cluster_path()
                .cluster(&self.layout)
                .unwrap()
                .text_range();
            ActiveText::FocusedCluster(self.selection.focus().affinity(), &self.buffer[range])
        } else {
            ActiveText::Selection(&self.buffer[self.selection.text_range()])
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.update_layout(size.width as f32, 1.0);
                self.selection = self.selection.refresh(&self.layout);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = Some(*modifiers);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.state.is_pressed() {
                    return;
                }
                #[allow(unused)]
                let (shift, ctrl, cmd) = self
                    .modifiers
                    .map(|mods| {
                        (
                            mods.state().shift_key(),
                            mods.state().control_key(),
                            mods.state().super_key(),
                        )
                    })
                    .unwrap_or_default();
                #[cfg(target_os = "macos")]
                let action_mod = cmd;
                #[cfg(not(target_os = "macos"))]
                let action_mod = ctrl;
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyC if action_mod => {
                            if !self.selection.is_collapsed() {
                                let text = &self.buffer[self.selection.text_range()];
                                let mut cb: clipboard::ClipboardContext =
                                    ClipboardProvider::new().unwrap();
                                cb.set_contents(text.to_owned()).ok();
                            }
                        }
                        KeyCode::KeyX if action_mod => {
                            if !self.selection.is_collapsed() {
                                let text = &self.buffer[self.selection.text_range()];
                                let mut cb: clipboard::ClipboardContext =
                                    ClipboardProvider::new().unwrap();
                                cb.set_contents(text.to_owned()).ok();
                                if let Some(start) = self.delete_current_selection() {
                                    self.update_layout(self.width, 1.0);
                                    let (start, affinity) = if start > 0 {
                                        (start - 1, Affinity::Upstream)
                                    } else {
                                        (start, Affinity::Downstream)
                                    };
                                    self.selection =
                                        Selection::from_index(&self.layout, start, affinity);
                                }
                            }
                        }
                        KeyCode::KeyV if action_mod => {
                            let mut cb: clipboard::ClipboardContext =
                                ClipboardProvider::new().unwrap();
                            let text = cb.get_contents().unwrap_or_default();
                            let start = self
                                .delete_current_selection()
                                .unwrap_or_else(|| self.selection.focus().text_range().start);
                            self.buffer.insert_str(start, &text);
                            self.update_layout(self.width, 1.0);
                            self.selection = Selection::from_index(
                                &self.layout,
                                start + text.len(),
                                Affinity::default(),
                            );
                        }
                        KeyCode::ArrowLeft => {
                            self.selection = if ctrl {
                                self.selection.previous_word(&self.layout, shift)
                            } else {
                                self.selection.previous_visual(
                                    &self.layout,
                                    self.cursor_mode,
                                    shift,
                                )
                            };
                        }
                        KeyCode::ArrowRight => {
                            self.selection = if ctrl {
                                self.selection.next_word(&self.layout, shift)
                            } else {
                                self.selection
                                    .next_visual(&self.layout, self.cursor_mode, shift)
                            };
                        }
                        KeyCode::ArrowUp => {
                            self.selection = self.selection.previous_line(&self.layout, shift);
                        }
                        KeyCode::ArrowDown => {
                            self.selection = self.selection.next_line(&self.layout, shift);
                        }
                        KeyCode::Home => {
                            self.selection = self.selection.line_start(&self.layout, shift);
                        }
                        KeyCode::End => {
                            self.selection = self.selection.line_end(&self.layout, shift);
                        }
                        KeyCode::Delete => {
                            if self.selection.is_collapsed() {
                                let range = self.selection.focus().text_range();
                                self.buffer.replace_range(range, "");
                            } else {
                                self.delete_current_selection();
                            };
                            self.update_layout(self.width, 1.0);
                            self.selection = self.selection.refresh(&self.layout);
                        }
                        KeyCode::Backspace => {
                            let start = if self.selection.is_collapsed() {
                                let end = self.selection.focus().text_range().start;
                                if let Some((start, _)) =
                                    self.buffer[..end].char_indices().next_back()
                                {
                                    self.buffer.replace_range(start..end, "");
                                    Some(start)
                                } else {
                                    None
                                }
                            } else {
                                self.delete_current_selection()
                            };
                            if let Some(start) = start {
                                self.update_layout(self.width, 1.0);
                                let (start, affinity) = if start > 0 {
                                    (start - 1, Affinity::Upstream)
                                } else {
                                    (start, Affinity::Downstream)
                                };
                                self.selection =
                                    Selection::from_index(&self.layout, start, affinity);
                            }
                        }
                        _ => {
                            if let Some(text) = &event.text {
                                let start = self
                                    .delete_current_selection()
                                    .unwrap_or_else(|| self.selection.focus().text_range().start);
                                self.buffer.insert_str(start, text);
                                self.update_layout(self.width, 1.0);
                                self.selection = Selection::from_index(
                                    &self.layout,
                                    start + text.len() - 1,
                                    Affinity::Upstream,
                                );
                            }
                        }
                    }
                }

                println!("Active text: {:?}", self.active_text());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == winit::event::MouseButton::Left {
                    self.pointer_down = state.is_pressed();
                    if self.pointer_down {
                        let now = Instant::now();
                        if let Some(last) = self.last_click_time.take() {
                            if now.duration_since(last).as_secs_f64() < 0.25 {
                                self.click_count = (self.click_count + 1) % 3;
                            } else {
                                self.click_count = 1;
                            }
                        } else {
                            self.click_count = 1;
                        }
                        self.last_click_time = Some(now);
                        match self.click_count {
                            2 => {
                                println!("SELECTING WORD");
                                self.selection = Selection::word_from_point(
                                    &self.layout,
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                );
                            }
                            // TODO: handle line
                            _ => {
                                self.selection = Selection::from_point(
                                    &self.layout,
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                );
                            }
                        }
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

    fn delete_current_selection(&mut self) -> Option<usize> {
        if !self.selection.is_collapsed() {
            let range = self.selection.text_range();
            let start = range.start;
            self.buffer.replace_range(range, "");
            Some(start)
        } else {
            None
        }
    }

    pub fn draw(&self, scene: &mut Scene) {
        let transform = Affine::translate((INSET as f64, INSET as f64));
        self.selection.geometry_with(&self.layout, |rect| {
            scene.fill(Fill::NonZero, transform, Color::STEEL_BLUE, None, &rect);
        });
        if let Some(cursor) = self.selection.focus().strong_geometry(&self.layout, 1.5) {
            scene.fill(Fill::NonZero, transform, Color::WHITE, None, &cursor);
        };
        if let Some(cursor) = self.selection.focus().weak_geometry(&self.layout, 1.5) {
            scene.fill(Fill::NonZero, transform, Color::LIGHT_GRAY, None, &cursor);
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

pub const LOREM: &str = r" Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi cursus mi sed euismod euismod. Orci varius natoque penatibus et magnis dis parturient montes, nascetur ridiculus mus. Nullam placerat efficitur tellus at semper. Morbi ac risus magna. Donec ut cursus ex. Etiam quis posuere tellus. Mauris posuere dui et turpis mollis, vitae luctus tellus consectetur. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Curabitur eu facilisis nisl.

Phasellus in viverra dolor, vitae facilisis est. Maecenas malesuada massa vel ultricies feugiat. Vivamus venenatis et gהתעשייה בנושא האינטרנטa nibh nec pharetra. Phasellus vestibulum elit enim, nec scelerisque orci faucibus id. Vivamus consequat purus sit amet orci egestas, non iaculis massa porttitor. Vestibulum ut eros leo. In fermentum convallis magna in finibus. Donec justo leo, maximus ac laoreet id, volutpat ut elit. Mauris sed leo non neque laoreet faucibus. Aliquam orci arcu, faucibus in molestie eget, ornare non dui. Donec volutpat nulla in fringilla elementum. Aliquam vitae ante egestas ligula tempus vestibulum sit amet sed ante. ";
