use std::{sync::Arc, time::Instant};

use egui::{mutex::Mutex, ViewportInfo};
use parking_lot::RwLock;
use tokio::runtime::{Builder, Runtime};
use wgpu::InstanceFlags;
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
    window::Window,
};

use crate::{
    emulator::EmulatorWrapper,
    error::Result,
    event::{EventResult, UserEvent},
    renderer::Renderer,
    server::Server,
    state::State,
    update_flag::UpdateFlag,
};

pub struct Simulator {
    runtime: Runtime,
    server: Option<Server>,
    emulator: EmulatorWrapper,
    instance: wgpu::Instance,
    repaint_proxy: Arc<Mutex<EventLoopProxy<UserEvent>>>,
    windows_next_repaint_time: Option<Instant>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    run_result: Result<()>,
    update_flag: UpdateFlag,
    state: State,
}

impl Simulator {
    pub fn run(event_loop: winit::event_loop::EventLoop<UserEvent>, state: State) -> Result<State> {
        let runtime = Builder::new_multi_thread().enable_all().build()?;

        let rx_buf = Arc::new(RwLock::default());
        let server = Server::new(
            &runtime,
            8080,
            true,
            rx_buf.clone(),
            event_loop.create_proxy(),
        )?;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: if state.debug {
                InstanceFlags::VALIDATION | InstanceFlags::GPU_BASED_VALIDATION
            } else {
                InstanceFlags::empty()
            },
            ..Default::default()
        });

        let mut app = Self {
            runtime,
            instance,
            repaint_proxy: Arc::new(Mutex::new(event_loop.create_proxy())),
            server: Some(server),
            emulator: EmulatorWrapper::new(rx_buf),
            windows_next_repaint_time: None,
            window: None,
            renderer: None,
            run_result: Ok(()),
            update_flag: UpdateFlag::empty(),
            state,
        };

        event_loop.run_app(&mut app)?;

        app.run_result?;

        Ok(app.state)
    }

    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let egui_ctx = Renderer::create_egui_context();
        let window = self.create_window(&egui_ctx, event_loop)?;
        self.init_run_state(egui_ctx, window)?;
        Ok(())
    }

    fn create_window(
        &self,
        egui_ctx: &egui::Context,
        event_loop: &ActiveEventLoop,
    ) -> Result<Window> {
        tracing::info!("Initializing window...");
        let viewport_builder = egui::ViewportBuilder::default()
            .with_inner_size([self.state.window_size.0 as _, self.state.window_size.1 as _])
            .with_visible(false);
        let window = egui_winit::create_window(egui_ctx, event_loop, &viewport_builder)?;
        Ok(window)
    }

    fn init_run_state(&mut self, egui_ctx: egui::Context, window: Window) -> Result<()> {
        let window = Arc::new(window);

        {
            let event_loop_proxy = self.repaint_proxy.clone();
            egui_ctx.set_request_repaint_callback(move |info| {
                let when = Instant::now() + info.delay;
                let cumulative_pass_nr = info.current_cumulative_pass_nr;
                event_loop_proxy
                    .lock()
                    .send_event(UserEvent::RequestRepaint {
                        when,
                        cumulative_pass_nr,
                    })
                    .ok();
            });
        }

        let mut info = ViewportInfo::default();
        egui_winit::update_viewport_info(&mut info, &egui_ctx, &window, true);

        self.renderer = Some(self.runtime.block_on(Renderer::new(
            &self.instance,
            egui_ctx,
            window.clone(),
            self.state.window_size.0,
            self.state.window_size.1,
            &self.state,
        ))?);
        self.window = Some(window);

        Ok(())
    }

    fn update(&mut self, event: Option<&UserEvent>) {
        let system_time = self.state.system_time();
        self.emulator.update(system_time);

        match event {
            Some(UserEvent::Server(signal)) => match signal {
                crate::event::Signal::ConfigGeometry(geometry) => {
                    self.emulator.initialize(geometry);
                    self.renderer.as_mut().unwrap().initialize(&self.emulator);

                    self.update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                    self.update_flag.set(UpdateFlag::UPDATE_TRANS_POS, true);
                    self.update_flag.set(UpdateFlag::UPDATE_TRANS_ALPHA, true);
                    self.update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
                    self.update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                    self.update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                    self.update_flag
                        .set(UpdateFlag::UPDATE_SLICE_COLOR_MAP, true);
                    self.update_flag.set(UpdateFlag::UPDATE_CONFIG, true);
                }
                crate::event::Signal::UpdateGeometry(geometry) => {
                    self.emulator.update_geometry(geometry);

                    self.update_flag.set(UpdateFlag::UPDATE_TRANS_POS, true);
                }
                crate::event::Signal::Send(tx) => {
                    self.emulator.send(tx);

                    self.update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
                }
                crate::event::Signal::Close => {
                    self.emulator.clear();
                    tracing::info!("Server is closed by client");
                    tracing::info!(
                        "Waiting for client connection on http://0.0.0.0:{}",
                        self.state.port
                    );
                }
            },
            _ => {}
        }
    }

    fn run_ui_and_paint(&mut self, window: &Window) -> Result<EventResult> {
        let Self {
            renderer,
            state,
            emulator,
            update_flag,
            ..
        } = self;

        if let Some(renderer) = renderer {
            if update_flag.contains(UpdateFlag::UPDATE_CAMERA) {
                renderer.update_camera(state, window);
                update_flag.remove(UpdateFlag::UPDATE_CAMERA);
            }

            if update_flag.contains(UpdateFlag::UPDATE_TRANS_POS) {
                renderer.update_trans_pos(emulator);
                update_flag.remove(UpdateFlag::UPDATE_TRANS_POS);
            }

            if update_flag.contains(UpdateFlag::UPDATE_TRANS_ALPHA)
                | update_flag.contains(UpdateFlag::UPDATE_TRANS_STATE)
            {
                if update_flag.contains(UpdateFlag::UPDATE_TRANS_STATE) {
                    emulator.update_transducers(state.mod_enable);
                    renderer.update_trans_state(emulator);

                    update_flag.remove(UpdateFlag::UPDATE_TRANS_STATE);
                }
                renderer.update_color(emulator);
                update_flag.remove(UpdateFlag::UPDATE_TRANS_ALPHA);
            }

            if update_flag.contains(UpdateFlag::UPDATE_SLICE_POS)
                | update_flag.contains(UpdateFlag::UPDATE_SLICE_SIZE)
            {
                renderer.update_slice(state);
                update_flag.remove(UpdateFlag::UPDATE_SLICE_POS);
                update_flag.remove(UpdateFlag::UPDATE_SLICE_SIZE);
            }

            if update_flag.contains(UpdateFlag::UPDATE_CONFIG) {
                renderer.update_config(state, emulator);
                update_flag.remove(UpdateFlag::UPDATE_CONFIG);
            }

            if update_flag.contains(UpdateFlag::UPDATE_SLICE_COLOR_MAP) {
                renderer.update_color_map(state);
                update_flag.remove(UpdateFlag::UPDATE_SLICE_COLOR_MAP);
            }

            let result = renderer.run_ui_and_paint(state, emulator, window, update_flag)?;

            if emulator.initialized() && state.auto_play {
                return Ok(EventResult::RepaintNow);
            }

            Ok(result)
        } else {
            Ok(EventResult::Wait)
        }
    }

    fn on_resumed(&mut self, event_loop: &ActiveEventLoop) -> Result<EventResult> {
        if self.window.is_none() {
            self.initialize(event_loop)?;
        }
        Ok(EventResult::RepaintNow)
    }

    fn on_window_event(&mut self, event: winit::event::WindowEvent) -> Result<EventResult> {
        self.update(None);
        if let Some(window) = self.window.as_ref().cloned() {
            match event {
                winit::event::WindowEvent::RedrawRequested => self.run_ui_and_paint(&window),
                _ => {
                    if let Some(renderer) = &mut self.renderer {
                        Ok(renderer.on_window_event(&event, &window, &self.state))
                    } else {
                        Ok(EventResult::Wait)
                    }
                }
            }
        } else {
            Ok(EventResult::Wait)
        }
    }

    fn on_user_event(&mut self, event: UserEvent) -> Result<EventResult> {
        self.update(Some(&event));
        if let Some(renderer) = &mut self.renderer {
            return Ok(renderer.on_user_event(&event));
        }
        Ok(EventResult::Wait)
    }

    fn on_device_event(&mut self, event: winit::event::DeviceEvent) -> Result<EventResult> {
        self.update(None);
        if let Some(renderer) = &mut self.renderer {
            Ok(renderer.on_device_event(event))
        } else {
            Ok(EventResult::Wait)
        }
    }

    fn handle_event_result(
        &mut self,
        event_loop: &ActiveEventLoop,
        event_result: Result<EventResult>,
    ) {
        let mut exit = false;

        let combined_result = event_result.and_then(|event_result| match event_result {
            EventResult::Wait => {
                event_loop.set_control_flow(ControlFlow::Wait);
                Ok(event_result)
            }
            EventResult::RepaintNow => {
                if cfg!(target_os = "windows") {
                    if let Some(ref window) = self.window.as_ref().cloned() {
                        self.update(None);
                        self.run_ui_and_paint(window)
                    } else {
                        Ok(event_result)
                    }
                } else {
                    self.windows_next_repaint_time = Some(Instant::now());
                    Ok(event_result)
                }
            }
            EventResult::RepaintNext => {
                self.windows_next_repaint_time = Some(Instant::now());
                Ok(event_result)
            }
            EventResult::RepaintAt(repaint_time) => {
                self.windows_next_repaint_time = Some(
                    self.windows_next_repaint_time
                        .map_or(repaint_time, |last| last.min(repaint_time)),
                );
                Ok(event_result)
            }
            EventResult::Exit => {
                exit = true;
                Ok(event_result)
            }
        });

        if let Err(err) = combined_result {
            exit = true;
            self.run_result = Err(err);
        };

        if exit {
            event_loop.exit();
        }

        self.check_redraw_requests(event_loop);
    }

    fn check_redraw_requests(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if let Some(next_repaint_time) = self.windows_next_repaint_time {
            if now >= next_repaint_time {
                self.windows_next_repaint_time = None;
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(next_repaint_time));
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for Simulator {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let event_result = self.on_resumed(event_loop);
        self.handle_event_result(event_loop, event_result);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let event_result = self.on_window_event(event);
        self.handle_event_result(event_loop, event_result);
    }

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::event::StartCause,
    ) {
        self.check_redraw_requests(event_loop);
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: UserEvent) {
        let event_result = self.on_user_event(event);
        self.handle_event_result(event_loop, event_result);
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let event_result = self.on_device_event(event);
        self.handle_event_result(event_loop, event_result);
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.handle_event_result(event_loop, Ok(EventResult::Wait));
    }

    fn exiting(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        if let Some(server) = self.server.take() {
            tracing::info!("Shutting down server...");
            let r = self.runtime.block_on(server.shutdown());
            if let Err(err) = r {
                tracing::error!("Failed to shutdown server: {:?}", err);
            } else {
                tracing::info!("Shutting down server...done");
            }
        }
    }
}
