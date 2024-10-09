use std::sync::Arc;

use camera_controllers::{Camera, CameraPerspective, FirstPerson, FirstPersonSettings};
use winit::{event::Event, window::Window};

use crate::{
    common::camera_helper, context::Context, state::State, surface::SurfaceWrapper,
    update_flag::UpdateFlag, Matrix4, SimulatorError, Vector3,
};

mod imgui;
mod slice;
mod transducer;

struct DepthTexture {
    view: wgpu::TextureView,
}

impl DepthTexture {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    fn new(context: &Context, surface: &SurfaceWrapper) -> Self {
        let config = surface.config();
        let size = wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = context.device().create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { view }
    }
}

pub struct Renderer {
    imgui: imgui::ImGuiRenderer,
    transducer: transducer::TransducerRenderer,
    slice: slice::SliceRenderer,
    depth_texture: DepthTexture,
    camera: Camera<f32>,
}

impl Renderer {
    pub fn new(
        state: &State,
        context: &Context,
        surface: &SurfaceWrapper,
        window: Arc<Window>,
    ) -> Result<Self, SimulatorError> {
        let mut camera =
            FirstPerson::new([0., -500.0, 120.0], FirstPersonSettings::keyboard_wasd()).camera(0.);
        camera.set_yaw_pitch(0., -std::f32::consts::PI / 2.0);
        Ok(Self {
            imgui: imgui::ImGuiRenderer::new(state, context, window.clone())?,
            transducer: transducer::TransducerRenderer::new(surface, context)?,
            slice: slice::SliceRenderer::new(surface, context),
            depth_texture: DepthTexture::new(context, surface),
            camera,
        })
    }

    pub fn render(
        &mut self,
        state: &mut State,
        context: &Context,
        window: Arc<Window>,
        view: &wgpu::TextureView,
        update_flag: &mut UpdateFlag,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), SimulatorError> {
        self.imgui.update_delta_time();

        let load = if state.initialized() {
            self.imgui.update(state, context, &window, update_flag)?;

            if update_flag.contains(UpdateFlag::UPDATE_CAMERA) {
                self.move_camera(state);
                let view_proj = self.proj_view(state, window);
                self.transducer.update_camera(view_proj, context);
                self.slice.update_camera(view_proj, context);

                update_flag.remove(UpdateFlag::UPDATE_CAMERA);
            }

            if update_flag.contains(UpdateFlag::UPDATE_TRANS_POS) {
                self.transducer.update_model(state, context);
                self.slice.update_trans_pos(state, context);

                update_flag.remove(UpdateFlag::UPDATE_TRANS_POS);
            }

            if update_flag.contains(UpdateFlag::UPDATE_TRANS_ALPHA)
                | update_flag.contains(UpdateFlag::UPDATE_TRANS_STATE)
            {
                if update_flag.contains(UpdateFlag::UPDATE_TRANS_STATE) {
                    state.update_trans();
                    self.slice.update_trans_state(state, context);
                }
                self.transducer.update_color(state, context);

                update_flag.remove(UpdateFlag::UPDATE_TRANS_ALPHA);
                update_flag.remove(UpdateFlag::UPDATE_TRANS_STATE);
            }

            if update_flag.contains(UpdateFlag::UPDATE_SLICE_POS)
                | update_flag.contains(UpdateFlag::UPDATE_SLICE_SIZE)
            {
                self.slice.update_slice(state, context);

                update_flag.remove(UpdateFlag::UPDATE_SLICE_SIZE);
                update_flag.remove(UpdateFlag::UPDATE_SLICE_POS);
            }

            if update_flag.contains(UpdateFlag::UPDATE_CONFIG) {
                self.slice.update_config(state, context);

                update_flag.remove(UpdateFlag::UPDATE_CONFIG);
            }

            if update_flag.contains(UpdateFlag::UPDATE_SLICE_COLOR_MAP) {
                self.slice.update_color_map(state, context);

                update_flag.remove(UpdateFlag::UPDATE_SLICE_COLOR_MAP);
            }

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                self.slice.compute(&mut compute_pass);
            }

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(state.background()),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_texture.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),

                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                self.transducer.render(&mut rpass);
                self.slice.render(&mut rpass);
            }

            wgpu::LoadOp::Load
        } else {
            self.imgui.waiting(context, &window)?;
            wgpu::LoadOp::Clear(state.background())
        };

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        self.imgui.render(context, &mut rpass)?;

        Ok(())
    }

    pub fn init(&mut self, context: &Context, state: &State) -> Result<(), SimulatorError> {
        self.imgui.init(state.num_devices());
        self.transducer.init(state, context);
        self.slice.init(state, context);

        Ok(())
    }

    pub fn resize<T>(
        &mut self,
        context: &Context,
        state: &State,
        surface: &SurfaceWrapper,
        window: Arc<Window>,
        event: &Event<T>,
    ) {
        self.imgui.handle_event(&window, event);
        let view_proj = self.proj_view(state, window);
        self.transducer.resize(view_proj, context);
        self.slice.resize(view_proj, context);
        self.depth_texture = DepthTexture::new(context, surface);
    }

    pub fn handle_event<T>(&mut self, window: Arc<Window>, event: &Event<T>) {
        self.imgui.handle_event(&window, event);
    }

    fn proj_view(&self, state: &State, window: Arc<Window>) -> Matrix4 {
        Self::projection(state, window) * Self::view(&self.camera)
    }

    fn projection(state: &State, window: Arc<Window>) -> Matrix4 {
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

    fn move_camera(&mut self, state: &State) {
        camera_helper::set_camera(
            &mut self.camera,
            Vector3::new(state.camera.pos.x, state.camera.pos.y, state.camera.pos.z),
            Vector3::new(state.camera.rot.x, state.camera.rot.y, state.camera.rot.z),
        );
    }
}
