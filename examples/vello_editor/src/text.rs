// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use parley::layout::PositionedLayoutItem;
use peniko::{kurbo::Affine, Color, Fill};
use std::time::Instant;
use vello::Scene;
use winit::{
    event::{Modifiers, Touch, WindowEvent},
    keyboard::{Key, NamedKey},
};

extern crate alloc;
use alloc::{sync::Arc, vec};

use core::{default::Default, iter::IntoIterator};

use parley::{FontContext, LayoutContext, PlainEditor, PlainEditorOp};

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
    modifiers: Option<Modifiers>,
}

impl Editor {
    pub fn transact(&mut self, t: impl IntoIterator<Item = PlainEditorOp<Color>>) {
        self.editor
            .transact(&mut self.font_cx, &mut self.layout_cx, t);
    }

    pub fn text(&self) -> Arc<str> {
        self.editor.text()
    }

    pub fn handle_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.editor.transact(
                    &mut self.font_cx,
                    &mut self.layout_cx,
                    [PlainEditorOp::SetWidth(Some(
                        size.width as f32 - 2f32 * INSET,
                    ))],
                );
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = Some(modifiers);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.state.is_pressed() {
                    return;
                }
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

                self.editor.transact(
                    &mut self.font_cx,
                    &mut self.layout_cx,
                    match event.logical_key {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        Key::Character(c)
                            if action_mod && matches!(c.as_str(), "c" | "x" | "v") =>
                        {
                            use clipboard_rs::{Clipboard, ClipboardContext};
                            use parley::layout::editor::ActiveText;

                            match c.to_lowercase().as_str() {
                                "c" => {
                                    if let ActiveText::Selection(text) = self.editor.active_text() {
                                        let cb = ClipboardContext::new().unwrap();
                                        cb.set_text(text.to_owned()).ok();
                                    }
                                    vec![]
                                }
                                "x" => {
                                    if let ActiveText::Selection(text) = self.editor.active_text() {
                                        let cb = ClipboardContext::new().unwrap();
                                        cb.set_text(text.to_owned()).ok();
                                        vec![PlainEditorOp::DeleteSelection]
                                    } else {
                                        vec![]
                                    }
                                }
                                "v" => {
                                    let cb = ClipboardContext::new().unwrap();
                                    let text = cb.get_text().unwrap_or_default();
                                    vec![PlainEditorOp::InsertOrReplaceSelection(text.into())]
                                }
                                _ => vec![],
                            }
                        }
                        Key::Character(c)
                            if action_mod && matches!(c.to_lowercase().as_str(), "a") =>
                        {
                            vec![if shift {
                                PlainEditorOp::CollapseSelection
                            } else {
                                PlainEditorOp::SelectAll
                            }]
                        }
                        Key::Named(NamedKey::ArrowLeft) => vec![if action_mod {
                            if shift {
                                PlainEditorOp::SelectWordLeft
                            } else {
                                PlainEditorOp::MoveWordLeft
                            }
                        } else if shift {
                            PlainEditorOp::SelectLeft
                        } else {
                            PlainEditorOp::MoveLeft
                        }],
                        Key::Named(NamedKey::ArrowRight) => vec![if action_mod {
                            if shift {
                                PlainEditorOp::SelectWordRight
                            } else {
                                PlainEditorOp::MoveWordRight
                            }
                        } else if shift {
                            PlainEditorOp::SelectRight
                        } else {
                            PlainEditorOp::MoveRight
                        }],
                        Key::Named(NamedKey::ArrowUp) => vec![if shift {
                            PlainEditorOp::SelectUp
                        } else {
                            PlainEditorOp::MoveUp
                        }],
                        Key::Named(NamedKey::ArrowDown) => vec![if shift {
                            PlainEditorOp::SelectDown
                        } else {
                            PlainEditorOp::MoveDown
                        }],
                        Key::Named(NamedKey::Home) => vec![if action_mod {
                            if shift {
                                PlainEditorOp::SelectToTextStart
                            } else {
                                PlainEditorOp::MoveToTextStart
                            }
                        } else if shift {
                            PlainEditorOp::SelectToLineStart
                        } else {
                            PlainEditorOp::MoveToLineStart
                        }],
                        Key::Named(NamedKey::End) => vec![if action_mod {
                            if shift {
                                PlainEditorOp::SelectToTextEnd
                            } else {
                                PlainEditorOp::MoveToTextEnd
                            }
                        } else if shift {
                            PlainEditorOp::SelectToLineEnd
                        } else {
                            PlainEditorOp::MoveToLineEnd
                        }],
                        Key::Named(NamedKey::Delete) => vec![if action_mod {
                            PlainEditorOp::DeleteWord
                        } else {
                            PlainEditorOp::Delete
                        }],
                        Key::Named(NamedKey::Backspace) => vec![if action_mod {
                            PlainEditorOp::BackdeleteWord
                        } else {
                            PlainEditorOp::Backdelete
                        }],
                        Key::Named(NamedKey::Enter) => {
                            vec![PlainEditorOp::InsertOrReplaceSelection("\n".into())]
                        }
                        Key::Named(NamedKey::Space) => {
                            vec![PlainEditorOp::InsertOrReplaceSelection(" ".into())]
                        }
                        Key::Character(s) => {
                            vec![PlainEditorOp::InsertOrReplaceSelection(s.into())]
                        }
                        _ => vec![],
                    },
                );

