pub(crate) use std::{
    collections::HashMap, f32::consts::PI, ffi::CString, path::PathBuf, sync::Arc, time::Instant,
};

pub(crate) use autd3_driver::{
    autd3_device::AUTD3, defined::ULTRASOUND_FREQUENCY, fpga::FPGA_CLK_FREQ, geometry::Geometry,
};
pub(crate) use autd3_firmware_emulator::{CPUEmulator, FPGAEmulator};

pub(crate) use bytemuck::{Pod, Zeroable};
pub(crate) use camera_controllers::{Camera, CameraPerspective, FirstPerson, FirstPersonSettings};
pub(crate) use cgmath::{Deg, Euler};
pub(crate) use scarlet::{color::RGBColor, colormap::ColorMap};
pub(crate) use strum::IntoEnumIterator;
pub(crate) use winit::{
    event::Event,
    event_loop::{EventLoop, EventLoopBuilder},
    window::{Window, WindowBuilder},
};

pub(crate) use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, BlitImageInfo,
        CommandBufferUsage, CopyBufferToImageInfo, ImageBlit, PrimaryAutoCommandBuffer,
        PrimaryCommandBufferAbstract,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
    },
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo, QueueFlags,
    },
    format::{Format, FormatFeatures},
    image::{
        sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
        view::ImageView,
        Image, ImageCreateInfo, ImageSubresourceLayers, ImageType, ImageUsage, SampleCount,
        SampleCounts,
    },
    instance::{
        debug::{
            DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessenger,
            DebugUtilsMessengerCallback, DebugUtilsMessengerCreateInfo,
        },
        Instance, InstanceCreateFlags, InstanceCreateInfo,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{
        compute::ComputePipelineCreateInfo,
        graphics::{
            color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState},
            depth_stencil::{DepthState, DepthStencilState},
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            multisample::MultisampleState,
            rasterization::RasterizationState,
            vertex_input::{Vertex, VertexDefinition},
            viewport::{Viewport, ViewportState},
            GraphicsPipelineCreateInfo,
        },
        layout::PipelineDescriptorSetLayoutCreateInfo,
        ComputePipeline, DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint,
        PipelineLayout, PipelineShaderStageCreateInfo,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    swapchain::{
        self, ColorSpace, FullScreenExclusive, PresentMode, Surface, SurfaceTransform, Swapchain,
        SwapchainCreateInfo, SwapchainPresentInfo,
    },
    sync::{self, GpuFuture},
    DeviceSize, Validated, VulkanError, VulkanLibrary,
};

pub(crate) use crate::{
    common::{
        camera_helper,
        coloring_method::{coloring_hsv, ColoringMethod},
        transform::{quaternion_to, to_gl_pos, to_gl_rot},
    },
    patch::{
        imgui_vulkano_renderer,
        imgui_winit_support::{HiDpiMode, WinitPlatform},
    },
    renderer::Renderer,
    sound_sources::{Drive, SoundSources},
    update_flag::UpdateFlag,
    viewer_settings::{ColorMapType, ViewerSettings},
    Matrix4, Quaternion, Vector3, Vector4, MILLIMETER, ZPARITY,
};
