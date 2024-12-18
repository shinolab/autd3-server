use wgpu::{Device, SurfaceConfiguration};

pub struct DepthTexture {
    view: wgpu::TextureView,
}

impl DepthTexture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(device: &Device, surface_config: &SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: surface_config.width.max(1),
            height: surface_config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { view }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}
