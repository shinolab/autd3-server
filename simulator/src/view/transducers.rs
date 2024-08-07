use crate::prelude::*;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod, Vertex)]
struct CircleVertex {
    #[format(R32G32B32A32_SFLOAT)]
    position: [f32; 4],
    #[format(R32G32_SFLOAT)]
    tex_coords: [f32; 2],
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod, Vertex)]
struct ModelInstanceData {
    #[format(R32G32B32A32_SFLOAT)]
    model: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod, Vertex)]
struct ColorInstanceData {
    #[format(R32G32B32A32_SFLOAT)]
    color: [f32; 4],
}

#[allow(clippy::needless_question_mark)]
mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "./assets/shaders/circle.vert"
    }
}

#[allow(clippy::needless_question_mark)]
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "./assets/shaders/circle.frag"
    }
}

pub struct TransViewer {
    vertices: Subbuffer<[CircleVertex]>,
    indices: Subbuffer<[u32]>,
    model_instance_data: Option<Subbuffer<[ModelInstanceData]>>,
    color_instance_data: Option<Subbuffer<[ColorInstanceData]>>,
    pipeline: Arc<GraphicsPipeline>,
    texture_desc_set: Arc<PersistentDescriptorSet>,
    coloring_method: ColoringMethod,
}

impl TransViewer {
    pub fn new(renderer: &Renderer) -> anyhow::Result<Self> {
        let vertices = Self::create_vertices(renderer)?;
        let indices = Self::create_indices(renderer)?;

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
                vertex_input_state: Some(
                    [
                        CircleVertex::per_vertex(),
                        ModelInstanceData::per_instance(),
                        ColorInstanceData::per_instance(),
                    ]
                    .definition(&interface)?,
                ),
                input_assembly_state: Some(InputAssemblyState {
                    topology: PrimitiveTopology::TriangleStrip,
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
            vertices,
            indices,
            model_instance_data: None,
            color_instance_data: None,
            texture_desc_set: Self::create_texture_desc_set(pipeline.clone(), renderer)?,
            pipeline,
            coloring_method: coloring_hsv,
        })
    }

    pub fn init(&mut self, renderer: &Renderer, sources: &SoundSources) -> anyhow::Result<()> {
        let size = sources.len();
        self.color_instance_data = Some(Self::create_color_instance_data(renderer, size)?);
        self.update_color_instance_data(sources)?;
        self.model_instance_data = Some(Self::create_model_instance_data(renderer, size)?);
        self.update_model_instance_data(sources)?;
        Ok(())
    }

