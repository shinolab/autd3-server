use std::sync::Arc;

use autd3_driver::ethercat::DcSysTime;
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    keyboard::{Key, NamedKey},
    window::Window,
};

use crate::{
    context::Context, error::SimulatorError, renderer::Renderer, server::ServerWrapper,
    update_flag::UpdateFlag,
};
use crate::{state::State, surface::SurfaceWrapper};

pub struct Simulator {
    event_loop: EventLoop<()>,
    window: Arc<Window>,
    surface: SurfaceWrapper,
    context: Context,
    renderer: Renderer,
    server: ServerWrapper,
    state: State,
}

impl Simulator {
    pub async fn new(mut state: State) -> Result<Self, SimulatorError> {
        state.real_time = DcSysTime::now().sys_time();
        let event_loop = EventLoop::new()?;
        let window = Arc::new(
            winit::window::WindowBuilder::new()
                .with_title("AUTD3 Simulator")
                .with_inner_size(PhysicalSize::new(state.window_size.0, state.window_size.1))
                .build(&event_loop)?,
        );
        let mut surface = SurfaceWrapper::new();
        let context = Context::init(&mut surface, window.clone()).await?;
        let renderer = Renderer::new(None, &context, &surface, window.clone())?;
        let server = ServerWrapper::new(state.port, state.lightweight, state.lightweight_port);
        Ok(Self {
            event_loop,
            window,
            surface,
            context,
            renderer,
            server,
            state,
        })
    }

    pub fn run(self) -> Result<State, SimulatorError> {
        let Self {
            event_loop,
            window,
            mut surface,
            mut renderer,
            context,
            mut server,
            mut state,
        } = self;

        let mut update_flag = UpdateFlag::empty();
        event_loop.set_control_flow(ControlFlow::Poll);
        let res = event_loop.run(|event, elwt| {
            if let Err(e) = Self::event_loop(
                &mut surface,
                &mut renderer,
                &context,
                &mut state,
                &window,
                &mut server,
                &mut update_flag,
                event,
                elwt,
            ) {
                tracing::error!("{}", e);
                elwt.exit();
            }
        });

        server.shutdown();

        res?;

        Ok(state)
    }

    #[allow(clippy::too_many_arguments)]
    fn event_loop(
        surface: &mut SurfaceWrapper,
        renderer: &mut Renderer,
        context: &Context,
        state: &mut State,
        window: &Arc<Window>,
        server: &mut ServerWrapper,
        update_flag: &mut UpdateFlag,
        event: Event<()>,
        target: &EventLoopWindowTarget<()>,
    ) -> Result<(), SimulatorError> {
        state.update();
        server.update(update_flag, renderer, context, state)?;

        #[allow(clippy::single_match)]
        match event {
            Event::WindowEvent {
                event: ref window_event,
                ..
            } => match window_event {
                WindowEvent::Resized(size) => {
                    tracing::info!("Surface resize {size:?}");
                    state.window_size = (size.width, size.height);
                    surface.resize(context, size);
                    renderer.resize(context, state, surface, window.clone(), &event);
                    window.request_redraw();
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            logical_key: Key::Named(NamedKey::Escape),
                            ..
                        },
                    ..
                }
                | WindowEvent::CloseRequested => {
                    target.exit();
                }
                WindowEvent::RedrawRequested => {
                    let frame = surface.acquire(context)?;
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                        ..wgpu::TextureViewDescriptor::default()
                    });

                    let mut encoder: wgpu::CommandEncoder = context
                        .device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                    renderer.render(
                        state,
                        context,
                        window.clone(),
                        &view,
                        update_flag,
                        &mut encoder,
                    )?;

                    context.queue().submit(Some(encoder.finish()));

                    frame.present();

                    window.request_redraw();
                }
                _ => {}
            },
            _ => {}
        }

        renderer.handle_event(window.clone(), &event);

        Ok(())
    }
}
