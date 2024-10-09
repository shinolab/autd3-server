use bytemuck::{NoUninit, Pod, Zeroable};
use std::{borrow::Cow, mem};
use wgpu::{util::DeviceExt, ComputePass, RenderPass};

use crate::{
    common::transform::{to_gl_pos, to_gl_rot},
    context::Context,
    state::State,
    surface::SurfaceWrapper,
    Matrix4, Vector2, Vector3, Vector4,
};

use super::DepthTexture;

const TEXTURE_DIMS: (u32, u32) = (1024, 1024);
const WORKGROUP_SIZE: (u32, u32) = (8, 8);
const COLOR_MAP_TEXTURE_SIZE: u32 = 256;

#[derive(NoUninit, Clone, Copy)]
#[repr(C)]
struct Config {
    sound_speed: f32,
    num_trans: u32,
    max_pressure: f32,
    _pad: u32,
}

pub struct SliceRenderer {
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    proj_view_buf: wgpu::Buffer,
    model_buf: wgpu::Buffer,
    slice_size_buf: wgpu::Buffer,
    trans_pos_buf: Option<wgpu::Buffer>,
    trans_state_buf: Option<wgpu::Buffer>,
    config_buf: Option<wgpu::Buffer>,
    texture_view: wgpu::TextureView,
    color_map_texture: wgpu::Texture,
    index_count: usize,
    bind_group: Option<wgpu::BindGroup>,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    _pos: [f32; 4],
    _tex_coord: [f32; 2],
}

fn vertex(pos: [f32; 3], tc: [i8; 2]) -> Vertex {
    Vertex {
        _pos: [pos[0], pos[1], pos[2], 1.0],
        _tex_coord: [tc[0] as f32, tc[1] as f32],
    }
}

fn create_vertices() -> (Vec<Vertex>, Vec<u16>) {
    let vertex_data = [
        vertex([-0.5, -0.5, 0.], [0, 0]),
        vertex([0.5, -0.5, 0.], [1, 0]),
        vertex([0.5, 0.5, 0.], [1, 1]),
        vertex([-0.5, 0.5, 0.], [0, 1]),
    ];

    let index_data: &[u16] = &[0, 2, 1, 0, 3, 2];

    (vertex_data.to_vec(), index_data.to_vec())
}

