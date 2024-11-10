// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use accesskit::{Node, Rect, Role, Tree, TreeUpdate};
use anyhow::Result;
use peniko::Color;
#[cfg(feature = "tiny-skia")]
use softbuffer::{Context, Surface};
#[cfg(not(feature = "vello"))]
use std::marker::PhantomData;
#[cfg(feature = "tiny-skia")]
use std::num::NonZeroU32;
#[cfg(feature = "vello")]
use std::num::NonZeroUsize;
use std::sync::Arc;
#[cfg(feature = "tiny-skia")]
use tiny_skia::PixmapMut;
#[cfg(feature = "vello")]
use vello::util::{RenderContext, RenderSurface};
#[cfg(feature = "vello")]
use vello::wgpu;
#[cfg(feature = "vello")]
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::*;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::Window;

mod access_ids;
use access_ids::{TEXT_INPUT_ID, WINDOW_ID};

#[cfg(feature = "tiny-skia")]
mod tiny_skia_util;
#[cfg(feature = "tiny-skia")]
use tiny_skia_util::*;

// #[path = "text2.rs"]
mod text;
use parley::{GenericFamily, StyleProperty};

const BACKGROUND_COLOR: Color = Color::rgb8(30, 30, 30);
const WINDOW_TITLE: &str = "Text Editor";

// Simple struct to hold the state of the renderer
pub struct ActiveRenderState<'s> {
    // The fields MUST be in this order, so that the surface and AccessKit adapter are dropped before the window
    #[cfg(feature = "vello")]
    surface: RenderSurface<'s>,
    #[cfg(feature = "tiny-skia")]
    surface: Surface<Arc<Window>, Arc<Window>>,
    access_adapter: accesskit_winit::Adapter,
    window: Arc<Window>,
    sent_initial_access_update: bool,
    #[cfg(not(feature = "vello"))]
    _marker: PhantomData<&'s ()>,
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
            node.set_bounds(Rect {
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
    #[cfg(feature = "vello")]
    context: RenderContext,

    /// An array of renderers, one per wgpu device.
    #[cfg(feature = "vello")]
    renderers: Vec<Option<Renderer>>,

    /// State for our example where we store the winit Window and the wgpu Surface.
    state: RenderState<'s>,

    /// A `vello::Scene` where the editor layout will be drawn.
    #[cfg(feature = "vello")]
    scene: Scene,

    /// Our `Editor`, which owns a `parley::PlainEditor`.
    editor: text::Editor,

    /// The last generation of the editor layout that we drew.
    last_drawn_generation: text::Generation,

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

        let size = window.inner_size();

        self.editor.transact(|txn| {
            txn.set_scale(1.0);
            txn.set_width(Some(size.width as f32 - 2f32 * text::INSET));
            txn.set_text(text::LOREM);
        });

        #[cfg(feature = "vello")]
        // Create a vello Surface
        let surface = {
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
            pollster::block_on(surface_future).expect("Error creating surface")
        };

        #[cfg(feature = "tiny-skia")]
        let surface = {
            let context = Context::new(Arc::clone(&window)).unwrap();
            Surface::new(&context, Arc::clone(&window)).unwrap()
        };

        #[cfg(feature = "vello")]
        {
            // Create a vello Renderer for the surface (using its device id)
            self.renderers
                .resize_with(self.context.devices.len(), || None);

            self.renderers[surface.dev_id]
                .get_or_insert_with(|| create_vello_renderer(&self.context, &surface));
        }

        // Save the Window and Surface to a state variable
        self.state = RenderState::Active(ActiveRenderState {
            window,
            surface,
            access_adapter,
            sent_initial_access_update: false,
            #[cfg(not(feature = "vello"))]
            _marker: PhantomData,
        });

        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if let RenderState::Active(state) = &self.state {
            self.state = RenderState::Suspended(Some(state.window.clone()));
        }
        event_loop.set_control_flow(ControlFlow::Wait);
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
        }
        // render_state
        //     .window
        //     .set_cursor(winit::window::Cursor::Icon(winit::window::CursorIcon::Text));

        match event {
            // Exit the event loop when a close is requested (e.g. window's close button is pressed)
            WindowEvent::CloseRequested => event_loop.exit(),

            // Resize the surface when the window is resized
            WindowEvent::Resized(size) => {
                #[cfg(feature = "vello")]
                self.context
                    .resize_surface(&mut render_state.surface, size.width, size.height);
                #[cfg(feature = "tiny-skia")]
                render_state
                    .surface
                    .resize(
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    )
                    .unwrap();
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
                // Send an accessibility update if accessibility is active.
                render_state.access_update(&mut self.editor);

                #[cfg(feature = "vello")]
                {
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
                                base_color: BACKGROUND_COLOR,
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

                #[cfg(feature = "tiny-skia")]
                {
                    let mut buffer = render_state.surface.buffer_mut().unwrap();
                    let size = render_state.window.inner_size();
                    let mut pixmap = PixmapMut::from_bytes(
                        bytemuck::cast_slice_mut(&mut buffer),
                        size.width,
                        size.height,
                    )
                    .unwrap();
                    pixmap.fill(to_tiny_skia_color(BACKGROUND_COLOR));
                    self.last_drawn_generation = self.editor.draw(&mut pixmap);
                    // Swap the red and blue bytes, since tiny-skia and
                    // softbuffer can't agree on a pixel format. Adapted
                    // from iced_tiny_skia.
                    for pixel in buffer.iter_mut() {
                        *pixel = *pixel & 0xFF00_FF00
                            | ((0x0000_00FF & *pixel) << 16)
                            | ((0x00FF_0000 & *pixel) >> 16);
                    }
                    buffer.present().unwrap();
                }
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
        #[cfg(feature = "vello")]
        context: RenderContext::new(),
        #[cfg(feature = "vello")]
        renderers: vec![],
        state: RenderState::Suspended(None),
        #[cfg(feature = "vello")]
        scene: Scene::new(),
        editor: text::Editor::default(),
        last_drawn_generation: Default::default(),
        event_loop_proxy: event_loop.create_proxy(),
    };

    // Run the winit event loop
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
        .with_title(WINDOW_TITLE)
        .with_visible(false);
    Arc::new(event_loop.create_window(attr).unwrap())
}

/// Helper function that creates a vello `Renderer` for a given `RenderContext` and `RenderSurface`
#[cfg(feature = "vello")]
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
