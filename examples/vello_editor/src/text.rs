// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use accesskit::{Node, TreeUpdate};
use core::default::Default;
use parley::layout::PositionedLayoutItem;
use peniko::{kurbo::Affine, Color, Fill};
use std::time::{Duration, Instant};
use vello::Scene;
use winit::{
    event::{Modifiers, Touch, WindowEvent},
    keyboard::{Key, NamedKey},
};

pub use parley::layout::editor::Generation;
use parley::{FontContext, LayoutContext, PlainEditor, PlainEditorTxn};

use crate::access_ids::next_node_id;

pub const INSET: f32 = 32.0;

#[derive(Default)]
pub struct Editor {
    font_cx: FontContext,
    layout_cx: LayoutContext<Color>,
    editor: PlainEditor<Color>,
    last_click_time: Option<Instant>,
    click_count: u32,
    pointer_down: bool,
    cursor_pos: (f32, f32),
    cursor_visible: bool,
    modifiers: Option<Modifiers>,
    start_time: Option<Instant>,
    blink_period: Duration,
}

impl Editor {
    pub fn transact(&mut self, callback: impl FnOnce(&mut PlainEditorTxn<'_, Color>)) {
        self.editor
            .transact(&mut self.font_cx, &mut self.layout_cx, callback);
    }

    pub fn text(&self) -> &str {
        self.editor.text()
    }

    pub fn cursor_reset(&mut self) {
        self.start_time = Some(Instant::now());
        // TODO: for real world use, this should be reading from the system settings
        self.blink_period = Duration::from_millis(500);
        self.cursor_visible = true;
    }

    pub fn disable_blink(&mut self) {
        self.start_time = None;
    }

    pub fn next_blink_time(&self) -> Option<Instant> {
        self.start_time.map(|start_time| {
            let phase = Instant::now().duration_since(start_time);

            start_time
                + Duration::from_nanos(
                    ((phase.as_nanos() / self.blink_period.as_nanos() + 1)
                        * self.blink_period.as_nanos()) as u64,
                )
        })
    }

    pub fn cursor_blink(&mut self) {
        self.cursor_visible = self.start_time.map_or(false, |start_time| {
            let elapsed = Instant::now().duration_since(start_time);
            (elapsed.as_millis() / self.blink_period.as_millis()) % 2 == 0
        });
    }

    pub fn handle_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.transact(|txn| txn.set_width(Some(size.width as f32 - 2f32 * INSET)));
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = Some(modifiers);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.state.is_pressed() {
                    return;
                }
                self.cursor_reset();
                #[allow(unused)]
                let (shift, action_mod) = self
                    .modifiers
                    .map(|mods| {
                        (
                            mods.state().shift_key(),
                            if cfg!(target_os = "macos") {
                                mods.state().super_key()
                            } else {
                                mods.state().control_key()
                            },
                        )
                    })
                    .unwrap_or_default();

                match event.logical_key {
                    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
                    Key::Character(c) if action_mod && matches!(c.as_str(), "c" | "x" | "v") => {
                        use clipboard_rs::{Clipboard, ClipboardContext};
                        match c.to_lowercase().as_str() {
                            "c" => {
                                if let Some(text) = self.editor.selected_text() {
                                    let cb = ClipboardContext::new().unwrap();
                                    cb.set_text(text.to_owned()).ok();
                                }
                            }
                            "x" => {
                                if let Some(text) = self.editor.selected_text() {
                                    let cb = ClipboardContext::new().unwrap();
                                    cb.set_text(text.to_owned()).ok();
                                    self.transact(|txn| txn.delete_selection());
                                }
                            }
                            "v" => {
                                let cb = ClipboardContext::new().unwrap();
                                let text = cb.get_text().unwrap_or_default();
                                self.transact(|txn| txn.insert_or_replace_selection(&text));
                            }
                            _ => (),
                        }
                    }
                    Key::Character(c) if action_mod && matches!(c.to_lowercase().as_str(), "a") => {
                        self.transact(|txn| {
                            if shift {
                                txn.collapse_selection();
                            } else {
                                txn.select_all();
                            }
                        });
                    }
                    Key::Named(NamedKey::ArrowLeft) => self.transact(|txn| {
                        if action_mod {
                            if shift {
                                txn.select_word_left();
                            } else {
                                txn.move_word_left();
                            }
                        } else if shift {
                            txn.select_left();
                        } else {
                            txn.move_left();
                        }
                    }),
                    Key::Named(NamedKey::ArrowRight) => self.transact(|txn| {
                        if action_mod {
                            if shift {
                                txn.select_word_right();
                            } else {
                                txn.move_word_right();
                            }
                        } else if shift {
                            txn.select_right();
                        } else {
                            txn.move_right();
                        }
                    }),
                    Key::Named(NamedKey::ArrowUp) => self.transact(|txn| {
                        if shift {
                            txn.select_up();
                        } else {
                            txn.move_up();
                        }
                    }),
                    Key::Named(NamedKey::ArrowDown) => self.transact(|txn| {
                        if shift {
                            txn.select_down();
                        } else {
                            txn.move_down();
                        }
                    }),
                    Key::Named(NamedKey::Home) => self.transact(|txn| {
                        if action_mod {
                            if shift {
                                txn.select_to_text_start();
                            } else {
                                txn.move_to_text_start();
                            }
                        } else if shift {
                            txn.select_to_line_start();
                        } else {
                            txn.move_to_line_start();
                        }
                    }),
                    Key::Named(NamedKey::End) => self.transact(|txn| {
                        if action_mod {
                            if shift {
                                txn.select_to_text_end();
                            } else {
                                txn.move_to_text_end();
                            }
                        } else if shift {
                            txn.select_to_line_end();
                        } else {
                            txn.move_to_line_end();
                        }
                    }),
                    Key::Named(NamedKey::Delete) => self.transact(|txn| {
                        if action_mod {
                            txn.delete_word();
                        } else {
                            txn.delete();
                        }
                    }),
                    Key::Named(NamedKey::Backspace) => self.transact(|txn| {
                        if action_mod {
                            txn.backdelete_word();
                        } else {
                            txn.backdelete();
                        }
                    }),
                    Key::Named(NamedKey::Enter) => {
                        self.transact(|txn| txn.insert_or_replace_selection("\n"));
                    }
                    Key::Named(NamedKey::Space) => {
                        self.transact(|txn| txn.insert_or_replace_selection(" "));
                    }
                    Key::Character(s) => {
                        self.transact(|txn| txn.insert_or_replace_selection(&s));
                    }
                    _ => (),
                }
            }
            WindowEvent::Touch(Touch {
                phase, location, ..
            }) => {
                use winit::event::TouchPhase::*;
                match phase {
                    Started => {
                        // TODO: start a timer to convert to a SelectWordAtPoint
                        self.transact(|txn| {
                            txn.move_to_point(location.x as f32 - INSET, location.y as f32 - INSET);
                        });
                    }
                    Cancelled => {
                        self.transact(|txn| txn.collapse_selection());
                    }
                    Moved => {
                        // TODO: cancel SelectWordAtPoint timer
                        self.transact(|txn| {
                            txn.extend_selection_to_point(
                                location.x as f32 - INSET,
                                location.y as f32 - INSET,
                            );
                        });
                    }
                    Ended => (),
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == winit::event::MouseButton::Left {
                    self.pointer_down = state.is_pressed();
                    self.cursor_reset();
                    if self.pointer_down {
                        let now = Instant::now();
                        if let Some(last) = self.last_click_time.take() {
                            if now.duration_since(last).as_secs_f64() < 0.25 {
                                self.click_count = (self.click_count + 1) % 4;
                            } else {
                                self.click_count = 1;
                            }
                        } else {
                            self.click_count = 1;
                        }
                        self.last_click_time = Some(now);
                        let click_count = self.click_count;
                        let cursor_pos = self.cursor_pos;
                        self.transact(|txn| match click_count {
                            2 => txn.select_word_at_point(cursor_pos.0, cursor_pos.1),
                            3 => txn.select_line_at_point(cursor_pos.0, cursor_pos.1),
                            _ => txn.move_to_point(cursor_pos.0, cursor_pos.1),
                        });
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let prev_pos = self.cursor_pos;
                self.cursor_pos = (position.x as f32 - INSET, position.y as f32 - INSET);
                // macOS seems to generate a spurious move after selecting word?
                if self.pointer_down && prev_pos != self.cursor_pos {
                    self.cursor_reset();
                    let cursor_pos = self.cursor_pos;
                    self.transact(|txn| txn.extend_selection_to_point(cursor_pos.0, cursor_pos.1));
                }
            }
            _ => {}
        }
    }

    pub fn handle_accesskit_action_request(&mut self, req: &accesskit::ActionRequest) {
        if req.action == accesskit::Action::SetTextSelection {
            if let Some(accesskit::ActionData::SetTextSelection(selection)) = &req.data {
                self.transact(|txn| {
                    txn.select_from_accesskit(selection);
                });
            }
        }
    }

    /// Return the current `Generation` of the layout.
    pub fn generation(&self) -> Generation {
        self.editor.generation()
    }

    /// Draw into scene.
    ///
    /// Returns drawn `Generation`.
    pub fn draw(&self, scene: &mut Scene) -> Generation {
        let transform = Affine::translate((INSET as f64, INSET as f64));
        for rect in self.editor.selection_geometry().iter() {
            scene.fill(Fill::NonZero, transform, Color::STEEL_BLUE, None, &rect);
        }
        if self.cursor_visible {
            if let Some(cursor) = self.editor.cursor_geometry(1.5) {
                scene.fill(Fill::NonZero, transform, Color::WHITE, None, &cursor);
            };
        }
        for line in self.editor.lines() {
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
                            vello::Glyph {
                                id: glyph.id as _,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
            }
        }
        self.editor.generation()
    }

    pub fn accessibility(&mut self, update: &mut TreeUpdate, node: &mut Node) {
        self.editor
            .accessibility(update, node, next_node_id, INSET.into(), INSET.into());
    }
}

pub const LOREM: &str = r" Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi cursus mi sed euismod euismod. Orci varius natoque penatibus et magnis dis parturient montes, nascetur ridiculus mus. Nullam placerat efficitur tellus at semper. Morbi ac risus magna. Donec ut cursus ex. Etiam quis posuere tellus. Mauris posuere dui et turpis mollis, vitae luctus tellus consectetur. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Curabitur eu facilisis nisl.

Phasellus in viverra dolor, vitae facilisis est. Maecenas malesuada massa vel ultricies feugiat. Vivamus venenatis et gהתעשייה בנושא האינטרנטa nibh nec pharetra. Phasellus vestibulum elit enim, nec scelerisque orci faucibus id. Vivamus consequat purus sit amet orci egestas, non iaculis massa porttitor. Vestibulum ut eros leo. In fermentum convallis magna in finibus. Donec justo leo, maximus ac laoreet id, volutpat ut elit. Mauris sed leo non neque laoreet faucibus. Aliquam orci arcu, faucibus in molestie eget, ornare non dui. Donec volutpat nulla in fringilla elementum. Aliquam vitae ante egestas ligula tempus vestibulum sit amet sed ante. ";