    pub fn update(
        &mut self,
        sources: &SoundSources,
        update_flag: &UpdateFlag,
    ) -> anyhow::Result<()> {
        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_ALPHA)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_FLAG)
        {
            self.update_color_instance_data(sources)?;
        }
        Ok(())
    }

    pub fn update_source_pos(&mut self, sources: &SoundSources) -> anyhow::Result<()> {
        self.update_model_instance_data(sources)
    }

    pub fn render(
        &mut self,
        view: Matrix4,
        proj: Matrix4,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> anyhow::Result<()> {
        if let (Some(model), Some(color)) = (&self.model_instance_data, &self.color_instance_data) {
            builder
                .bind_pipeline_graphics(self.pipeline.clone())?
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    self.pipeline.layout().clone(),
                    0,
                    self.texture_desc_set.clone(),
                )?
                .push_constants(
                    self.pipeline.layout().clone(),
                    0,
                    vs::PushConsts {
                        proj_view: (proj * view).into(),
                    },
                )?
                .bind_vertex_buffers(0, (self.vertices.clone(), model.clone(), color.clone()))?
                .bind_index_buffer(self.indices.clone())?
                .draw_indexed(self.indices.len() as u32, model.len() as u32, 0, 0, 0)?;
        }
        Ok(())
    }

    fn create_model_instance_data(
        renderer: &Renderer,
        size: usize,
    ) -> anyhow::Result<Subbuffer<[ModelInstanceData]>> {
        Ok(Buffer::from_iter(
            renderer.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER | BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            (0..size).map(|_| ModelInstanceData::default()),
        )?)
    }

    fn create_color_instance_data(
        renderer: &Renderer,
        size: usize,
    ) -> anyhow::Result<Subbuffer<[ColorInstanceData]>> {
        Ok(Buffer::from_iter(
            renderer.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER | BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            (0..size).map(|_| ColorInstanceData::default()),
        )?)
    }

    fn create_vertices(renderer: &Renderer) -> anyhow::Result<Subbuffer<[CircleVertex]>> {
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
            [
                CircleVertex {
                    position: [-1.0, -1.0, 0.0, 1.0],
                    tex_coords: [0.0, 1.0],
                },
                CircleVertex {
                    position: [1.0, -1.0, 0.0, 1.0],
                    tex_coords: [1.0, 1.0],
                },
                CircleVertex {
                    position: [1.0, 1.0, 0.0, 1.0],
                    tex_coords: [1.0, 0.0],
                },
                CircleVertex {
                    position: [-1.0, 1.0, 0.0, 1.0],
                    tex_coords: [0.0, 0.0],
                },
            ]
            .into_iter(),
        )?)
    }

    fn create_indices(renderer: &Renderer) -> anyhow::Result<Subbuffer<[u32]>> {
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
            [0, 1, 2, 2, 3, 0],
        )?)
    }

    fn create_texture_desc_set(
        pipeline: Arc<GraphicsPipeline>,
        renderer: &Renderer,
    ) -> anyhow::Result<Arc<PersistentDescriptorSet>> {
        let (uploads, texture) = Self::load_image(renderer)?;

        uploads
            .execute(renderer.queue())?
            .then_signal_fence_and_flush()?
            .wait(None)?;

        let set = PersistentDescriptorSet::new(
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
                        mipmap_mode: SamplerMipmapMode::Nearest,
                        address_mode: [SamplerAddressMode::Repeat; 3],
                        mip_lod_bias: 0.0,
                        ..Default::default()
                    },
                )?,
            )],
            [],
        )?;
        Ok(set)
    }

    fn load_image(
        renderer: &Renderer,
    ) -> anyhow::Result<(Arc<PrimaryAutoCommandBuffer>, Arc<ImageView>)> {
        let decoder =
            png::Decoder::new(include_bytes!("../../assets/textures/circle.png").as_slice());

        let mut reader = decoder.read_info()?;

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
            (reader.info().width * reader.info().height * 4) as DeviceSize,
        )?;
        reader.next_frame(&mut upload_buffer.write()?)?;

        let image = Image::new(
            renderer.memory_allocator(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::R8G8B8A8_SRGB,
                extent: [reader.info().width, reader.info().height, 1],
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )?;

        let mut uploads = AutoCommandBufferBuilder::primary(
            renderer.command_buffer_allocator(),
            renderer.queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        uploads.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            upload_buffer,
            image.clone(),
        ))?;

        Ok((uploads.build()?, ImageView::new_default(image)?))
    }

    fn update_color_instance_data(&mut self, sources: &SoundSources) -> anyhow::Result<()> {
        if let Some(data) = &mut self.color_instance_data {
            data.write()?
                .iter_mut()
                .zip(sources.drives().zip(sources.visibilities()))
                .for_each(|(d, (drive, &v))| {
                    d.color = (self.coloring_method)(drive.phase / (2.0 * PI), drive.amp, v);
                });
        }
        Ok(())
    }

    fn update_model_instance_data(&mut self, sources: &SoundSources) -> anyhow::Result<()> {
        if let Some(data) = &mut self.model_instance_data {
            data.write()?
                .iter_mut()
                .zip(sources.positions().zip(sources.rotations()))
                .for_each(|(d, (pos, rot))| {
                    let s = 0.5 * AUTD3::TRANS_SPACING;
                    let mut m = Matrix4::from_scale(s);
                    m[3][0] = pos[0];
                    m[3][1] = pos[1];
                    m[3][2] = pos[2];
                    let rotm = Matrix4::from(*rot);
                    d.model = (m * rotm).into();
                });
        }
        Ok(())
    }
}
