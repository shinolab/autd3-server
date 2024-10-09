use thiserror::Error;

use crate::imgui_wgpu;

#[derive(Error, Debug)]
pub enum SimulatorError {
    #[error("{0}")]
    OsError(#[from] winit::error::OsError),
    #[error("{0}")]
    EventLoopError(#[from] winit::error::EventLoopError),
    #[error("{0}")]
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),
    #[error("{0}")]
    RequestDeviceError(#[from] wgpu::RequestDeviceError),
    #[error("{0}")]
    SurfaceError(#[from] wgpu::SurfaceError),
    #[error("{0}")]
    ExternalError(#[from] winit::error::ExternalError),
    #[error("{0}")]
    ImguiRendererError(#[from] imgui_wgpu::RendererError),
    #[error("{0}")]
    ImageError(#[from] image::ImageError),
    #[error("{0}")]
    AUTDProtoBufError(#[from] autd3_protobuf::AUTDProtoBufError),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to find adapter")]
    NoSuitableAdapter,
}
