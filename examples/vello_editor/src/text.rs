// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(not(target_os = "android"))]
use clipboard_rs::{Clipboard, ClipboardContext};
use parley::layout::cursor::{Cursor, Selection, VisualMode};
use parley::layout::Affinity;
use parley::{layout::PositionedLayoutItem, FontContext};
use peniko::{kurbo::Affine, Color, Fill};
use std::time::Instant;
use vello::kurbo::{Line, Stroke};
use vello::Scene;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::Ime;
use winit::window::Window;
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
enum ComposeState {
    #[default]
    None,
    Preedit {
        /// The location of the (uncommitted) preedit text
        text_at: Selection,
    },
}

#[derive(Default)]
pub struct Editor {
    font_cx: FontContext,
    layout_cx: LayoutContext,
    buffer: String,
    layout: Layout,
    selection: Selection,
    compose_state: ComposeState,
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
        if let ComposeState::Preedit { text_at } = self.compose_state {
            let text_range = text_at.text_range();
            builder.push(
                &parley::style::StyleProperty::UnderlineBrush(Some(Color::SPRING_GREEN)),
                text_range.clone(),
            );
            builder.push(&parley::style::StyleProperty::Underline(true), text_range);
        }
        builder.build_into(&mut self.layout);
        self.layout.break_all_lines(Some(width - INSET * 2.0));
        self.layout
            .align(Some(width - INSET * 2.0), parley::layout::Alignment::Start);
        self.width = width;
    }

    #[allow(unused)]
    pub fn active_text(&self) -> ActiveText {
        if self.selection.is_collapsed() {
            let range = self
                .selection
                .focus()
                .cluster_path()
                .cluster(&self.layout)
                .map(|c| c.text_range())
                .unwrap_or_default();
            ActiveText::FocusedCluster(self.selection.focus().affinity(), &self.buffer[range])
        } else {
            ActiveText::Selection(&self.buffer[self.selection.text_range()])
        }
    }

    #[cfg(not(target_os = "android"))]
    fn handle_clipboard(&mut self, code: KeyCode) {
        match code {
            KeyCode::KeyC => {
                if !self.selection.is_collapsed() {
                    let text = &self.buffer[self.selection.text_range()];
                    let cb = ClipboardContext::new().unwrap();
                    cb.set_text(text.to_owned()).ok();
                }
            }
            KeyCode::KeyX => {
                if !self.selection.is_collapsed() {
                    let text = &self.buffer[self.selection.text_range()];
                    let cb = ClipboardContext::new().unwrap();
                    cb.set_text(text.to_owned()).ok();
                    if let Some(start) = self.delete_current_selection() {
                        self.update_layout(self.width, 1.0);
                        let (start, affinity) = if start > 0 {
                            (start - 1, Affinity::Upstream)
                        } else {
                            (start, Affinity::Downstream)
                        };
                        self.selection = Selection::from_index(&self.layout, start, affinity);
                    }
                }
            }
            KeyCode::KeyV => {
                let cb = ClipboardContext::new().unwrap();
                let text = cb.get_text().unwrap_or_default();
                let start = self
                    .delete_current_selection()
                    .unwrap_or_else(|| self.selection.focus().text_range().start);
                self.buffer.insert_str(start, &text);
                self.update_layout(self.width, 1.0);
                self.selection =
                    Selection::from_index(&self.layout, start + text.len(), Affinity::default());
            }
            _ => {}
        }
    }

    #[cfg(target_os = "android")]
    fn handle_clipboard(&mut self, _code: KeyCode) {
        // TODO: support clipboard on Android
    }

    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) {
        if let ComposeState::Preedit { text_at } = self.compose_state {
            // Clear old preedit state when handling events that potentially mutate text/selection.
            // This is a bit overzealous, e.g., pressing and releasing shift probably shouldnt't
            // clear the preedit.
            if matches!(
                event,
                WindowEvent::KeyboardInput { .. }
                    | WindowEvent::MouseInput { .. }
                    | WindowEvent::Ime(..)
            ) {
                let range = text_at.text_range();
                self.selection =
                    Selection::from_index(&self.layout, range.start - 1, Affinity::Upstream);
                self.buffer.replace_range(range, "");
                self.compose_state = ComposeState::None;
                // TODO: defer updating layout. If the event itself also causes an update, we now
                // update twice.
                self.update_layout(self.width, 1.0);
            }
        }

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
                            self.handle_clipboard(code);
                        }
                        KeyCode::KeyX if action_mod => {
                            self.handle_clipboard(code);
                        }
                        KeyCode::KeyV if action_mod => {
                            self.handle_clipboard(code);
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
                            if ctrl {
                                self.selection =
                                    self.selection.move_lines(&self.layout, isize::MIN, shift);
                            } else {
                                self.selection = self.selection.line_start(&self.layout, shift);
                            }
                        }
                        KeyCode::End => {
                            if ctrl {
                                self.selection =
                                    self.selection.move_lines(&self.layout, isize::MAX, shift);
                            } else {
                                self.selection = self.selection.line_end(&self.layout, shift);
                            }
                        }
                        KeyCode::Delete => {
                            let start = if self.selection.is_collapsed() {
                                let range = self.selection.focus().text_range();
                                let start = range.start;
                                self.buffer.replace_range(range, "");
                                Some(start)
                            } else {
                                self.delete_current_selection()
                            };
                            if let Some(start) = start {
                                self.update_layout(self.width, 1.0);
                                self.selection =
                                    Selection::from_index(&self.layout, start, Affinity::default());
                            }
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

                // println!("Active text: {:?}", self.active_text());
            }
            WindowEvent::Ime(ime) => {
                match ime {
                    Ime::Commit(text) => {
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
                    Ime::Preedit(text, compose_cursor) => {
                        if text.is_empty() {
                            // Winit sends empty preedit text to indicate the preedit was cleared.
                            return;
                        }

                        let start = self
                            .delete_current_selection()
                            .unwrap_or_else(|| self.selection.focus().text_range().start);
                        self.buffer.insert_str(start, text);

                        {
                            // winit says the cursor should be hidden when compose_cursor is None.
                            // Do we handle that? We also don't extend the cursor to the end
                            // indicated by winit, instead IME composing is currently indicated by
                            // highlighting the entire preedit text. Should we even update the
                            // selection at all?
                            let compose_cursor = compose_cursor.unwrap_or((0, 0));
                            self.selection = Selection::from_index(
                                &self.layout,
                                start - 1 + compose_cursor.0,
                                Affinity::Upstream,
                            );
                        }

                        {
                            let text_end = Cursor::from_index(
                                &self.layout,
                                start - 1 + text.len(),
                                Affinity::Upstream,
                            );
                            let ime_cursor = self.selection.extend_to_cursor(text_end);
                            self.compose_state = ComposeState::Preedit {
                                text_at: ime_cursor,
                            };

                            // Find the smallest rectangle that contains the entire preedit text.
                            // Send that rectangle to the platform to suggest placement for the IME
                            // candidate box.
                            let mut union_rect = None;
                            ime_cursor.geometry_with(&self.layout, |rect| {
                                if union_rect.is_none() {
                                    union_rect = Some(rect);
                                }
                                union_rect = Some(union_rect.unwrap().union(rect));
                            });
                            if let Some(union_rect) = union_rect {
                                window.set_ime_cursor_area(
                                    LogicalPosition::new(union_rect.x0, union_rect.y0),
                                    LogicalSize::new(
                                        union_rect.width(),
                                        // TODO: an offset is added here to prevent the IME
                                        // candidate box from overlapping with the IME cursor. From
                                        // the Winit docs I would've expected the IME candidate box
                                        // not to overlap the indicated IME cursor area, but for
                                        // some reason it does (tested using fcitx5
                                        // on wayland)
                                        union_rect.height() + 40.0,
                                    ),
                                );
                            }
                        }

                        self.update_layout(self.width, 1.0);
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == winit::event::MouseButton::Left {
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
                        match self.click_count {
                            2 => {
                                self.selection = Selection::word_from_point(
                                    &self.layout,
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                );
                            }
                            3 => {
                                let focus = *Selection::from_point(
                                    &self.layout,
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                )
                                .line_start(&self.layout, true)
                                .focus();
                                self.selection =
                                    Selection::from(focus).line_end(&self.layout, true);
                            }
                            _ => {
                                self.selection = Selection::from_point(
                                    &self.layout,
                                    self.cursor_pos.0,
                                    self.cursor_pos.1,
                                );
                            }
                        }
                        // println!("Active text: {:?}", self.active_text());
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let prev_pos = self.cursor_pos;
                self.cursor_pos = (position.x as f32 - INSET, position.y as f32 - INSET);
                // macOS seems to generate a spurious move after selecting word?
                if self.pointer_down && prev_pos != self.cursor_pos {
                    self.selection = self.selection.extend_to_point(
                        &self.layout,
                        self.cursor_pos.0,
                        self.cursor_pos.1,
                    );
                    // println!("Active text: {:?}", self.active_text());
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
                if let Some(underline) = &style.underline {
                    let underline_brush = &underline.brush;
                    let run_metrics = glyph_run.run().metrics();
                    let offset = match underline.offset {
                        Some(offset) => offset,
                        None => run_metrics.underline_offset,
                    };
                    let width = match underline.size {
                        Some(size) => size,
                        None => run_metrics.underline_size,
                    };
                    // The `offset` is the distance from the baseline to the *top* of the underline
                    // so we move the line down by half the width
                    // Remember that we are using a y-down coordinate system
                    let y = glyph_run.baseline() - offset + width / 2.;

                    let line = Line::new(
                        (glyph_run.offset() as f64, y as f64),
                        ((glyph_run.offset() + glyph_run.advance()) as f64, y as f64),
                    );
                    scene.stroke(
                        &Stroke::new(width.into()),
                        transform,
                        underline_brush,
                        None,
                        &line,
                    );
                }
            }
        }
    }
}

pub const LOREM: &str = r" Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi cursus mi sed euismod euismod. Orci varius natoque penatibus et magnis dis parturient montes, nascetur ridiculus mus. Nullam placerat efficitur tellus at semper. Morbi ac risus magna. Donec ut cursus ex. Etiam quis posuere tellus. Mauris posuere dui et turpis mollis, vitae luctus tellus consectetur. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Curabitur eu facilisis nisl.

Phasellus in viverra dolor, vitae facilisis est. Maecenas malesuada massa vel ultricies feugiat. Vivamus venenatis et gהתעשייה בנושא האינטרנטa nibh nec pharetra. Phasellus vestibulum elit enim, nec scelerisque orci faucibus id. Vivamus consequat purus sit amet orci egestas, non iaculis massa porttitor. Vestibulum ut eros leo. In fermentum convallis magna in finibus. Donec justo leo, maximus ac laoreet id, volutpat ut elit. Mauris sed leo non neque laoreet faucibus. Aliquam orci arcu, faucibus in molestie eget, ornare non dui. Donec volutpat nulla in fringilla elementum. Aliquam vitae ante egestas ligula tempus vestibulum sit amet sed ante. ";
