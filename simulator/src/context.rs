use std::sync::Arc;

use autd3_derive::Builder;
use winit::window::Window;

use crate::{error::SimulatorError, surface::SurfaceWrapper, State};

#[derive(Builder)]
pub struct Context {
    _instance: wgpu::Instance,
    _adapter: wgpu::Adapter,
    #[get(ref)]
    device: wgpu::Device,
    #[get(ref)]
    queue: wgpu::Queue,
}

impl Context {
    pub async fn init(
        state: &State,
        surface: &mut SurfaceWrapper,
        window: Arc<Window>,
    ) -> Result<Self, SimulatorError> {
        tracing::info!("Initializing wgpu...");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        surface.init_surface(&instance, window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(surface.surface()),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| SimulatorError::NoSuitableAdapter)?;

        let adapter_info = adapter.get_info();
        tracing::info!("Using {} ({:?})", adapter_info.name, adapter_info.backend);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    ..Default::default()
                },
                None,
            )
            .await?;

        surface.configure(state, window);

        Ok(Self {
            _instance: instance,
            _adapter: adapter,
            device,
            queue,
        })
    }
}
