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
    renderer::Renderer,
    sound_sources::{Drive, SoundSources},
    update_flag::UpdateFlag,
    view::*,
    viewer_settings::ViewerSettings,
    Quaternion, Vector3,
};
use autd3_driver::{
    defined::{T4010A1_AMPLITUDE, ULTRASOUND_PERIOD_COUNT},
    firmware::cpu::TxDatagram,
};
use autd3_firmware_emulator::CPUEmulator;
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
    rx_buf: Arc<RwLock<Vec<autd3_driver::firmware::cpu::RxMessage>>>,
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
            data: rx.iter().flat_map(|c| [c.data(), c.ack()]).collect(),
        }))
    }

    async fn close(&self, _: Request<CloseRequest>) -> Result<Response<CloseResponse>, Status> {
        if self.sender.send(Signal::Close).is_err() {
            return Err(Status::unavailable("Simulator is closed"));
        }
        Ok(Response::new(CloseResponse { success: true }))
    }
}

#[derive(Default)]
pub struct Simulator {
    window_width: Option<u32>,
    window_height: Option<u32>,
    vsync: Option<bool>,
    port: Option<u16>,
    lighweight_port: Option<u16>,
    gpu_idx: Option<i32>,
    settings: ViewerSettings,
    resource_path: PathBuf,
    config_path: Option<PathBuf>,
    lightweight: bool,
}

impl Simulator {
    pub fn new<P: AsRef<Path>>(resource_path: P) -> Self {
        Self {
            window_width: None,
            window_height: None,
            vsync: None,
            port: None,
            lighweight_port: None,
            gpu_idx: None,
            settings: ViewerSettings::default(),
            resource_path: resource_path.as_ref().to_owned(),
            config_path: None,
            lightweight: false,
        }
    }

    pub const fn with_window_size(mut self, width: u32, height: u32) -> Self {
        self.window_width = Some(width);
        self.window_height = Some(height);
        self
    }

    pub const fn with_vsync(mut self, vsync: bool) -> Self {
        self.vsync = Some(vsync);
        self
    }

    pub const fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub const fn with_lightweight_port(mut self, port: u16) -> Self {
        self.lighweight_port = Some(port);
        self
    }

    pub const fn with_gpu_idx(mut self, gpu_idx: i32) -> Self {
        self.gpu_idx = Some(gpu_idx);
        self
    }

    pub fn with_settings(mut self, settings: ViewerSettings) -> Self {
        self.settings = settings;
        self
    }

    pub fn with_config_path<S: AsRef<OsStr> + Sized>(mut self, config_path: S) -> Self {
        self.config_path = Some(Path::new(&config_path).to_owned());
        self
    }

    pub const fn get_settings(&self) -> &ViewerSettings {
        &self.settings
    }

