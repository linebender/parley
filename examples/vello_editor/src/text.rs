// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(not(target_os = "android"))]
use clipboard_rs::{Clipboard, ClipboardContext};
use parley::layout::cursor::{Cursor, Selection, VisualMode};
use parley::layout::Affinity;
use parley::{layout::PositionedLayoutItem, FontContext};
use peniko::{kurbo::Affine, Color, Fill};
use std::ops::Range;
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
pub struct Editor {
    font_cx: FontContext,
    layout_cx: LayoutContext,
    buffer: String,
    layout: Layout,
    selection: Selection,
    /// The portion of the text currently marked as preedit by the IME.
    preedit_range: Option<Range<usize>>,
    cursor_mode: VisualMode,
    last_click_time: Option<Instant>,
    click_count: u32,
    pointer_down: bool,
    cursor_pos: (f32, f32),
    modifiers: Option<Modifiers>,
    width: f32,
}

/// Shrink the selection by the given amount of bytes by moving the focus towards the anchor.
fn shrink_selection(layout: &Layout, selection: Selection, bytes: usize) -> Selection {
    let mut selection = selection;
    let shrink = bytes.min(selection.text_range().len());
    if shrink == 0 {
        return selection;
    }

    let anchor = *selection.anchor();
    let focus = *selection.focus();

    let new_focus_index = if focus.text_range().start > anchor.text_range().start {
        focus.index() - shrink
    } else {
        focus.index() + shrink
    };

    selection = Selection::from_index(layout, anchor.index(), anchor.affinity());
    selection = selection.extend_to_cursor(Cursor::from_index(
        layout,
        new_focus_index,
        focus.affinity(),
    ));

    selection
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
        if let Some(ref text_range) = self.preedit_range {
            builder.push(
                &parley::style::StyleProperty::UnderlineBrush(Some(Color::SPRING_GREEN)),
                text_range.clone(),
            );
            builder.push(
                &parley::style::StyleProperty::Underline(true),
                text_range.clone(),
            );
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

    /// Suggest an area for IME candidate box placement based on the current IME state.
    fn set_ime_cursor_area(&self, window: &Window) {
        if let Some(ref text_range) = self.preedit_range {
            // Find the smallest rectangle that contains the entire preedit text.
            // Send that rectangle to the platform to suggest placement for the IME
            // candidate box.
            let mut union_rect = None;
            let preedit_selection =
                Selection::from_index(&self.layout, text_range.start, Affinity::Downstream);
            let preedit_selection = preedit_selection.extend_to_cursor(Cursor::from_index(
                &self.layout,
                text_range.end,
                Affinity::Downstream,
            ));

            preedit_selection.geometry_with(&self.layout, |rect| {
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
    }

    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.update_layout(size.width as f32, 1.0);
                self.selection = self.selection.refresh(&self.layout);
                self.set_ime_cursor_area(window);
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
                    Ime::Enabled => {}
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
                    Ime::Preedit(text, _compose_cursor) => {
                        if let Some(text_range) = self.preedit_range.take() {
                            self.buffer.replace_range(text_range.clone(), "");

                            // Invariant: the selection anchor and start of preedit text are at the same
                            // position.
                            // If the focus extends into the preedit range, shrink the selection.
                            if self.selection.focus().text_range().start > text_range.start {
                                self.selection = shrink_selection(
                                    &self.layout,
                                    self.selection,
                                    text_range.len(),
                                );
                            }
                        }

                        if let Some(start) = self.delete_current_selection() {
                            self.selection =
                                Selection::from_index(&self.layout, start, Affinity::Downstream);
                        }

                        let insertion_index = self.selection.insertion_index();
                        self.buffer.insert_str(insertion_index, text);
                        self.preedit_range = Some(insertion_index..insertion_index + text.len());

                        self.update_layout(self.width, 1.0);

                        // winit says the cursor should be hidden when compose_cursor is None.
                        // Do we handle that? We also don't set the cursor based on the cursor
                        // indicated by winit, instead IME composing is currently indicated by
                        // underlining the entire preedit text, and the IME candidate box
                        // placement is based on the preedit text location.
                        self.selection = Selection::from_index(
                            &self.layout,
                            self.selection.insertion_index(),
                            Affinity::Downstream,
                        );
                        self.set_ime_cursor_area(window);
                    }
                    Ime::Disabled => {
                        if let Some(text_range) = self.preedit_range.take() {
                            self.buffer.replace_range(text_range.clone(), "");
                            self.update_layout(self.width, 1.0);

                            // Invariant: the selection anchor and start of preedit text are at the same
                            // position.
                            // If the focus extends into the preedit range, shrink the selection.
                            if self.selection.focus().text_range().start > text_range.start {
                                self.selection = shrink_selection(
                                    &self.layout,
                                    self.selection,
                                    text_range.len(),
                                );
                            } else {
                                self.selection = self.selection.refresh(&self.layout);
                            }
                        }
                    }
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

        if let Some(ref text_range) = self.preedit_range {
            if text_range.start != self.selection.anchor().text_range().start {
                // If the selection anchor is no longer at the same position as the preedit text, the
                // selection has been moved. Move the preedit to the selection's new anchor position.

                // TODO: we can be smarter here to prevent need of the String allocation
                let text = self.buffer[text_range.clone()].to_owned();
                self.buffer.replace_range(text_range.clone(), "");

                if self.selection.anchor().text_range().start > text_range.start {
                    // shift the selection to the left to account for the preedit text that was
                    // just removed
                    let anchor = *self.selection.anchor();
                    let focus = *self.selection.focus();
                    let shift = text_range
                        .len()
                        .min(anchor.text_range().start - text_range.start);
                    self.selection = Selection::from_index(
                        &self.layout,
                        anchor.index() - shift,
                        anchor.affinity(),
                    );
                    self.selection = self.selection.extend_to_cursor(Cursor::from_index(
                        &self.layout,
                        focus.index() - shift,
                        focus.affinity(),
                    ));
                }

                let insertion_index = self.selection.insertion_index();
                self.buffer.insert_str(insertion_index, &text);
                self.preedit_range = Some(insertion_index..insertion_index + text.len());

                // TODO: events that caused the preedit to be moved may also have updated the
                // layout, in that case we're now updating twice.
                self.update_layout(self.width, 1.0);

                self.set_ime_cursor_area(window);
                self.selection = self.selection.refresh(&self.layout);
            }
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
