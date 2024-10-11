use autd3_driver::autd3_device::AUTD3;
use bytemuck::{Pod, Zeroable};
use image::{ImageBuffer, Rgba};
use std::{borrow::Cow, f32::consts::PI, mem};
use wgpu::{util::DeviceExt, RenderPass};

use crate::{
    common::color::{Color, Hsv},
    context::Context,
    state::State,
    surface::SurfaceWrapper,
    Matrix4, SimulatorError, Vector3, Vector4,
};

use super::DepthTexture;

pub struct TransducerRenderer {
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    model_instance_buf: Option<wgpu::Buffer>,
    color_instance_buf: Option<wgpu::Buffer>,
    proj_view_buf: wgpu::Buffer,
    index_count: usize,
    instance_count: u32,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    _pos: [f32; 4],
    _tex_coord: [f32; 2],
}

fn vertex(pos: [f32; 3], tc: [i16; 2]) -> Vertex {
    Vertex {
        _pos: [pos[0], pos[1], pos[2], 1.0],
        _tex_coord: [tc[0] as f32, tc[1] as f32],
    }
}

fn create_vertices() -> (Vec<Vertex>, Vec<u16>) {
    let vertex_data = [
        vertex([-0.5, -0.5, 0.], [0, 0]),
        vertex([0.5, -0.5, 0.], [128, 0]),
        vertex([0.5, 0.5, 0.], [128, 128]),
        vertex([-0.5, 0.5, 0.], [0, 128]),
    ];

    let index_data: &[u16] = &[0, 1, 2, 2, 3, 0];

    (vertex_data.to_vec(), index_data.to_vec())
}

#[allow(clippy::type_complexity)]
fn create_texels() -> Result<((u32, u32), ImageBuffer<Rgba<u8>, Vec<u8>>), SimulatorError> {
    let diffuse_bytes = include_bytes!("circle.png");
    let diffuse_image = image::load_from_memory(diffuse_bytes)?;
    let diffuse_rgba = diffuse_image.to_rgba8();

    use image::GenericImageView;
    let dimensions = diffuse_image.dimensions();

    Ok((dimensions, diffuse_rgba))
}

fn coloring_hsv(h: f32, v: f32, a: f32) -> [f32; 4] {
    let hsv = Hsv { h, s: 1., v, a };
    hsv.rgba()
}

impl TransducerRenderer {
    pub fn new(surface: &SurfaceWrapper, context: &Context) -> Result<Self, SimulatorError> {
        let vertex_size = mem::size_of::<Vertex>();
        let (vertex_data, index_data) = create_vertices();

        let vertex_buf = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buf = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&index_data),
                usage: wgpu::BufferUsages::INDEX,
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
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
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

        let ((width, height), texels) = create_texels()?;
        let texture_extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = context.device().create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        context.queue().write_texture(
            texture.as_image_copy(),
            &texels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_extent,
        );

        let proj_view_buf = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Projection View Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: size_of::<Matrix4>() as wgpu::BufferAddress,
            mapped_at_creation: false,
        });

        let bind_group = context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: proj_view_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                ],
                label: None,
            });

        let shader = context
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
            });

        let vertex_buffers = [
            wgpu::VertexBufferLayout {
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
            },
            wgpu::VertexBufferLayout {
                array_stride: size_of::<Matrix4>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 2,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                        shader_location: 3,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                        shader_location: 4,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexAttribute {
                        offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                        shader_location: 5,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                ],
            },
            wgpu::VertexBufferLayout {
                array_stride: size_of::<Vector4>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                }],
            },
        ];

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

        Ok(Self {
            vertex_buf,
            index_buf,
            index_count: index_data.len(),
            model_instance_buf: None,
            color_instance_buf: None,
            instance_count: 0,
            bind_group,
            proj_view_buf,
            pipeline,
        })
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

    pub fn render(&mut self, pass: &mut RenderPass) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
        pass.set_vertex_buffer(1, self.model_instance_buf.as_ref().unwrap().slice(..));
        pass.set_vertex_buffer(2, self.color_instance_buf.as_ref().unwrap().slice(..));
        pass.draw_indexed(0..self.index_count as u32, 0, 0..self.instance_count);
    }

    pub fn init(&mut self, state: &State, context: &Context) {
        let instance_count = state.transducers.positions.len();
        self.model_instance_buf = Some(context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Model Instance Buffer"),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            size: (size_of::<Matrix4>() * instance_count) as _,
            mapped_at_creation: false,
        }));
        self.color_instance_buf = Some(context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Color Instance Buffer"),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            size: (size_of::<Vector4>() * instance_count) as _,
            mapped_at_creation: false,
        }));
        self.instance_count = instance_count as _;
    }

    pub fn update_model(&mut self, state: &State, context: &Context) {
        let instance_data = state
            .transducers
            .positions
            .iter()
            .zip(state.transducers.rotations.iter())
            .map(|(p, r)| {
                Matrix4::from_rotation_translation(*r, p.truncate())
                    * Matrix4::from_scale(Vector3::new(
                        AUTD3::TRANS_SPACING,
                        AUTD3::TRANS_SPACING,
                        1.,
                    ))
            })
            .collect::<Vec<_>>();
        context.queue().write_buffer(
            self.model_instance_buf.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(instance_data.as_ref()),
        );
    }

    pub fn update_color(&mut self, state: &State, context: &Context) {
        let instance_data = state
            .transducers
            .states
            .iter()
            .map(|d| coloring_hsv(d.phase / (2.0 * PI), d.amp, d.alpha))
            .collect::<Vec<_>>();
        context.queue().write_buffer(
            self.color_instance_buf.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(instance_data.as_ref()),
        );
    }
}
