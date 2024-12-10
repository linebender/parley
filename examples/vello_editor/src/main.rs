// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(missing_debug_implementations)]
#![allow(missing_docs)]
#![allow(unreachable_pub)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::unseparated_literal_suffix)]

use accesskit::{Node, Role, Tree, TreeUpdate};
use anyhow::Result;
use std::num::NonZeroUsize;
use std::sync::Arc;
use vello::kurbo;
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::Window;

mod access_ids;
use access_ids::{TEXT_INPUT_ID, WINDOW_ID};

mod text;

const WINDOW_TITLE: &str = "Vello Text Editor";

// Simple struct to hold the state of the renderer
pub struct ActiveRenderState<'s> {
    // The fields MUST be in this order, so that the surface and AccessKit adapter are dropped before the window
    surface: RenderSurface<'s>,
    access_adapter: accesskit_winit::Adapter,
    window: Arc<Window>,
    sent_initial_access_update: bool,
}

impl ActiveRenderState<'_> {
    fn access_update(&mut self, editor: &mut text::Editor) {
        self.access_adapter.update_if_active(|| {
            let mut update = TreeUpdate {
                nodes: vec![],
                tree: (!self.sent_initial_access_update).then(|| Tree::new(WINDOW_ID)),
                focus: TEXT_INPUT_ID,
            };
            if !self.sent_initial_access_update {
                let mut node = Node::new(Role::Window);
                node.set_label(WINDOW_TITLE);
                node.push_child(TEXT_INPUT_ID);
                update.nodes.push((WINDOW_ID, node));
                self.sent_initial_access_update = true;
            }
            let mut node = Node::new(Role::TextInput);
            let size = self.window.inner_size();
            node.set_bounds(accesskit::Rect {
                x0: 0.0,
                y0: 0.0,
                x1: size.width as _,
                y1: size.height as _,
            });
            editor.accessibility(&mut update, &mut node);
            update.nodes.push((TEXT_INPUT_ID, node));
            update
        });
    }
}

enum RenderState<'s> {
    Active(ActiveRenderState<'s>),
    // Cache a window so that it can be reused when the app is resumed after being suspended
    Suspended(Option<Arc<Window>>),
}

struct SimpleVelloApp<'s> {
    /// The vello `RenderContext` which is a global context that lasts for the
    /// lifetime of the application.
    context: RenderContext,

    /// An array of renderers, one per wgpu device.
    renderers: Vec<Option<Renderer>>,

    /// State for our example where we store the winit Window and the wgpu Surface.
    state: RenderState<'s>,

    /// A `vello::Scene` where the editor layout will be drawn.
    scene: Scene,

    /// Our `Editor`, which owns a `parley::PlainEditor`.
    editor: text::Editor,

    /// The last generation of the editor layout that we drew.
    last_drawn_generation: text::Generation,

    /// The IME cursor area we last sent to the platform.
    last_sent_ime_cursor_area: kurbo::Rect,

    /// The event loop proxy required by the AccessKit winit adapter.
    event_loop_proxy: EventLoopProxy<accesskit_winit::Event>,
}

