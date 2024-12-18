mod depth_texture;
mod egui_renderer;
mod slice_renderer;
mod transducer_renderer;

use std::{num::NonZeroU32, sync::Arc};

use crate::{
    common::camera::{create_camera, Camera, CameraPerspective},
    emulator::EmulatorWrapper,
    error::{Result, SimulatorError},
    event::{EventResult, UserEvent},
    update_flag::UpdateFlag,
    Matrix4, State, Vector3,
};

use depth_texture::DepthTexture;
use egui::ViewportId;
use egui_renderer::EguiRenderer;
use egui_wgpu::ScreenDescriptor;
use winit::{event::DeviceEvent, event_loop::EventLoopProxy, window::Window};

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    egui_renderer: egui_renderer::EguiRenderer,
    transducer_renderer: transducer_renderer::TransducerRenderer,
    slice_renderer: slice_renderer::SliceRenderer,
    depth_texture: DepthTexture,
    camera: Camera<f32>,
}

impl Renderer {
    pub async fn new(
        instance: &wgpu::Instance,
        event_loop_proxy: EventLoopProxy<UserEvent>,
        egui_ctx: egui::Context,
        window: Arc<Window>,
        width: u32,
        height: u32,
        state: &State,
    ) -> Result<Self> {
        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or(SimulatorError::NoSuitableAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    required_limits: Default::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await?;

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities
            .formats
            .iter()
            .find(|d| **d == wgpu::TextureFormat::Bgra8UnormSrgb)
            .ok_or(SimulatorError::NoSuitableFormat)?;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *swapchain_format,
            width,
            height,
            present_mode: if state.vsync {
                wgpu::PresentMode::Fifo
            } else {
                wgpu::PresentMode::Immediate
            },
            desired_maximum_frame_latency: 2,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        };

        surface.configure(&device, &surface_config);

        Ok(Self {
            egui_renderer: EguiRenderer::new(
                state,
                &device,
                event_loop_proxy,
                egui_ctx,
                window,
                &surface_config,
            ),
            transducer_renderer: transducer_renderer::TransducerRenderer::new(
                &device,
                &queue,
                &surface_config,
            )?,
            slice_renderer: slice_renderer::SliceRenderer::new(&device, &surface_config),
            depth_texture: DepthTexture::new(&device, &surface_config),
            camera: create_camera(),
            surface,
            surface_config,
            device,
            queue,
        })
    }

    pub fn create_egui_context() -> egui::Context {
        EguiRenderer::create_egui_context()
    }

    pub fn initialize(&mut self, emulator: &EmulatorWrapper) {
        self.transducer_renderer.initialize(&self.device, emulator);
        self.slice_renderer.initialize(&self.device, emulator);
    }

    pub fn run_ui_and_paint(
        &mut self,
        state: &mut State,
        emulator: &mut EmulatorWrapper,
        window: &Window,
        update_flag: &mut UpdateFlag,
    ) -> Result<EventResult> {
        let Self {
            surface,
            surface_config,
            device,
            queue,
            egui_renderer,
            transducer_renderer,
            slice_renderer,
            ..
        } = self;

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point: window.scale_factor() as f32 * state.ui_scale,
        };

