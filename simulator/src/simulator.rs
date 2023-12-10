/*
 * File: simulator.rs
 * Project: src
 * Created Date: 24/05/2023
 * Author: Shun Suzuki
 * -----
 * Last Modified: 22/11/2023
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2023 Shun Suzuki. All rights reserved.
 *
 */

use std::{
    error::Error,
    f32::consts::PI,
    ffi::OsStr,
    net::ToSocketAddrs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::{
    common::transform::{to_gl_pos, to_gl_rot},
    field_compute_pipeline::{Config, FieldComputePipeline},
    imgui_renderer::ImGuiRenderer,
    renderer::Renderer,
    slice_viewer::SliceViewer,
    sound_sources::{Drive, SoundSources},
    trans_viewer::TransViewer,
    update_flag::UpdateFlag,
    viewer_settings::ViewerSettings,
    Quaternion, Vector3, MILLIMETER,
};
use autd3_driver::cpu::TxDatagram;
use autd3_firmware_emulator::{CPUEmulator, FPGAEmulator};
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo},
    sync::GpuFuture,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
};

use futures_util::future::FutureExt;
use tokio::{runtime::Builder, sync::oneshot};
use tonic::{transport::Server, Request, Response, Status};

use autd3_protobuf::*;

enum Signal {
    ConfigGeometry(Geometry),
    UpdateGeometry(Geometry),
    Send(TxRawData),
    Close,
}

struct SimulatorServer {
    rx_buf: Arc<RwLock<Vec<autd3_driver::cpu::RxMessage>>>,
    sender: Sender<Signal>,
}

#[tonic::async_trait]
impl simulator_server::Simulator for SimulatorServer {
    async fn config_geomety(
        &self,
        req: Request<Geometry>,
    ) -> Result<Response<GeometryResponse>, Status> {
        if self
            .sender
            .send(Signal::ConfigGeometry(req.into_inner()))
            .is_err()
        {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(GeometryResponse {}))
    }

    async fn update_geomety(
        &self,
        req: Request<Geometry>,
    ) -> Result<Response<GeometryResponse>, Status> {
        if self
            .sender
            .send(Signal::UpdateGeometry(req.into_inner()))
            .is_err()
        {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(GeometryResponse {}))
    }

    async fn send_data(&self, req: Request<TxRawData>) -> Result<Response<SendResponse>, Status> {
        if self.sender.send(Signal::Send(req.into_inner())).is_err() {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(SendResponse { success: true }))
    }

    async fn read_data(&self, _: Request<ReadRequest>) -> Result<Response<RxMessage>, Status> {
        let rx = self.rx_buf.read().unwrap();
        Ok(Response::new(RxMessage {
            data: rx.iter().flat_map(|c| [c.data, c.ack]).collect(),
        }))
    }

    async fn close(&self, _: Request<CloseRequest>) -> Result<Response<CloseResponse>, Status> {
        if self.sender.send(Signal::Close).is_err() {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(CloseResponse { success: true }))
    }
}

/// AUTD Simulator
#[derive(Default)]
pub struct Simulator {
    window_width: Option<u32>,
    window_height: Option<u32>,
    vsync: Option<bool>,
    port: Option<u16>,
    gpu_idx: Option<i32>,
    settings: ViewerSettings,
    config_path: Option<PathBuf>,
}

impl Simulator {
    pub fn new() -> Self {
        Self {
            window_width: None,
            window_height: None,
            vsync: None,
            port: None,
            gpu_idx: None,
            settings: ViewerSettings::default(),
            config_path: None,
        }
    }

    /// Set window size
    pub const fn with_window_size(mut self, width: u32, height: u32) -> Self {
        self.window_width = Some(width);
        self.window_height = Some(height);
        self
    }

    /// Set vsync
    pub const fn with_vsync(mut self, vsync: bool) -> Self {
        self.vsync = Some(vsync);
        self
    }

    /// Set port
    pub const fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Set GPU index
    ///
    /// # Arguments
    ///
    /// * `gpu_idx` - GPU index. If -1, use the most suitable GPU.
    ///
    pub const fn with_gpu_idx(mut self, gpu_idx: i32) -> Self {
        self.gpu_idx = Some(gpu_idx);
        self
    }

