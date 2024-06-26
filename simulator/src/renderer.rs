use crate::prelude::*;

/// List available GPUs
pub fn available_gpus() -> anyhow::Result<Vec<(u32, String, PhysicalDeviceType)>> {
    let event_loop = EventLoopBuilder::<()>::new().build();

    let library = VulkanLibrary::new()?;
    let required_extensions = Surface::required_extensions(&event_loop);
    let instance = Instance::new(
        library,
        InstanceCreateInfo {
            flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
            enabled_extensions: required_extensions,
            ..Default::default()
        },
    )?;

    let window = Arc::new(
        WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(1, 1))
            .with_title("tmp")
            .with_visible(false)
            .build(&event_loop)?,
    );
    let surface = Surface::from_window(instance.clone(), window.clone())?;

    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ..DeviceExtensions::default()
    };

    let available_properties = instance
        .enumerate_physical_devices()?
        .filter(|p| p.supported_extensions().contains(&device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.intersects(QueueFlags::GRAPHICS)
                        && p.surface_support(i as u32, &surface).unwrap_or(false)
                })
                .map(|i| (p, i as u32))
        })
        .collect::<Vec<_>>();

    Ok(available_properties
        .iter()
        .map(|(p, idx)| {
            (
                *idx,
                p.properties().device_name.to_owned(),
                p.properties().device_type,
            )
        })
        .collect())
}

pub struct Renderer {
    device: Arc<Device>,
    surface: Arc<Surface>,
    queue: Arc<Queue>,
    swap_chain: Arc<Swapchain>,
    image_index: u32,
    images: Vec<Arc<Image>>,
    recreate_swapchain: bool,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    frame_buffers: Vec<Arc<Framebuffer>>,
    render_pass: Arc<RenderPass>,
    viewport: Viewport,
    command_buffer_allocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
    memory_allocator: Arc<StandardMemoryAllocator>,
    camera: Camera<f32>,
    _debug_callback: Option<DebugUtilsMessenger>,
    msaa_sample: SampleCounts,
    depth_format: Format,
}