impl ApplicationHandler<accesskit_winit::Event> for SimpleVelloApp<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };

        // Get the winit window cached in a previous Suspended event or else create a new window
        let window = cached_window
            .take()
            .unwrap_or_else(|| create_winit_window(event_loop));
        let access_adapter =
            accesskit_winit::Adapter::with_event_loop_proxy(&window, self.event_loop_proxy.clone());
        window.set_visible(true);
        window.set_ime_allowed(true);

        let size = window.inner_size();

        // Create a vello Surface
        let surface_future = {
            let surface = self
                .context
                .instance
                .create_surface(wgpu::SurfaceTarget::from(window.clone()))
                .expect("Error creating surface");
            let dev_id = pollster::block_on(self.context.device(Some(&surface)))
                .expect("No compatible device");
            let device_handle = &self.context.devices[dev_id];
            let capabilities = surface.get_capabilities(device_handle.adapter());
            let present_mode = if capabilities
                .present_modes
                .contains(&wgpu::PresentMode::Mailbox)
            {
                wgpu::PresentMode::Mailbox
            } else {
                wgpu::PresentMode::AutoVsync
            };

            self.context
                .create_render_surface(surface, size.width, size.height, present_mode)
        };
        let surface = pollster::block_on(surface_future).expect("Error creating surface");

        // Create a vello Renderer for the surface (using its device id)
        self.renderers
            .resize_with(self.context.devices.len(), || None);

        self.renderers[surface.dev_id]
            .get_or_insert_with(|| create_vello_renderer(&self.context, &surface));

        // Save the Window and Surface to a state variable
        self.state = RenderState::Active(ActiveRenderState {
            window,
            surface,
            access_adapter,
            sent_initial_access_update: false,
        });

        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if let RenderState::Active(state) = &self.state {
            self.state = RenderState::Suspended(Some(state.window.clone()));
        }
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        match cause {
            StartCause::Init => {
                self.editor.cursor_reset();
                if let Some(next_time) = self.editor.next_blink_time() {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(next_time));
                }
            }
            StartCause::ResumeTimeReached { .. } => {
                self.editor.cursor_blink();

                if let Some(next_time) = self.editor.next_blink_time() {
                    self.last_drawn_generation = text::Generation::default();
                    if let RenderState::Active(state) = &self.state {
                        state.window.request_redraw();
                    }
                    event_loop.set_control_flow(ControlFlow::WaitUntil(next_time));
                }
            }
            StartCause::WaitCancelled { .. } => {
                if let Some(next_time) = self.editor.next_blink_time() {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(next_time));
                }
            }
            _ => {}
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Ignore the event (return from the function) if
        //   - we have no render_state
        //   - OR the window id of the event doesn't match the window id of our render_state
        //
        // Else extract a mutable reference to the render state from its containing option for use below
        let render_state = match &mut self.state {
            RenderState::Active(state) if state.window.id() == window_id => state,
            _ => return,
        };

        render_state
            .access_adapter
            .process_event(&render_state.window, &event);
        self.editor.handle_event(event.clone());
        if self.last_drawn_generation != self.editor.generation() {
            render_state.window.request_redraw();
            let area = self.editor.editor().ime_cursor_area();
            if self.last_sent_ime_cursor_area != area {
                self.last_sent_ime_cursor_area = area;
                // Note: on X11 `set_ime_cursor_area` may cause the exclusion area to be obscured
                // until https://github.com/rust-windowing/winit/pull/3966 is in the Winit release
                // used by this example.
                render_state.window.set_ime_cursor_area(
                    PhysicalPosition::new(
                        area.x0 + text::INSET as f64,
                        area.y0 + text::INSET as f64,
                    ),
                    PhysicalSize::new(area.width(), area.height()),
                );
            }
        }
        // render_state
        //     .window
        //     .set_cursor(winit::window::Cursor::Icon(winit::window::CursorIcon::Text));

        match event {
            // Exit the event loop when a close is requested (e.g. window's close button is pressed)
            WindowEvent::CloseRequested => event_loop.exit(),

            // Resize the surface when the window is resized
            WindowEvent::Resized(size) => {
                self.context
                    .resize_surface(&mut render_state.surface, size.width, size.height);
                let editor = self.editor.editor();
                editor.set_scale(1.0);
                editor.set_width(Some(size.width as f32 - 2f32 * text::INSET));
                render_state.window.request_redraw();
            }

            // Don't blink the cursor when we're not focused.
            WindowEvent::Focused(false) => {
                self.editor.disable_blink();
                self.editor.cursor_blink();
                self.last_drawn_generation = text::Generation::default();
                if let RenderState::Active(state) = &self.state {
                    state.window.request_redraw();
                }
            }
            // Make sure cursor is visible when we regain focus.
            WindowEvent::Focused(true) => self.editor.cursor_reset(),

            // This is where all the rendering happens
            WindowEvent::RedrawRequested => {
                // Send an accessibility update if accessibility is active.
                render_state.access_update(&mut self.editor);

                // Get the RenderSurface (surface + config).
                let surface = &render_state.surface;

                // Get the window size.
                let width = surface.config.width;
                let height = surface.config.height;

                // Get a handle to the device.
                let device_handle = &self.context.devices[surface.dev_id];

                // Get the surface's texture.
                let surface_texture = surface
                    .surface
                    .get_current_texture()
                    .expect("failed to get surface texture");

                // Sometimes `Scene` is stale and needs to be redrawn.
                if self.last_drawn_generation != self.editor.generation() {
                    // Empty the scene of objects to draw. You could create a new Scene each time, but in this case
                    // the same Scene is reused so that the underlying memory allocation can also be reused.
                    self.scene.reset();

                    self.last_drawn_generation = self.editor.draw(&mut self.scene);
                }

                // Render to the surface's texture.
                self.renderers[surface.dev_id]
                    .as_mut()
                    .unwrap()
                    .render_to_surface(
                        &device_handle.device,
                        &device_handle.queue,
                        &self.scene,
                        &surface_texture,
                        &vello::RenderParams {
                            base_color: Color::rgb8(30, 30, 30), // Background color
                            width,
                            height,
                            antialiasing_method: AaConfig::Area,
                        },
                    )
                    .expect("failed to render to surface");

                // Queue the texture to be presented on the surface.
                surface_texture.present();

                device_handle.device.poll(wgpu::Maintain::Poll);
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: accesskit_winit::Event) {
        let render_state = match &mut self.state {
            RenderState::Active(state) if state.window.id() == event.window_id => state,
            _ => return,
        };

        match event.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                render_state.access_update(&mut self.editor);
            }
            accesskit_winit::WindowEvent::ActionRequested(req) => {
                if req.target == TEXT_INPUT_ID {
                    self.editor.handle_accesskit_action_request(&req);
                    if self.last_drawn_generation != self.editor.generation() {
                        render_state.window.request_redraw();
                    }
                }
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                render_state.sent_initial_access_update = false;
            }
        }
    }
}