    /// Set viewer settings
    pub fn with_settings(mut self, settings: ViewerSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Set config path where settings are saved
    pub fn with_config_path<S: AsRef<OsStr> + Sized>(mut self, config_path: S) -> Self {
        self.config_path = Some(Path::new(&config_path).to_owned());
        self
    }

    /// Get viewer settings
    pub const fn get_settings(&self) -> &ViewerSettings {
        &self.settings
    }

    /// Run Simulator
    ///
    /// # Returns
    ///
    /// ## Platform-specific
    ///
    /// X11 / Wayland: This function returns 1 upon disconnection from the display server.
    pub fn run(&mut self) -> anyhow::Result<i32> {
        tracing::info!("Initializing window...");

        let (tx, rx) = bounded(32);

        let (tx_shutdown, rx_shutdown) = oneshot::channel::<()>();
        let port = self.port.unwrap_or(self.settings.port);

        let rx_buf = Arc::new(RwLock::new(vec![]));
        let server_th = std::thread::spawn({
            let rx_buf = rx_buf.clone();
            move || {
                tracing::info!("Waiting for client connection on http://0.0.0.0:{}", port);
                let body = async {
                    Server::builder()
                        .add_service(simulator_server::SimulatorServer::new(SimulatorServer {
                            rx_buf,
                            sender: tx,
                        }))
                        .serve_with_shutdown(
                            format!("0.0.0.0:{port}")
                                .to_socket_addrs()
                                .unwrap()
                                .next()
                                .unwrap(),
                            rx_shutdown.map(drop),
                        )
                        .await
                };
                Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(body)
            }
        });

        self.run_simulator(server_th, rx_buf, rx, tx_shutdown)
    }

    fn run_simulator(
        &mut self,
        server_th: std::thread::JoinHandle<Result<(), tonic::transport::Error>>,
        rx_buf: Arc<RwLock<Vec<autd3_driver::cpu::RxMessage>>>,
        receiver: Receiver<Signal>,
        shutdown: tokio::sync::oneshot::Sender<()>,
    ) -> anyhow::Result<i32> {
        let mut event_loop = EventLoopBuilder::<()>::with_user_event().build();

        let mut render = Renderer::new(
            &event_loop,
            "AUTD Simulator",
            self.window_width.unwrap_or(self.settings.window_width) as _,
            self.window_height.unwrap_or(self.settings.window_height) as _,
            self.vsync.unwrap_or(self.settings.vsync),
            self.gpu_idx.unwrap_or(self.settings.gpu_idx),
        )?;

        render.move_camera(&self.settings);

        let mut sources = SoundSources::new();
        let mut cpus: Vec<CPUEmulator> = Vec::new();
        let mut body_pointer = vec![];

        let mut field_compute_pipeline = FieldComputePipeline::new(&render, &self.settings)?;
        let mut slice_viewer = SliceViewer::new(&render, &self.settings)?;
        let model = crate::device_viewer::Model::new()?;
        let mut device_viewer = crate::device_viewer::DeviceViewer::new(&render, &model)?;
        let mut imgui = ImGuiRenderer::new(self.settings.clone(), &self.config_path, &render)?;
        let mut trans_viewer = TransViewer::new(&render)?;

        let mut is_initialized = false;
        let mut is_source_update = false;
        let mut is_running = true;

        let server_th_ref = &server_th;

        let res = event_loop.run_return(move |event, _, control_flow| {
            let mut run_loop = |event, control_flow: &mut ControlFlow| -> anyhow::Result<()> {
                cpus.iter_mut().for_each(CPUEmulator::update);
                if cpus.iter().any(CPUEmulator::should_update) {
                    rx_buf
                        .write()
                        .unwrap()
                        .iter_mut()
                        .zip(cpus.iter())
                        .for_each(|(d, s)| {
                            d.ack = s.ack();
                            d.data = s.rx_data();
                        });
                }

                match receiver.try_recv() {
                    Ok(Signal::ConfigGeometry(geometry)) => {
                        sources.clear();
                        cpus.clear();

                        let geometry = autd3_driver::geometry::Geometry::from_msg(&geometry);
                        geometry.iter().for_each(|dev| {
                            dev.iter().for_each(|tr| {
                                let p = tr.position();
                                let r = tr.rotation();
                                sources.add(
                                    to_gl_pos(Vector3::new(p.x as _, p.y as _, p.z as _)),
                                    to_gl_rot(Quaternion::new(
                                        r.w as _, r.i as _, r.j as _, r.k as _,
                                    )),
                                    Drive::new(1.0, 0.0, 1.0, 40e3, self.settings.sound_speed),
                                    1.0,
                                );
                            });
                        });

                        cpus = geometry
                            .iter()
                            .map(|dev| CPUEmulator::new(dev.idx(), dev.num_transducers()))
                            .collect();

                        body_pointer = [0usize]
                            .into_iter()
                            .chain(geometry.iter().map(|dev| dev.num_transducers()))
                            .scan(0, |state, tr_num| {
                                *state += tr_num;
                                Some(*state)
                            })
                            .collect::<Vec<_>>();

                        *rx_buf.write().unwrap() =
                            vec![
                                autd3_driver::cpu::RxMessage { ack: 0, data: 0 };
                                geometry.num_devices()
                            ];

                        field_compute_pipeline.init(&render, &sources)?;
                        trans_viewer.init(&render, &sources)?;
                        slice_viewer.init(&self.settings);
                        device_viewer.init(&geometry);
                        imgui.init(geometry.num_devices());

                        is_initialized = true;
                    }
                    Ok(Signal::UpdateGeometry(geometry)) => {
                        let geometry = autd3_driver::geometry::Geometry::from_msg(&geometry);
                        geometry
                            .iter()
                            .flat_map(|dev| {
                                dev.iter().map(|tr| {
                                    let p = tr.position();
                                    let r = tr.rotation();
                                    (
                                        Vector3::new(p.x as _, p.y as _, p.z as _),
                                        Quaternion::new(r.w as _, r.i as _, r.j as _, r.k as _),
                                    )
                                })
                            })
                            .enumerate()
                            .for_each(|(i, (p, r))| sources.update_geometry(i, p, r));

                        device_viewer.init(&geometry);
                        field_compute_pipeline.update_source_pos(&sources)?;
                        trans_viewer.update_source_pos(&sources)?;
                    }
                    Ok(Signal::Send(raw)) => {
                        let tx = TxDatagram::from_msg(&raw);
                        cpus.iter_mut().for_each(|cpu| {
                            cpu.send(&tx);
                        });
                        rx_buf
                            .write()
                            .unwrap()
                            .iter_mut()
                            .zip(cpus.iter())
                            .for_each(|(d, s)| {
                                d.ack = s.ack();
                                d.data = s.rx_data();
                            });

                        is_source_update = true;
                    }
                    Ok(Signal::Close) => {
                        is_initialized = false;
                        sources.clear();
                        cpus.clear();
                    }
                    Err(TryRecvError::Empty) => {}
                    _ => {}
                }

                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => {
                        is_running = false;
                        *control_flow = ControlFlow::Exit;
                    }
                    Event::WindowEvent {
                        event: WindowEvent::Resized(..),
                        window_id,
                    } if window_id == render.window().id() => {
                        render.resize();
                        imgui.resized(render.window(), &event);
                    }
                    Event::WindowEvent {
                        event:
                            WindowEvent::ScaleFactorChanged {
                                scale_factor,
                                new_inner_size,
                            },
                        window_id,
                    } if window_id == render.window().id() => {
                        *new_inner_size = render
                            .window()
                            .inner_size()
                            .to_logical::<u32>(render.window().scale_factor())
                            .to_physical(scale_factor);
                        render.resize();
                        let event_imgui: Event<'_, ()> = Event::WindowEvent {
                            window_id,
                            event: WindowEvent::ScaleFactorChanged {
                                scale_factor,
                                new_inner_size,
                            },
                        };
                        imgui.resized(render.window(), &event_imgui);
                    }
                    Event::MainEventsCleared => {
                        imgui.prepare_frame(render.window())?;
                        render.window().request_redraw();
                    }
                    Event::NewEvents(_) => {
                        imgui.update_delta_time();
                        render.window().request_redraw();
                    }
                    Event::RedrawRequested(_) => {
                        let before_pipeline_future = render.start_frame()?;

                        let after_future = {
                            let framebuffer = render.frame_buffer();

                            let mut builder = AutoCommandBufferBuilder::primary(
                                render.command_buffer_allocator(),
                                render.queue().queue_family_index(),
                                CommandBufferUsage::OneTimeSubmit,
                            )?;

                            let clear_values = vec![
                                Some(self.settings.background.into()),
                                Some(1f32.into()),
                                None,
                            ];
                            builder
                                .begin_render_pass(
                                    RenderPassBeginInfo {
                                        clear_values,
                                        ..RenderPassBeginInfo::framebuffer(framebuffer)
                                    },
                                    Default::default(),
                                )?
                                .set_viewport(
                                    0,
                                    [render.viewport().clone()].into_iter().collect(),
                                )?;

                            if is_initialized {
                                render.move_camera(&self.settings);
                                let view = render.get_view();
                                let proj = render.get_projection(&self.settings);
                                let slice_model = slice_viewer.model();
                                if self.settings.view_device {
                                    device_viewer.render(
                                        &model,
                                        (view, proj),
                                        &self.settings,
                                        &imgui.visible(),
                                        &mut builder,
                                    )?;
                                } else {
                                    trans_viewer.render(view, proj, &mut builder)?;
                                }
                                slice_viewer.render(
                                    &render,
                                    view,
                                    proj,
                                    &self.settings,
                                    &mut builder,
                                )?;
                                builder.end_render_pass(Default::default())?;

                                let mut update_flag = imgui.update(
                                    &mut cpus,
                                    &mut sources,
                                    &body_pointer,
                                    &render,
                                    &mut builder,
                                    &mut self.settings,
                                )?;
                                if is_source_update {
                                    update_flag.set(UpdateFlag::UPDATE_SOURCE_DRIVE, true);
                                    is_source_update = false;
                                }

                                if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE) {
                                    cpus.iter().for_each(|cpu| {
                                        let idx = if cpu.fpga().is_stm_mode() {
                                            ImGuiRenderer::stm_idx(imgui.system_time(), cpu)
                                        } else {
                                            0
                                        };
                                        let drives = cpu.fpga().intensities_and_phases(idx);
                                        let m = if self.settings.mod_enable {
                                            let mod_idx =
                                                ImGuiRenderer::mod_idx(imgui.system_time(), cpu);
                                            cpu.fpga().modulation_at(mod_idx)
                                        } else {
                                            0xFF
                                        };
                                        sources
                                            .drives_mut()
                                            .skip(body_pointer[cpu.idx()])
                                            .take(cpu.num_transducers())
                                            .enumerate()
                                            .for_each(|(i, d)| {
                                                d.amp = (PI
                                                    * FPGAEmulator::to_pulse_width(drives[i].0, m)
                                                        as f32
                                                    / 512.0)
                                                    .sin();
                                                d.phase = 2. * PI * (drives[i].1 as f32) / 256.0;
                                                d.set_wave_number(40e3, self.settings.sound_speed);
                                            });
                                    });
                                }

                                field_compute_pipeline.update(
                                    &render,
                                    &sources,
                                    &self.settings,
                                    &update_flag,
                                )?;
                                slice_viewer.update(&render, &self.settings, &update_flag)?;
                                trans_viewer.update(&sources, &update_flag)?;
                                let command_buffer = builder.build()?;

                                let field_image = slice_viewer.field_image_view();

                                if update_flag.contains(UpdateFlag::SAVE_IMAGE) {
                                    let image_buffer_content = field_image.read()?;
                                    let img_x = (self.settings.slice_width
                                        / self.settings.slice_pixel_size)
                                        as u32;
                                    let img_y = (self.settings.slice_height
                                        / self.settings.slice_pixel_size)
                                        as u32;
                                    let mut img_buf = image::ImageBuffer::new(img_x, img_y);
                                    img_buf
                                        .enumerate_pixels_mut()
                                        .zip(image_buffer_content.iter())
                                        .for_each(|((_, _, pixel), [r, g, b, a])| {
                                            let r = (r * 255.0) as u8;
                                            let g = (g * 255.0) as u8;
                                            let b = (b * 255.0) as u8;
                                            let a = (a * 255.0) as u8;
                                            *pixel = image::Rgba([r, g, b, a]);
                                        });
                                    image::imageops::flip_vertical(&img_buf)
                                        .save(&self.settings.image_save_path)?;
                                }

                                let config = Config {
                                    source_num: sources.len() as _,
                                    color_scale: self.settings.slice_color_scale,
                                    width: (self.settings.slice_width
                                        / self.settings.slice_pixel_size)
                                        as _,
                                    height: (self.settings.slice_height
                                        / self.settings.slice_pixel_size)
                                        as _,
                                    pixel_size: self.settings.slice_pixel_size as _,
                                    scale: MILLIMETER,
                                    model: slice_model.into(),
                                    ..Default::default()
                                };
                                let after_compute = field_compute_pipeline
                                    .compute(&render, config, field_image, &self.settings)?
                                    .join(before_pipeline_future);
                                let future =
                                    after_compute.then_execute(render.queue(), command_buffer)?;

                                future.boxed()
                            } else {
                                builder.end_render_pass(Default::default())?;
                                imgui.waiting(&render, &mut builder)?;

                                let command_buffer = builder.build()?;

                                let future = before_pipeline_future
                                    .then_execute(render.queue(), command_buffer)?;

                                future.boxed()
                            }
                        };

                        render.finish_frame(after_future);
                    }
                    event => {
                        imgui.handle_event(render.window(), &event);
                    }
                }

                if server_th_ref.is_finished() || !is_running {
                    *control_flow = ControlFlow::Exit;
                }

                Ok(())
            };
            if let Err(e) = run_loop(event, control_flow) {
                tracing::error!("{}", e);
                *control_flow = ControlFlow::Exit;
            }
        });

        let _ = shutdown.send(());
        if let Err(e) = server_th.join().unwrap() {
            match e.source() {
                Some(e) => tracing::error!("Server error: {}", e),
                None => tracing::error!("Server error: {}", e),
            }
        }

        Ok(res)
    }
}