impl SliceRenderer {
    pub fn new(surface: &SurfaceWrapper, context: &Context) -> Self {
        let vertex_size = mem::size_of::<Vertex>();
        let (vertex_data, index_data) = create_vertices();

        let vertex_buf = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Slice Vertex Buffer"),
                usage: wgpu::BufferUsages::VERTEX,
                contents: bytemuck::cast_slice(&vertex_data),
            });
        let index_buf = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Slice Index Buffer"),
                usage: wgpu::BufferUsages::INDEX,
                contents: bytemuck::cast_slice(&index_data),
            });

        let storage_texture = context.device().create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: TEXTURE_DIMS.0,
                height: TEXTURE_DIMS.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let storage_texture_view =
            storage_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let slice_size_buf = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Slice Size Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: size_of::<Vector2>() as _,
            mapped_at_creation: false,
        });

        let texture_extent = wgpu::Extent3d {
            width: COLOR_MAP_TEXTURE_SIZE,
            height: 1,
            depth_or_array_layers: 1,
        };
        let color_map_texture = context.device().create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D1,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let bind_group_layout =
            context
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(64),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(64),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::StorageTexture {
                                access: wgpu::StorageTextureAccess::ReadWrite,
                                format: wgpu::TextureFormat::Rgba8Unorm,
                                view_dimension: wgpu::TextureViewDimension::D2,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(8),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 5,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 6,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(16),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 7,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D1,
                                multisampled: false,
                            },
                            count: None,
                        },
                    ],
                });
        let pipeline_layout =
            context
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let proj_view_buf = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Slice Projection View Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: size_of::<Matrix4>() as wgpu::BufferAddress,
            mapped_at_creation: false,
        });
        let model_buf = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Slice Model Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: size_of::<Matrix4>() as wgpu::BufferAddress,
            mapped_at_creation: false,
        });

        let shader = context
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
            });

        let vertex_buffers = [wgpu::VertexBufferLayout {
            array_stride: vertex_size as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: size_of::<Vector4>() as _,
                    shader_location: 1,
                },
            ],
        }];

        let config = surface.config();
        let pipeline = context
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    compilation_options: Default::default(),
                    buffers: &vertex_buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.view_formats[0],
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DepthTexture::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let compute_pipeline_layout =
            context
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });
        let compute_pipeline =
            context
                .device()
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: None,
                    layout: Some(&compute_pipeline_layout),
                    module: &shader,
                    entry_point: "main",
                    compilation_options: Default::default(),
                    cache: None,
                });

        Self {
            vertex_buf,
            index_buf,
            index_count: index_data.len(),
            model_buf,
            proj_view_buf,
            slice_size_buf,
            texture_view: storage_texture_view,
            bind_group: None,
            bind_group_layout,
            pipeline,
            compute_pipeline,
            color_map_texture,
            trans_pos_buf: None,
            trans_state_buf: None,
            config_buf: None,
        }
    }

    pub fn init(&mut self, state: &State, context: &Context) {
        let n = state.transducers.positions.len();
        self.trans_pos_buf = Some(context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Transducer Position Buffer"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            size: (n * size_of::<Vector4>()) as _,
            mapped_at_creation: false,
        }));

        self.trans_state_buf = Some(context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Transducer State Buffer"),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            size: (n * size_of::<Vector4>()) as _,
            mapped_at_creation: false,
        }));

        self.config_buf = Some(context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Slice Config Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: size_of::<Config>() as _,
            mapped_at_creation: false,
        }));

        let color_map_texture_view = self
            .color_map_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.bind_group = Some(
            context
                .device()
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.proj_view_buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.model_buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&self.texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.slice_size_buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: self.trans_pos_buf.as_ref().unwrap().as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: self.trans_state_buf.as_ref().unwrap().as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: self.config_buf.as_ref().unwrap().as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: wgpu::BindingResource::TextureView(&color_map_texture_view),
                        },
                    ],
                    label: None,
                }),
        )
    }

    pub fn update_trans_pos(&mut self, state: &State, context: &Context) {
        let trans_pos = state.transducers.positions.to_vec();
        context.queue().write_buffer(
            self.trans_pos_buf.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&trans_pos),
        );
    }

    pub fn update_trans_state(&mut self, state: &State, context: &Context) {
        let trans_pos = state.transducers.states.to_vec();
        context.queue().write_buffer(
            self.trans_state_buf.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&trans_pos),
        );
    }

    pub fn update_config(&mut self, state: &State, context: &Context) {
        let config = Config {
            sound_speed: state.sound_speed,
            num_trans: state.transducers.positions.len() as u32,
            max_pressure: state.slice.pressure_max,
            _pad: 0,
        };
        context.queue().write_buffer(
            self.config_buf.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&[config]),
        );
    }

    pub fn update_slice(&mut self, state: &State, context: &Context) {
        let model =
            Matrix4::from_rotation_translation(
                to_gl_rot(state.slice.rotation()),
                to_gl_pos(state.slice.pos),
            ) * Matrix4::from_scale(Vector3::new(state.slice.size.x, state.slice.size.y, 1.));
        context
            .queue()
            .write_buffer(&self.model_buf, 0, bytemuck::cast_slice(model.as_ref()));
        context.queue().write_buffer(
            &self.slice_size_buf,
            0,
            bytemuck::cast_slice(state.slice.size.as_ref()),
        );
    }

    pub fn update_color_map(&mut self, state: &State, context: &Context) {
        let iter = (0..COLOR_MAP_TEXTURE_SIZE).map(|x| x as f64 / COLOR_MAP_TEXTURE_SIZE as f64);
        let texels = state
            .slice
            .color_map
            .color_map(iter)
            .into_iter()
            .flat_map(|color| {
                [
                    (color.r * 255.) as u8,
                    (color.g * 255.) as u8,
                    (color.b * 255.) as u8,
                    255,
                ]
            })
            .collect::<Vec<_>>();
        context.queue().write_texture(
            self.color_map_texture.as_image_copy(),
            bytemuck::cast_slice(&texels),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: COLOR_MAP_TEXTURE_SIZE,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn update_camera(&mut self, proj_view: Matrix4, context: &Context) {
        context.queue().write_buffer(
            &self.proj_view_buf,
            0,
            bytemuck::cast_slice(proj_view.as_ref()),
        );
    }

    pub fn resize(&mut self, proj_view: Matrix4, context: &Context) {
        self.update_camera(proj_view, context);
    }

    pub fn compute(&mut self, pass: &mut ComputePass) {
        pass.set_bind_group(0, self.bind_group.as_ref().unwrap(), &[]);
        pass.set_pipeline(&self.compute_pipeline);
        pass.dispatch_workgroups(
            (TEXTURE_DIMS.0 - 1) / WORKGROUP_SIZE.0 + 1,
            (TEXTURE_DIMS.1 - 1) / WORKGROUP_SIZE.1 + 1,
            1,
        );
    }

    pub fn render(&mut self, pass: &mut RenderPass) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, self.bind_group.as_ref().unwrap(), &[]);
        pass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
        pass.draw_indexed(0..self.index_count as u32, 0, 0..1);
    }
}
