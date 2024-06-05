use std::path::Path;

use super::model::{Model, ModelVertex};

use crate::prelude::*;

#[allow(clippy::needless_question_mark)]
mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "./assets/shaders/base.vert"
    }
}

#[allow(clippy::needless_question_mark)]
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "./assets/shaders/base.frag"
    }
}

pub struct DeviceViewer {
    model: Model,
    vertices: Subbuffer<[ModelVertex]>,
    indices: Subbuffer<[u32]>,
    texture_desc_set: Arc<PersistentDescriptorSet>,
    pipeline: Arc<GraphicsPipeline>,
    pos_rot: Vec<(Vector3, Quaternion)>,
}

impl DeviceViewer {
    pub fn new<P: AsRef<Path>>(renderer: &Renderer, resource_path: P) -> anyhow::Result<Self> {
        let model = Model::new(resource_path)?;

        let vertices = Self::create_vertices(renderer, &model.vertices)?;
        let indices = Self::create_indices(renderer, &model.indices)?;

        let vs = vs::load(renderer.device())?.entry_point("main").unwrap();
        let fs = fs::load(renderer.device())?.entry_point("main").unwrap();

        let interface = vs.info().input_interface.clone();
        let stages = [
            PipelineShaderStageCreateInfo::new(vs),
            PipelineShaderStageCreateInfo::new(fs),
        ];
        let subpass = Subpass::from(renderer.render_pass(), 0).unwrap();
        let pipeline = GraphicsPipeline::new(
            renderer.device(),
            None,
            GraphicsPipelineCreateInfo {
                stages: stages.iter().cloned().collect(),
                vertex_input_state: Some(ModelVertex::per_vertex().definition(&interface)?),
                input_assembly_state: Some(InputAssemblyState {
                    topology: PrimitiveTopology::TriangleList,
                    ..Default::default()
                }),
                viewport_state: Some(ViewportState::default()),
                rasterization_state: Some(RasterizationState::default()),
                multisample_state: Some(MultisampleState {
                    rasterization_samples: renderer.sample_count(),
                    ..MultisampleState::default()
                }),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    subpass.num_color_attachments(),
                    ColorBlendAttachmentState {
                        blend: Some(AttachmentBlend::alpha()),
                        ..Default::default()
                    },
                )),
                depth_stencil_state: Some(DepthStencilState {
                    depth: Some(DepthState::simple()),
                    ..Default::default()
                }),
                dynamic_state: [DynamicState::Viewport].into_iter().collect(),
                subpass: Some(subpass.into()),
                ..GraphicsPipelineCreateInfo::layout(PipelineLayout::new(
                    renderer.device(),
                    PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                        .into_pipeline_layout_create_info(renderer.device())?,
                )?)
            },
        )?;

        Ok(Self {
            texture_desc_set: Self::create_texture_desc_set(
                pipeline.clone(),
                renderer,
                &model.image,
            )?,
            model,
            vertices,
            indices,
            pipeline,
            pos_rot: Vec::new(),
        })
    }

    pub fn init(&mut self, geometry: &Geometry) {
        self.pos_rot = geometry
            .iter()
            .map(|dev| {
                let p = dev[0].position();
                let r = dev.rotation();
                (
                    to_gl_pos(Vector3::new(p.x as _, p.y as _, p.z as _)),
                    to_gl_rot(Quaternion::new(r.w as _, r.i as _, r.j as _, r.k as _)),
                )
            })
            .collect();
    }

    pub fn render(
        &mut self,
        view_proj: (Matrix4, Matrix4),
        setting: &ViewerSettings,
        visible: &[bool],
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> anyhow::Result<()> {
        builder
            .bind_vertex_buffers(0, self.vertices.clone())?
            .bind_index_buffer(self.indices.clone())?
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                self.texture_desc_set.clone(),
            )?;

        let (mut view, proj) = view_proj;
        view[3][0] /= crate::METER;
        view[3][1] /= crate::METER;
        view[3][2] /= crate::METER;

        self.pos_rot
            .iter()
            .zip(visible.iter())
            .filter(|&(_, &s)| s)
            .try_for_each(|(&(pos, rot), _)| -> anyhow::Result<()> {
                self.model
                    .primitives
                    .iter()
                    .try_for_each(|primitive| -> anyhow::Result<()> {
                        let material = &self.model.materials[primitive.material_index];
                        builder
                            .bind_pipeline_graphics(self.pipeline.clone())?
                            .push_constants(
                                self.pipeline.layout().clone(),
                                0,
                                fs::PushConsts {
                                    proj_view: (proj * view).into(),
                                    model: (Matrix4::from_translation(pos / crate::METER)
                                        * Matrix4::from(rot))
                                    .into(),
                                    lightPos: [
                                        setting.light_pos_x / crate::METER,
                                        setting.light_pos_y / crate::METER,
                                        setting.light_pos_z / crate::METER,
                                        1.,
                                    ],
                                    viewPos: [
                                        setting.camera_pos_x / crate::METER,
                                        setting.camera_pos_y / crate::METER,
                                        setting.camera_pos_z / crate::METER,
                                        1.,
                                    ],
                                    ambient: setting.ambient,
                                    specular: setting.specular,
                                    lightPower: setting.light_power,
                                    metallic: material.metallic_factor,
                                    roughness: material.roughness_factor,
                                    baseColorR: material.base_color_factor[0],
                                    baseColorG: material.base_color_factor[1],
                                    baseColorB: material.base_color_factor[2],
                                    hasTexture: if material.base_color_texture_idx.is_some() {
                                        1
                                    } else {
                                        0
                                    },
                                },
                            )?
                            .draw_indexed(primitive.index_count, 1, primitive.first_index, 0, 0)?;

                        Ok(())
                    })
            })?;

        Ok(())
    }

    fn create_vertices(
        renderer: &Renderer,
        vertices: &[ModelVertex],
    ) -> anyhow::Result<Subbuffer<[ModelVertex]>> {
        Ok(Buffer::from_iter(
            renderer.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            vertices.iter().cloned(),
        )?)
    }

    fn create_indices(renderer: &Renderer, indices: &[u32]) -> anyhow::Result<Subbuffer<[u32]>> {
        Ok(Buffer::from_iter(
            renderer.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            indices.iter().cloned(),
        )?)
    }

    fn create_texture_desc_set(
        pipeline: Arc<GraphicsPipeline>,
        renderer: &Renderer,
        image: &gltf::image::Data,
    ) -> anyhow::Result<Arc<PersistentDescriptorSet>> {
        let (uploads, texture) = Self::load_image(renderer, image)?;

        uploads
            .execute(renderer.queue())?
            .then_signal_fence_and_flush()?
            .wait(None)?;

        let mip_levels = texture.image().mip_levels();
        Ok(PersistentDescriptorSet::new(
            renderer.descriptor_set_allocator(),
            pipeline.layout().set_layouts().first().unwrap().clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                texture,
                Sampler::new(
                    pipeline.device().clone(),
                    SamplerCreateInfo {
                        mag_filter: Filter::Linear,
                        min_filter: Filter::Linear,
                        mipmap_mode: SamplerMipmapMode::Linear,
                        address_mode: [SamplerAddressMode::Repeat; 3],
                        mip_lod_bias: 0.0,
                        lod: 0.0..=mip_levels as f32,
                        ..Default::default()
                    },
                )?,
            )],
            [],
        )?)
    }

    fn load_image(
        renderer: &Renderer,
        image: &gltf::image::Data,
    ) -> anyhow::Result<(Arc<PrimaryAutoCommandBuffer>, Arc<ImageView>)> {
        let upload_buffer = Buffer::new_slice(
            renderer.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            (image.width * image.height * 4) as DeviceSize,
        )?;
        upload_buffer.write()?.copy_from_slice(&image.pixels);

        let mut uploads = AutoCommandBufferBuilder::primary(
            renderer.command_buffer_allocator(),
            renderer.queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        let image = Image::new(
            renderer.memory_allocator(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::R8G8B8A8_SRGB,
                extent: [image.width, image.height, 1],
                usage: ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                mip_levels: (image.width.min(image.height) as f32).log2().floor() as u32 + 1,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )?;

        uploads.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            upload_buffer,
            image.clone(),
        ))?;

        let image = ImageView::new_default(image)?;

        Self::generate_mipmaps(&mut uploads, image.image(), image.image().extent())?;

        Ok((uploads.build()?, image))
    }

    fn generate_mipmaps(
        cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        image: &Arc<Image>,
        dimensions: [u32; 3],
    ) -> anyhow::Result<()> {
        let mut src_size = [dimensions[0], dimensions[1], dimensions[2]];
        for level in 1..image.mip_levels() {
            let dst_size = [
                if src_size[0] > 1 { src_size[0] / 2 } else { 1 },
                if src_size[1] > 1 { src_size[1] / 2 } else { 1 },
                src_size[2],
            ];
            cbb.blit_image(BlitImageInfo {
                regions: [ImageBlit {
                    src_subresource: ImageSubresourceLayers {
                        mip_level: level - 1,
                        ..image.subresource_layers()
                    },
                    src_offsets: [[0; 3], src_size],
                    dst_subresource: ImageSubresourceLayers {
                        mip_level: level,
                        ..image.subresource_layers()
                    },
                    dst_offsets: [[0; 3], dst_size],
                    ..Default::default()
                }]
                .into(),
                filter: Filter::Linear,
                ..BlitImageInfo::images(image.clone(), image.clone())
            })?;
            src_size[0] /= 2;
            src_size[1] /= 2;
        }

        Ok(())
    }
}