impl Renderer {
    pub fn new(
        event_loop: &EventLoop<()>,
        title: &str,
        width: f64,
        height: f64,
        v_sync: bool,
        gpu_idx: i32,
    ) -> anyhow::Result<Self> {
        let library = VulkanLibrary::new()?;
        let mut required_extensions = Surface::required_extensions(&event_loop);
        if cfg!(feature = "enable_debug") {
            required_extensions.ext_debug_utils = true;
        }

        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                enabled_extensions: required_extensions,
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                ..Default::default()
            },
        )?;

        let _debug_callback = if cfg!(feature = "enable_debug") {
            unsafe {
                DebugUtilsMessenger::new(
                    instance.clone(),
                    DebugUtilsMessengerCreateInfo {
                        message_severity: DebugUtilsMessageSeverity::ERROR
                            | DebugUtilsMessageSeverity::WARNING
                            | DebugUtilsMessageSeverity::INFO
                            | DebugUtilsMessageSeverity::VERBOSE,
                        message_type: DebugUtilsMessageType::GENERAL
                            | DebugUtilsMessageType::VALIDATION
                            | DebugUtilsMessageType::PERFORMANCE,
                        ..DebugUtilsMessengerCreateInfo::user_callback(
                            DebugUtilsMessengerCallback::new(
                                |message_severity, message_type, callback_data| {
                                    let severity = if message_severity
                                        .intersects(DebugUtilsMessageSeverity::ERROR)
                                    {
                                        "error"
                                    } else if message_severity
                                        .intersects(DebugUtilsMessageSeverity::WARNING)
                                    {
                                        "warning"
                                    } else if message_severity
                                        .intersects(DebugUtilsMessageSeverity::INFO)
                                    {
                                        "information"
                                    } else if message_severity
                                        .intersects(DebugUtilsMessageSeverity::VERBOSE)
                                    {
                                        "verbose"
                                    } else {
                                        panic!("no-impl");
                                    };

                                    let ty = if message_type
                                        .intersects(DebugUtilsMessageType::GENERAL)
                                    {
                                        "general"
                                    } else if message_type
                                        .intersects(DebugUtilsMessageType::VALIDATION)
                                    {
                                        "validation"
                                    } else if message_type
                                        .intersects(DebugUtilsMessageType::PERFORMANCE)
                                    {
                                        "performance"
                                    } else {
                                        panic!("no-impl");
                                    };

                                    println!(
                                        "{} {} {}: {}",
                                        callback_data.message_id_name.unwrap_or("unknown"),
                                        ty,
                                        severity,
                                        callback_data.message
                                    );
                                },
                            ),
                        )
                    },
                )
                .ok()
            }
        } else {
            None
        };

        let window = Arc::new(
            WindowBuilder::new()
                .with_inner_size(winit::dpi::LogicalSize::new(width, height))
                .with_title(title)
                .build(event_loop)?,
        );
        let surface = Surface::from_window(instance.clone(), window.clone())?;

        let (device, queue) = Self::create_device(gpu_idx, instance, surface.clone())?;

        let msaa_sample = Self::get_max_usable_sample_count(device.physical_device().clone());

        let mut viewport = Viewport {
            offset: [0.0, 0.0],
            extent: [0.0, 0.0],
            depth_range: 0.0..=1.0,
        };
        let (swap_chain, images) = Self::create_swap_chain(
            surface.clone(),
            device.physical_device().clone(),
            device.clone(),
            if v_sync {
                PresentMode::Fifo
            } else {
                PresentMode::Immediate
            },
        )?;
        let depth_format = [
            Format::D32_SFLOAT,
            Format::D32_SFLOAT_S8_UINT,
            Format::D24_UNORM_S8_UINT,
            Format::D32_SFLOAT,
        ]
        .into_iter()
        .find(|&f| {
            if let Ok(props) = device.physical_device().format_properties(f) {
                props
                    .optimal_tiling_features
                    .contains(FormatFeatures::DEPTH_STENCIL_ATTACHMENT)
            } else {
                false
            }
        })
        .unwrap_or(Format::D16_UNORM);

        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                intermediary: {
                    format: swap_chain.image_format(),
                    samples: msaa_sample.max_count(),
                    load_op: Clear,
                    store_op: DontCare,
                },
                depth_stencil: {
                    format: depth_format,
                    samples: msaa_sample.max_count(),
                    load_op: Clear,
                    store_op: DontCare,
                },
                color: {
                    format: swap_chain.image_format(),
                    samples: 1,
                    load_op: DontCare,
                    store_op: Store,
                }
            },
            pass: {
                color: [intermediary],
                color_resolve: [color],
                depth_stencil: {depth_stencil},
            }
        )?;
        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
        let frame_buffers = Self::window_size_dependent_setup(
            memory_allocator,
            &images,
            render_pass.clone(),
            &mut viewport,
            swap_chain.image_format(),
            depth_format,
            msaa_sample.max_count(),
        )?;

        let mut camera =
            FirstPerson::new([0., -500.0, 120.0], FirstPersonSettings::keyboard_wasd()).camera(0.);
        camera.set_yaw_pitch(0., -std::f32::consts::PI / 2.0);

        let command_buffer_allocator =
            StandardCommandBufferAllocator::new(queue.device().clone(), Default::default());
        let descriptor_set_allocator =
            StandardDescriptorSetAllocator::new(queue.device().clone(), Default::default());
        let memory_allocator =
            Arc::new(StandardMemoryAllocator::new_default(queue.device().clone()));

        let previous_frame_end = Some(sync::now(device.clone()).boxed());
        Ok(Renderer {
            device,
            surface,
            queue,
            swap_chain,
            image_index: 0,
            images,
            previous_frame_end,
            recreate_swapchain: false,
            frame_buffers,
            render_pass,
            viewport,
            command_buffer_allocator,
            descriptor_set_allocator,
            memory_allocator,
            depth_format,
            camera,
            _debug_callback,
            msaa_sample,
        })
    }

    fn get_max_usable_sample_count(physical: Arc<PhysicalDevice>) -> SampleCounts {
        let properties = physical.properties();
        let counts =
            properties.framebuffer_color_sample_counts & properties.framebuffer_depth_sample_counts;
        [
            SampleCounts::SAMPLE_64,
            SampleCounts::SAMPLE_32,
            SampleCounts::SAMPLE_16,
            SampleCounts::SAMPLE_8,
            SampleCounts::SAMPLE_4,
            SampleCounts::SAMPLE_2,
        ]
        .into_iter()
        .find(|c| counts.contains(*c))
        .unwrap_or(SampleCounts::SAMPLE_1)
    }

    pub fn get_projection(&self, settings: &ViewerSettings) -> Matrix4 {
        let draw_size = self.window().inner_size();
        Matrix4::from({
            let mut projection = CameraPerspective {
                fov: settings.camera_fov,
                near_clip: settings.camera_near_clip,
                far_clip: settings.camera_far_clip,
                aspect_ratio: (draw_size.width as f32) / (draw_size.height as f32),
            }
            .projection();
            projection[0][1] = -projection[0][1];
            projection[1][1] = -projection[1][1];
            projection[2][1] = -projection[2][1];
            projection
        })
    }

    pub fn get_view(&self) -> Matrix4 {
        Matrix4::from(self.camera.orthogonal())
    }

    fn create_device(
        gpu_idx: i32,
        instance: Arc<Instance>,
        surface: Arc<Surface>,
    ) -> anyhow::Result<(Arc<Device>, Arc<Queue>)> {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::default()
        };

        let available_properties = instance
            .enumerate_physical_devices()?
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.intersects(QueueFlags::GRAPHICS)
                            && p.surface_support(i as u32, &surface).unwrap_or(false)
                    })
                    .map(|i| (p, i as u32))
            })
            .collect::<Vec<_>>();

        let (physical_device, queue_family) = match gpu_idx {
            idx if idx < 0 => available_properties
                .into_iter()
                .min_by_key(|(p, _)| match p.properties().device_type {
                    PhysicalDeviceType::DiscreteGpu => 0,
                    PhysicalDeviceType::IntegratedGpu => 1,
                    PhysicalDeviceType::VirtualGpu => 2,
                    PhysicalDeviceType::Cpu => 3,
                    PhysicalDeviceType::Other => 4,
                    _ => 5,
                })
                .unwrap(),
            idx if (idx as usize) < available_properties.len() => {
                available_properties[gpu_idx as usize].clone()
            }
            _ => {
                tracing::warn!("GPU {} not found. Using default GPU.", gpu_idx);
                available_properties
                    .into_iter()
                    .min_by_key(|(p, _)| match p.properties().device_type {
                        PhysicalDeviceType::DiscreteGpu => 0,
                        PhysicalDeviceType::IntegratedGpu => 1,
                        PhysicalDeviceType::VirtualGpu => 2,
                        PhysicalDeviceType::Cpu => 3,
                        PhysicalDeviceType::Other => 4,
                        _ => 5,
                    })
                    .unwrap()
            }
        };

        tracing::info!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        let features = Features::default();
        let (device, mut queues) = {
            Device::new(
                physical_device,
                DeviceCreateInfo {
                    enabled_extensions: device_extensions,
                    enabled_features: features,
                    queue_create_infos: vec![QueueCreateInfo {
                        queue_family_index: queue_family,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            )?
        };
        Ok((device, queues.next().unwrap()))
    }

    fn create_swap_chain(
        surface: Arc<Surface>,
        physical: Arc<PhysicalDevice>,
        device: Arc<Device>,
        present_mode: PresentMode,
    ) -> anyhow::Result<(Arc<Swapchain>, Vec<Arc<Image>>)> {
        let caps = physical.surface_capabilities(&surface, Default::default())?;
        let alpha = caps.supported_composite_alpha.into_iter().next().unwrap();
        let format = physical
            .surface_formats(&surface, Default::default())?
            .into_iter()
            .find(|&(f, c)| f == Format::B8G8R8A8_UNORM && c == ColorSpace::SrgbNonLinear);
        let image_extent: [u32; 2] = surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
            .inner_size()
            .into();
        Ok(Swapchain::new(
            device,
            surface,
            SwapchainCreateInfo {
                min_image_count: caps.min_image_count,
                image_format: format.map_or(Format::B8G8R8A8_UNORM, |f| f.0),
                image_color_space: format.map_or(ColorSpace::SrgbNonLinear, |f| f.1),
                image_extent,
                image_array_layers: 1,
                image_usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
                pre_transform: SurfaceTransform::Identity,
                composite_alpha: alpha,
                present_mode,
                clipped: true,
                full_screen_exclusive: FullScreenExclusive::Default,
                ..Default::default()
            },
        )?)
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn window(&self) -> &Window {
        self.surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
    }

    pub fn queue(&self) -> Arc<Queue> {
        self.queue.clone()
    }

    pub fn frame_buffer(&self) -> Arc<Framebuffer> {
        self.frame_buffers[self.image_index as usize].clone()
    }

    pub fn image(&self) -> Arc<Image> {
        self.images[self.image_index as usize].clone()
    }

    pub fn render_pass(&self) -> Arc<RenderPass> {
        self.render_pass.clone()
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport.clone()
    }

    pub const fn command_buffer_allocator(&self) -> &StandardCommandBufferAllocator {
        &self.command_buffer_allocator
    }

    pub const fn descriptor_set_allocator(&self) -> &StandardDescriptorSetAllocator {
        &self.descriptor_set_allocator
    }

    pub fn memory_allocator(&self) -> Arc<StandardMemoryAllocator> {
        self.memory_allocator.clone()
    }

    pub fn resize(&mut self) {
        self.recreate_swapchain = true
    }

    pub fn color_format(&self) -> Format {
        self.swap_chain.image_format()
    }

    pub const fn sample_count(&self) -> SampleCount {
        self.msaa_sample.max_count()
    }

    pub fn start_frame(&mut self) -> anyhow::Result<Box<dyn GpuFuture>> {
        if self.recreate_swapchain {
            self.recreate_swapchain_and_views()?;
        }

        let (image_num, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.swap_chain.clone(), None) {
                Ok(r) => r,
                Err(Validated::Error(VulkanError::OutOfDate)) => {
                    self.recreate_swapchain = true;
                    return Err(VulkanError::OutOfDate.into());
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        if suboptimal {
            self.recreate_swapchain = true;
        }
        self.image_index = image_num as _;

        let future = self.previous_frame_end.take().unwrap().join(acquire_future);

        Ok(future.boxed())
    }

    pub fn finish_frame(&mut self, after_future: Box<dyn GpuFuture>) {
        let future = after_future
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(
                    self.swap_chain.clone(),
                    self.image_index,
                ),
            )
            .then_signal_fence_and_flush();
        match future {
            Ok(future) => {
                match future.wait(None) {
                    Ok(x) => x,
                    Err(err) => println!("{:?}", err),
                }
                self.previous_frame_end = Some(future.boxed());
            }
            Err(Validated::Error(VulkanError::OutOfDate)) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
        }
    }

    fn recreate_swapchain_and_views(&mut self) -> anyhow::Result<()> {
        let dimensions: [u32; 2] = self.window().inner_size().into();
        let (new_swapchain, new_images) = self.swap_chain.recreate(SwapchainCreateInfo {
            image_extent: dimensions,
            ..self.swap_chain.create_info()
        })?;

        self.swap_chain = new_swapchain;
        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(self.device.clone()));
        let format = self.color_format();
        self.frame_buffers = Self::window_size_dependent_setup(
            memory_allocator,
            &new_images,
            self.render_pass.clone(),
            &mut self.viewport,
            format,
            self.depth_format,
            self.msaa_sample.max_count(),
        )?;
        self.images = new_images;
        self.recreate_swapchain = false;
        Ok(())
    }

    fn window_size_dependent_setup(
        memory_allocator: Arc<StandardMemoryAllocator>,
        images: &[Arc<Image>],
        render_pass: Arc<RenderPass>,
        viewport: &mut Viewport,
        color_format: Format,
        depth_format: Format,
        samples: SampleCount,
    ) -> anyhow::Result<Vec<Arc<Framebuffer>>> {
        let extent = images[0].extent();
        viewport.extent = [extent[0] as f32, extent[1] as f32];

        let color_image = ImageView::new_default(Image::new(
            memory_allocator.clone(),
            ImageCreateInfo {
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
                format: color_format,
                extent,
                samples,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )?)?;

        let depth_buffer = ImageView::new_default(Image::new(
            memory_allocator.clone(),
            ImageCreateInfo {
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::TRANSFER_DST,
                format: depth_format,
                extent,
                samples,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )?)?;

        Ok(images
            .iter()
            .map(|image| {
                let view = ImageView::new_default(image.clone())?;
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![color_image.clone(), depth_buffer.clone(), view],
                        ..Default::default()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn move_camera(&mut self, viewer_settings: &ViewerSettings) {
        camera_helper::set_camera(
            &mut self.camera,
            Vector3::new(
                viewer_settings.camera_pos_x,
                viewer_settings.camera_pos_y,
                viewer_settings.camera_pos_z,
            ),
            Vector3::new(
                viewer_settings.camera_rot_x,
                viewer_settings.camera_rot_y,
                viewer_settings.camera_rot_z,
            ),
        );
    }

    pub(crate) fn image_format(&self) -> Format {
        self.swap_chain.image_format()
    }
}
