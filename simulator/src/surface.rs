use std::sync::Arc;

use autd3_derive::Builder;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{context::Context, error::SimulatorError};

#[derive(Builder)]
pub struct SurfaceWrapper {
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
}

impl SurfaceWrapper {
    pub fn new() -> Self {
        Self {
            surface: None,
            config: None,
        }
    }

    pub fn surface(&self) -> &wgpu::Surface {
        self.surface.as_ref().unwrap()
    }

    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        self.config.as_ref().unwrap()
    }

    pub fn init_surface(
        &mut self,
        instance: &wgpu::Instance,
        window: Arc<Window>,
    ) -> Result<(), SimulatorError> {
        self.surface = Some(instance.create_surface(window)?);
        Ok(())
    }

    pub fn configure(&mut self, window: Arc<Window>) {
        let size = window.inner_size();
        self.config = Some(Self::surface_configuration(&size, true));
    }

    pub fn resize(&mut self, context: &Context, size: &PhysicalSize<u32>) {
        let config = self.config.as_mut().unwrap();
        config.width = size.width.max(1);
        config.height = size.height.max(1);
        let surface = self.surface.as_ref().unwrap();
        surface.configure(context.device(), config);
    }

    pub fn acquire(&mut self, context: &Context) -> Result<wgpu::SurfaceTexture, SimulatorError> {
        let surface = self.surface.as_ref().unwrap();

        match surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Timeout) => Ok(surface.get_current_texture()?),
            Err(
                wgpu::SurfaceError::Outdated
                | wgpu::SurfaceError::Lost
                | wgpu::SurfaceError::OutOfMemory,
            ) => {
                surface.configure(context.device(), self.config.as_ref().unwrap());
                Ok(surface.get_current_texture()?)
            }
        }
    }

    fn surface_configuration(size: &PhysicalSize<u32>, vsync: bool) -> wgpu::SurfaceConfiguration {
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: if vsync {
                wgpu::PresentMode::Fifo
            } else {
                wgpu::PresentMode::Immediate
            },
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
        }
    }
}