        let surface_texture = surface.get_current_texture()?;

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let load = if emulator.initialized() {
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                slice_renderer.compute(&mut compute_pass);
            }

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("main render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(state.background()),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: self.depth_texture.view(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                transducer_renderer.render(&mut rpass);
                slice_renderer.render(&mut rpass);
            }
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(state.background())
        };

        let result = egui_renderer.run_ui_and_paint(
            device,
            queue,
            load,
            &mut encoder,
            &surface_view,
            screen_descriptor,
            window,
            state,
            emulator,
            update_flag,
        )?;

        queue.submit(Some(encoder.finish()));
        surface_texture.present();

        Ok(result)
    }

    pub fn update_camera(&mut self, state: &State, window: &Window) {
        crate::common::camera::set_camera(
            &mut self.camera,
            Vector3::new(state.camera.pos.x, state.camera.pos.y, state.camera.pos.z),
            Vector3::new(state.camera.rot.x, state.camera.rot.y, state.camera.rot.z),
        );
        let view_proj = Self::proj_view(&self.camera, state, window);
        self.transducer_renderer
            .update_camera(view_proj, &self.queue);
        self.slice_renderer.update_camera(view_proj, &self.queue);
    }

    fn proj_view(camera: &Camera<f32>, state: &State, window: &Window) -> Matrix4 {
        fn projection(state: &State, window: &Window) -> Matrix4 {
            let draw_size = window.inner_size();
            Matrix4::from_cols_array_2d(
                &CameraPerspective {
                    fov: state.camera.fov,
                    near_clip: state.camera.near_clip,
                    far_clip: state.camera.far_clip,
                    aspect_ratio: (draw_size.width as f32) / (draw_size.height as f32),
                }
                .projection(),
            )
        }

        fn view(camera: &Camera<f32>) -> Matrix4 {
            Matrix4::from_cols_array_2d(&camera.orthogonal())
        }

        projection(state, window) * view(camera)
    }

    pub fn update_trans_pos(&mut self, emulator: &EmulatorWrapper) {
        self.transducer_renderer.update_model(emulator, &self.queue);
        self.slice_renderer.update_trans_pos(emulator, &self.queue);
    }

    pub fn update_trans_state(&mut self, emulator: &EmulatorWrapper) {
        self.slice_renderer
            .update_trans_state(emulator, &self.queue);
    }

    pub fn update_color(&mut self, emulator: &EmulatorWrapper) {
        self.transducer_renderer.update_color(emulator, &self.queue);
    }

    pub fn update_slice(&mut self, state: &State) {
        self.slice_renderer.update_slice(state, &self.queue);
    }

    pub fn update_config(&mut self, state: &State, emulator: &EmulatorWrapper) {
        self.slice_renderer
            .update_config(state, emulator, &self.queue);
    }

    pub fn update_color_map(&mut self, state: &State) {
        self.slice_renderer.update_color_map(state, &self.queue);
    }

    pub(crate) fn on_window_event(
        &mut self,
        event: &winit::event::WindowEvent,
        window: &Window,
        state: &State,
    ) -> EventResult {
        let Self {
            surface,
            surface_config,
            device,
            queue,
            egui_renderer,
            camera,
            ..
        } = self;
        let mut repaint_asap = false;

        match event {
            winit::event::WindowEvent::Resized(physical_size) => {
                if let (Some(width), Some(height)) = (
                    NonZeroU32::new(physical_size.width),
                    NonZeroU32::new(physical_size.height),
                ) {
                    repaint_asap = true;
                    surface_config.width = width.get();
                    surface_config.height = height.get();
                    surface.configure(device, surface_config);

                    let view_proj = Self::proj_view(camera, state, window);
                    self.transducer_renderer.resize(view_proj, queue);
                    self.slice_renderer.resize(view_proj, queue);
                    self.depth_texture = DepthTexture::new(device, surface_config);
                }
            }

            winit::event::WindowEvent::CloseRequested => {
                if egui_renderer.close() {
                    return EventResult::Exit;
                }

                egui_renderer.info().events.push(egui::ViewportEvent::Close);

                egui_renderer.context().request_repaint_of(ViewportId::ROOT);
            }
            _ => {}
        };

        let event_response = egui_renderer.on_window_event(window, event);

        if egui_renderer.close() {
            EventResult::Exit
        } else if event_response.repaint {
            if repaint_asap {
                EventResult::RepaintNow
            } else {
                EventResult::RepaintNext
            }
        } else {
            EventResult::Wait
        }
    }

    pub fn on_user_event(&self, event: &UserEvent) -> EventResult {
        match event {
            UserEvent::RequestRepaint {
                when,
                cumulative_pass_nr,
            } => {
                let current_pass_nr = self
                    .egui_renderer
                    .context()
                    .cumulative_pass_nr_for(ViewportId::ROOT);
                if current_pass_nr == *cumulative_pass_nr
                    || current_pass_nr == *cumulative_pass_nr + 1
                {
                    EventResult::RepaintAt(*when)
                } else {
                    EventResult::Wait
                }
            }
            _ => EventResult::RepaintNow,
        }
    }

    pub(crate) fn on_device_event(&mut self, event: DeviceEvent) -> EventResult {
        self.egui_renderer.on_device_event(event)
    }
}
