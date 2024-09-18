use crate::prelude::*;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod, Vertex)]
struct SliceVertex {
    #[format(R32G32B32A32_SFLOAT)]
    position: [f32; 4],
    #[format(R32G32_SFLOAT)]
    tex_coords: [f32; 2],
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "./assets/shaders/slice.vert"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "./assets/shaders/slice.frag"
    }
}

pub struct SliceViewer {
    vertices: Subbuffer<[SliceVertex]>,
    indices: Subbuffer<[u32]>,
    pipeline: Arc<GraphicsPipeline>,
    model: Matrix4,
    field_image_view: Subbuffer<[[f32; 4]]>,
}

impl SliceViewer {
    pub fn new(renderer: &Renderer, settings: &ViewerSettings) -> anyhow::Result<Self> {
        let vertices = Self::create_vertices(renderer, settings)?;
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
                vertex_input_state: Some(SliceVertex::per_vertex().definition(&interface)?),
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
            pipeline,
            model: Matrix4::from_scale(1.),
            field_image_view: Self::create_field_image_view(
                renderer,
                [
                    (settings.slice_width / settings.slice_pixel_size) as u32,
                    (settings.slice_height / settings.slice_pixel_size) as u32,
                ],
            )?,
        })
    }

    pub fn init(&mut self, settings: &ViewerSettings) {
        self.update_pos(settings);
    }

    fn update_pos(&mut self, settings: &ViewerSettings) {
        self.model = Matrix4::from_translation(to_gl_pos(settings.slice_pos()))
            * Matrix4::from(to_gl_rot(settings.slice_rotation()));
    }

    pub const fn model(&self) -> Matrix4 {
        self.model
    }

    pub fn field_image_view(&self) -> Subbuffer<[[f32; 4]]> {
        self.field_image_view.clone()
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        settings: &ViewerSettings,
        update_flag: &UpdateFlag,
    ) -> anyhow::Result<()> {
        if update_flag.contains(UpdateFlag::UPDATE_SLICE_POS) {
            self.update_pos(settings);
        }

        if update_flag.contains(UpdateFlag::UPDATE_SLICE_SIZE) {
            self.field_image_view = Self::create_field_image_view(
                renderer,
                [
                    (settings.slice_width / settings.slice_pixel_size) as u32,
                    (settings.slice_height / settings.slice_pixel_size) as u32,
                ],
            )?;
            self.vertices = Self::create_vertices(renderer, settings)?;
            self.indices = Self::create_indices(renderer)?;
        }

        Ok(())
    }

    pub fn render(
        &mut self,
        renderer: &Renderer,
        view: Matrix4,
        proj: Matrix4,
        settings: &ViewerSettings,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> anyhow::Result<()> {
        builder
            .bind_pipeline_graphics(self.pipeline.clone())?
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                PersistentDescriptorSet::new(
                    renderer.descriptor_set_allocator(),
                    self.pipeline
                        .layout()
                        .set_layouts()
                        .first()
                        .unwrap()
                        .clone(),
                    [WriteDescriptorSet::buffer(0, self.field_image_view())],
                    [],
                )?,
            )?
            .push_constants(
                self.pipeline.layout().clone(),
                0,
                fs::PushConstsConfig {
                    pvm: (proj * view * self.model).into(),
                    width: (settings.slice_width / settings.slice_pixel_size) as _,
                    height: (settings.slice_height / settings.slice_pixel_size) as _,
                },
            )?
            .bind_vertex_buffers(0, self.vertices.clone())?
            .bind_index_buffer(self.indices.clone())?
            .draw_indexed(self.indices.len() as u32, 1, 0, 0, 0)?;

        Ok(())
    }

    fn create_field_image_view(
        renderer: &Renderer,
        view_size: [u32; 2],
    ) -> anyhow::Result<Subbuffer<[[f32; 4]]>> {
        Ok(Buffer::from_iter(
            renderer.memory_allocator(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            vec![[0., 0., 0., 1.]; view_size[0] as usize * view_size[1] as usize],
        )?)
    }

    fn create_vertices(
        renderer: &Renderer,
        settings: &ViewerSettings,
    ) -> anyhow::Result<Subbuffer<[SliceVertex]>> {
        let width = settings.slice_width;
        let height = settings.slice_height;
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
                SliceVertex {
                    position: [-width / 2.0, -height / 2.0, 0.0, 1.0],
                    tex_coords: [0.0, 0.0],
                },
                SliceVertex {
                    position: [width / 2.0, -height / 2.0, 0.0, 1.0],
                    tex_coords: [1.0, 0.0],
                },
                SliceVertex {
                    position: [width / 2.0, height / 2.0, 0.0, 1.0],
                    tex_coords: [1.0, 1.0],
                },
                SliceVertex {
                    position: [-width / 2.0, height / 2.0, 0.0, 1.0],
                    tex_coords: [0.0, 1.0],
                },
            ],
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
            [0, 2, 1, 0, 3, 2],
        )?)
    }
}