                // println!("Active text: {:?}", self.active_text());
            }
            WindowEvent::Touch(Touch {
                phase, location, ..
            }) => {
                use winit::event::TouchPhase::*;
                self.editor.transact(
                    &mut self.font_cx,
                    &mut self.layout_cx,
                    match phase {
                        Started => {
                            // TODO: start a timer to convert to a SelectWordAtPoint
                            vec![PlainEditorOp::MoveToPoint(
                                location.x as f32 - INSET,
                                location.y as f32 - INSET,
                            )]
                        }
                        Cancelled => {
                            vec![PlainEditorOp::CollapseSelection]
                        }
                        Moved => {
                            // TODO: cancel SelectWordAtPoint timer
                            vec![PlainEditorOp::ExtendSelectionToPoint(
                                location.x as f32 - INSET,
                                location.y as f32 - INSET,
                            )]
                        }
                        Ended => vec![],
                    },
                );
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == winit::event::MouseButton::Left {
                    self.pointer_down = state.is_pressed();
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
                        self.editor.transact(
                            &mut self.font_cx,
                            &mut self.layout_cx,
                            match self.click_count {
                                2 => [PlainEditorOp::SelectWordAtPoint(
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                )],
                                3 => [PlainEditorOp::SelectLineAtPoint(
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                )],
                                _ => [PlainEditorOp::MoveToPoint(
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                )],
                            },
                        );

                        // println!("Active text: {:?}", self.active_text());
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let prev_pos = self.cursor_pos;
                self.cursor_pos = (position.x as f32 - INSET, position.y as f32 - INSET);
                // macOS seems to generate a spurious move after selecting word?
                if self.pointer_down && prev_pos != self.cursor_pos {
                    self.editor.transact(
                        &mut self.font_cx,
                        &mut self.layout_cx,
                        [PlainEditorOp::ExtendSelectionToPoint(
                            self.cursor_pos.0,
                            self.cursor_pos.1,
                        )],
                    );
                    // println!("Active text: {:?}", self.active_text());
                }
            }
            _ => {}
        }
    }

    /// Return the current generation of the layout.
    pub fn generation(&self) -> usize {
        self.editor.generation()
    }

    /// Draw into scene.
    ///
    /// Returns drawn generation.
    pub fn draw(&self, scene: &mut Scene) -> usize {
        let transform = Affine::translate((INSET as f64, INSET as f64));
        for rect in self.editor.selection_geometry().iter() {
            scene.fill(Fill::NonZero, transform, Color::STEEL_BLUE, None, &rect);
        }
        if let Some(cursor) = self.editor.selection_strong_geometry(1.5) {
            scene.fill(Fill::NonZero, transform, Color::WHITE, None, &cursor);
        };
        if let Some(cursor) = self.editor.selection_weak_geometry(1.5) {
            scene.fill(Fill::NonZero, transform, Color::LIGHT_GRAY, None, &cursor);
        };
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
}

pub const LOREM: &str = r" Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi cursus mi sed euismod euismod. Orci varius natoque penatibus et magnis dis parturient montes, nascetur ridiculus mus. Nullam placerat efficitur tellus at semper. Morbi ac risus magna. Donec ut cursus ex. Etiam quis posuere tellus. Mauris posuere dui et turpis mollis, vitae luctus tellus consectetur. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Curabitur eu facilisis nisl.

Phasellus in viverra dolor, vitae facilisis est. Maecenas malesuada massa vel ultricies feugiat. Vivamus venenatis et gהתעשייה בנושא האינטרנטa nibh nec pharetra. Phasellus vestibulum elit enim, nec scelerisque orci faucibus id. Vivamus consequat purus sit amet orci egestas, non iaculis massa porttitor. Vestibulum ut eros leo. In fermentum convallis magna in finibus. Donec justo leo, maximus ac laoreet id, volutpat ut elit. Mauris sed leo non neque laoreet faucibus. Aliquam orci arcu, faucibus in molestie eget, ornare non dui. Donec volutpat nulla in fringilla elementum. Aliquam vitae ante egestas ligula tempus vestibulum sit amet sed ante. ";
