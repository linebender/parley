// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use anyhow::Result;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::*;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::Window;

// #[path = "text2.rs"]
mod text;
use parley::{GenericFamily, StyleProperty};

// Simple struct to hold the state of the renderer
pub struct ActiveRenderState<'s> {
    // The fields MUST be in this order, so that the surface is dropped before the window
    surface: RenderSurface<'s>,
    window: Arc<Window>,
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
}

impl ApplicationHandler for SimpleVelloApp<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };

        // Get the winit window cached in a previous Suspended event or else create a new window
        let window = cached_window
            .take()
            .unwrap_or_else(|| create_winit_window(event_loop));

        let size = window.inner_size();

        self.editor.transact(|txn| {
            txn.set_scale(1.0);
            txn.set_width(Some(size.width as f32 - 2f32 * text::INSET));
            txn.set_text(text::LOREM);
        });

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
        self.state = RenderState::Active(ActiveRenderState { window, surface });
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
                self.editor.init();
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

        self.editor.handle_event(event.clone());
        if self.last_drawn_generation != self.editor.generation() {
            render_state.window.request_redraw();
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
                self.editor.transact(|txn| {
                    txn.set_scale(1.0);
                    txn.set_width(Some(size.width as f32 - 2f32 * text::INSET));
                    txn.set_default_style(Arc::new([
                        StyleProperty::FontSize(32.0),
                        StyleProperty::LineHeight(1.2),
                        GenericFamily::SystemUi.into(),
                    ]));
                });
                render_state.window.request_redraw();
            }

            // This is where all the rendering happens
            WindowEvent::RedrawRequested => {
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
                            antialiasing_method: AaConfig::Msaa16,
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
}

fn main() -> Result<()> {
    // Setup a bunch of state:
    let mut app = SimpleVelloApp {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        editor: text::Editor::default(),
        last_drawn_generation: Default::default(),
    };

    // Create and run a winit event loop
    let event_loop = EventLoop::new()?;
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
    print!("{}", app.editor.text());
    Ok(())
}

/// Helper function that creates a Winit window and returns it (wrapped in an Arc for sharing between threads)
fn create_winit_window(event_loop: &ActiveEventLoop) -> Arc<Window> {
    let attr = Window::default_attributes()
        .with_inner_size(LogicalSize::new(1044, 800))
        .with_resizable(true)
        .with_title("Vello Text Editor");
    Arc::new(event_loop.create_window(attr).unwrap())
}

/// Helper function that creates a vello `Renderer` for a given `RenderContext` and `RenderSurface`
fn create_vello_renderer(render_cx: &RenderContext, surface: &RenderSurface) -> Renderer {
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