    pub const fn enable_lightweight(mut self) -> Self {
        self.lightweight = true;
        self
    }

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
                Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
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
                    })
            }
        });

        let lightweight_server = if self.lightweight {
            let (tx_shutdown_lightweigh, rx_shutdown_lightweight) = oneshot::channel::<()>();
            let lighweight_port = self
                .lighweight_port
                .unwrap_or(self.settings.lighweight_port);
            Some((
                std::thread::spawn({
                    move || {
                        use autd3_protobuf::{lightweight::LightweightServer, *};
                        Builder::new_multi_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(async {
                                let server = LightweightServer::new(move || {
                                    autd3_link_simulator::Simulator::builder(
                                        format!("127.0.0.1:{}", port).parse().unwrap(),
                                    )
                                });
                                Server::builder()
                                    .add_service(ecat_light_server::EcatLightServer::new(server))
                                    .serve_with_shutdown(
                                        format!("0.0.0.0:{}", lighweight_port)
                                            .to_socket_addrs()
                                            .unwrap()
                                            .next()
                                            .unwrap(),
                                        rx_shutdown_lightweight.map(drop),
                                    )
                                    .await
                            })
                    }
                }),
                tx_shutdown_lightweigh,
            ))
        } else {
            None
        };

        self.run_simulator(server_th, rx_buf, rx, tx_shutdown, lightweight_server)
    }

    #[allow(clippy::type_complexity)]
    fn run_simulator(
        &mut self,
        server_th: std::thread::JoinHandle<Result<(), tonic::transport::Error>>,
        rx_buf: Arc<RwLock<Vec<autd3_driver::firmware::cpu::RxMessage>>>,
        receiver: Receiver<Signal>,
        shutdown: tokio::sync::oneshot::Sender<()>,
        lightweight_server: Option<(
            std::thread::JoinHandle<Result<(), tonic::transport::Error>>,
            tokio::sync::oneshot::Sender<()>,
        )>,
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
        let mut device_viewer = DeviceViewer::new(&render, &self.resource_path)?;
        let mut imgui = ImGuiViewer::new(self.settings.clone(), &self.config_path, &render)?;
        let mut trans_viewer = TransViewer::new(&render)?;

        let mut is_initialized = false;
        let mut is_source_update = false;
        let mut is_running = true;

        let server_th_ref = &server_th;

        let res = event_loop.run_return(move |event, _, control_flow| {
            let mut run_loop = |event, control_flow: &mut ControlFlow| -> anyhow::Result<()> {
                cpus.iter_mut().try_for_each(|cpu| -> anyhow::Result<()> {
                    cpu.update_with_sys_time(imgui.system_time()?);
                    Ok(())
                })?;

                if cpus.iter().any(CPUEmulator::should_update) {
                    rx_buf
                        .write()
                        .unwrap()
                        .iter_mut()
                        .zip(cpus.iter())
                        .for_each(|(d, s)| {
                            *d = s.rx();
                        });
                }

                match receiver.try_recv() {
                    Ok(Signal::ConfigGeometry(geometry)) => {
                        sources.clear();
                        cpus.clear();

                        let geometry = autd3_driver::geometry::Geometry::from_msg(&geometry)?;
                        geometry.iter().for_each(|dev| {
                            let r = dev.rotation();
                            dev.iter().for_each(|tr| {
                                let p = tr.position();
                                sources.add(
                                    to_gl_pos(Vector3::new(p.x as _, p.y as _, p.z as _)),
                                    to_gl_rot(Quaternion::new(
                                        r.w as _, r.i as _, r.j as _, r.k as _,
                                    )),
                                    Drive::new(1.0, 0.0, 1.0, self.settings.sound_speed),
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
                                autd3_driver::firmware::cpu::RxMessage::new(0, 0);
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
                        let geometry = autd3_driver::geometry::Geometry::from_msg(&geometry)?;
                        geometry
                            .iter()
                            .flat_map(|dev| {
                                let r = dev.rotation();
                                dev.iter().map(|tr| {
                                    let p = tr.position();
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
                        let tx = TxDatagram::from_msg(&raw)?;
                        cpus.iter_mut().for_each(|cpu| {
                            cpu.send(&tx);
                        });
                        rx_buf
                            .write()
                            .unwrap()
                            .iter_mut()
                            .zip(cpus.iter())
                            .for_each(|(d, s)| {
                                *d = s.rx();
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
                        let event_imgui: Event<()> = Event::WindowEvent {
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
                                if self.settings.slice_show {
                                    slice_viewer.render(
                                        &render,
                                        view,
                                        proj,
                                        &self.settings,
                                        &mut builder,
                                    )?;
                                }
                                if self.settings.view_device {
                                    device_viewer.render(
                                        (view, proj),
                                        imgui.visible(),
                                        &mut builder,
                                    )?;
                                } else {
                                    trans_viewer.render(view, proj, &mut builder)?;
                                }
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
                                    cpus.iter().try_for_each(|cpu| -> anyhow::Result<()> {
                                        let stm_segment = cpu.fpga().current_stm_segment();
                                        let idx = if cpu.fpga().stm_cycle(stm_segment) == 1 {
                                            0
                                        } else {
                                            cpu.fpga().current_stm_idx()
                                        };
                                        let drives = cpu.fpga().drives(stm_segment, idx);
                                        let mod_segment = cpu.fpga().current_mod_segment();
                                        let m = if self.settings.mod_enable {
                                            let mod_idx = cpu.fpga().current_mod_idx();
                                            cpu.fpga().modulation_at(mod_segment, mod_idx)
                                        } else {
                                            u8::MAX
                                        };
                                        sources
                                            .drives_mut()
                                            .skip(body_pointer[cpu.idx()])
                                            .take(cpu.num_transducers())
                                            .enumerate()
                                            .for_each(|(i, d)| {
                                                d.amp = (PI
                                                    * cpu
                                                        .fpga()
                                                        .to_pulse_width(drives[i].intensity(), m)
                                                        as f32
                                                    / ULTRASOUND_PERIOD_COUNT as f32)
                                                    .sin();
                                                d.phase = drives[i].phase().radian();
                                                d.set_wave_number(self.settings.sound_speed);
                                            });

                                        Ok(())
                                    })?;
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
                                    color_scale: T4010A1_AMPLITUDE / self.settings.pressure_max,
                                    width: (self.settings.slice_width
                                        / self.settings.slice_pixel_size)
                                        as _,
                                    height: (self.settings.slice_height
                                        / self.settings.slice_pixel_size)
                                        as _,
                                    pixel_size: self.settings.slice_pixel_size as _,
                                    model: slice_model.into(),
                                    ..Default::default()
                                };
                                let after_compute = field_compute_pipeline
                                    .compute(&render, config, field_image)?
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

        if let Some((server_th, tx_shutdown_lightweigh)) = lightweight_server {
            let _ = tx_shutdown_lightweigh.send(());
            if let Err(e) = server_th.join().unwrap() {
                match e.source() {
                    Some(e) => tracing::error!("Server error: {}", e),
                    None => tracing::error!("Server error: {}", e),
                }
            }
        }

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