fn main() -> Result<()> {
    // Create a winit event loop:
    let event_loop = EventLoop::with_user_event().build()?;

    // Setup a bunch of state:
    let mut app = SimpleVelloApp {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        editor: text::Editor::new(text::LOREM),
        last_drawn_generation: Default::default(),
        last_sent_ime_cursor_area: kurbo::Rect::new(f64::NAN, f64::NAN, f64::NAN, f64::NAN),
        event_loop_proxy: event_loop.create_proxy(),
    };

    // Run the winit event loop
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
    let text = app.editor.text();
    print!("{text}");
    Ok(())
}

/// Helper function that creates a Winit window and returns it (wrapped in an Arc for sharing between threads)
fn create_winit_window(event_loop: &ActiveEventLoop) -> Arc<Window> {
    let attr = Window::default_attributes()
        .with_inner_size(LogicalSize::new(1044, 800))
        .with_resizable(true)
        .with_title(WINDOW_TITLE)
        .with_visible(false);
    Arc::new(event_loop.create_window(attr).unwrap())
}

/// Helper function that creates a vello `Renderer` for a given `RenderContext` and `RenderSurface`
fn create_vello_renderer(render_cx: &RenderContext, surface: &RenderSurface<'_>) -> Renderer {
    Renderer::new(
        &render_cx.devices[surface.dev_id].device,
        RendererOptions {
            surface_format: Some(surface.format),
            use_cpu: false,
            antialiasing_support: vello::AaSupport::all(),
            num_init_threads: NonZeroUsize::new(1),
        },
    )
    .expect("Couldn't create renderer")
}
