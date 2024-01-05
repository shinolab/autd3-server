/*
 * File: imgui_renderer.rs
 * Project: src
 * Created Date: 23/05/2023
 * Author: Shun Suzuki
 * -----
 * Last Modified: 05/01/2024
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2023 Shun Suzuki. All rights reserved.
 *
 */

use std::{collections::HashMap, ffi::CString, path::PathBuf, time::Instant};

use crate::{
    common::transform::quaternion_to,
    patch::{
        imgui_vulkano_renderer,
        imgui_winit_support::{HiDpiMode, WinitPlatform},
    },
    renderer::Renderer,
    sound_sources::SoundSources,
    update_flag::UpdateFlag,
    viewer_settings::{ColorMapType, ViewerSettings},
    Matrix4, Quaternion, Vector3, Vector4, MILLIMETER, ZPARITY,
};
use autd3_driver::fpga::FPGA_CLK_FREQ;
use autd3_firmware_emulator::{CPUEmulator, FPGAEmulator};
use cgmath::{Deg, Euler};
use imgui::{
    sys::{igDragFloat, igDragFloat2},
    ColorEditFlags, Context, FontConfig, FontGlyphRanges, FontSource, TextureId, TreeNodeFlags,
};
use scarlet::{color::RGBColor, colormap::ColorMap};
use strum::IntoEnumIterator;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
        PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract,
    },
    format::Format,
    image::{
        sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
        view::ImageView,
        Image, ImageCreateInfo, ImageType, ImageUsage,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    sync::GpuFuture,
    DeviceSize,
};
use winit::{event::Event, window::Window};

pub struct ImGuiRenderer {
    imgui: Context,
    platform: WinitPlatform,
    imgui_renderer: imgui_vulkano_renderer::Renderer,
    last_frame: Instant,
    font_size: f32,
    hidpi_factor: f32,
    do_update_font: bool,
    visible: Vec<bool>,
    enable: Vec<bool>,
    thermal: Vec<bool>,
    show_mod_plot: Vec<bool>,
    mod_plot_size: Vec<[f32; 2]>,
    real_time: u64,
    time_step: i32,
    initial_settings: ViewerSettings,
    color_map_texture_ids: HashMap<ColorMapType, TextureId>,
}

impl ImGuiRenderer {
    pub fn new(
        initial_settings: ViewerSettings,
        config_path: &Option<PathBuf>,
        renderer: &Renderer,
    ) -> anyhow::Result<Self> {
        let mut imgui = Context::create();
        if let Some(path) = config_path {
            imgui.set_ini_filename(path.join("imgui.ini"));
        }

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), renderer.window(), HiDpiMode::Default);

        let hidpi_factor = platform.hidpi_factor();
        let font_size = 16.0;

        imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        let mut imgui_renderer = imgui_vulkano_renderer::Renderer::init(
            &mut imgui,
            renderer.device(),
            renderer.queue(),
            renderer.image_format(),
        )?;

        const COLOR_MAP_SIZE: u32 = 100;
        let color_map_texture_ids = ColorMapType::iter()
            .map(|color| -> anyhow::Result<(ColorMapType, TextureId)> {
                let mut uploads = AutoCommandBufferBuilder::primary(
                    renderer.command_buffer_allocator(),
                    renderer.queue().queue_family_index(),
                    CommandBufferUsage::OneTimeSubmit,
                )?;
                let iter = (0..COLOR_MAP_SIZE).map(|x| x as f64 / COLOR_MAP_SIZE as f64);
                let color_map: Vec<RGBColor> = match color {
                    crate::viewer_settings::ColorMapType::Viridis => {
                        scarlet::colormap::ListedColorMap::viridis().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Magma => {
                        scarlet::colormap::ListedColorMap::magma().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Inferno => {
                        scarlet::colormap::ListedColorMap::inferno().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Plasma => {
                        scarlet::colormap::ListedColorMap::plasma().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Bluered => {
                        scarlet::colormap::ListedColorMap::bluered().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Breeze => {
                        scarlet::colormap::ListedColorMap::breeze().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Circle => {
                        scarlet::colormap::ListedColorMap::circle().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Earth => {
                        scarlet::colormap::ListedColorMap::earth().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Hell => {
                        scarlet::colormap::ListedColorMap::hell().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Mist => {
                        scarlet::colormap::ListedColorMap::mist().transform(iter)
                    }
                    crate::viewer_settings::ColorMapType::Turbo => {
                        scarlet::colormap::ListedColorMap::turbo().transform(iter)
                    }
                };

                let extent = [COLOR_MAP_SIZE, 1, 1];
                let texels = color_map
                    .iter()
                    .flat_map(|c| {
                        [
                            (c.r * 255.) as u8,
                            (c.g * 255.) as u8,
                            (c.b * 255.) as u8,
                            255,
                        ]
                    })
                    .collect::<Vec<_>>();

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
                    (extent[0] * 4) as DeviceSize,
                )?;

                upload_buffer.write()?.copy_from_slice(&texels);

                let image = Image::new(
                    renderer.memory_allocator(),
                    ImageCreateInfo {
                        image_type: ImageType::Dim2d,
                        format: Format::R8G8B8A8_SRGB,
                        extent,
                        usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                        ..Default::default()
                    },
                    AllocationCreateInfo::default(),
                )?;

                uploads.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                    upload_buffer,
                    image.clone(),
                ))?;

                uploads
                    .build()?
                    .execute(renderer.queue())?
                    .then_signal_fence_and_flush()?
                    .wait(None)?;

                let texture = (
                    ImageView::new_default(image)?,
                    Sampler::new(
                        renderer.device(),
                        SamplerCreateInfo {
                            mag_filter: Filter::Linear,
                            min_filter: Filter::Linear,
                            mipmap_mode: SamplerMipmapMode::Nearest,
                            address_mode: [SamplerAddressMode::Repeat; 3],
                            mip_lod_bias: 0.0,
                            ..Default::default()
                        },
                    )?,
                );
                let id = imgui_renderer.textures().insert(texture);
                Ok((color, id))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()?;

        Ok(Self {
            imgui,
            platform,
            imgui_renderer,
            last_frame: Instant::now(),
            font_size,
            hidpi_factor: hidpi_factor as _,
            do_update_font: true,
            visible: Vec::new(),
            enable: Vec::new(),
            thermal: Vec::new(),
            real_time: FPGAEmulator::ec_time_now(),
            time_step: 1000000,
            show_mod_plot: Vec::new(),
            mod_plot_size: Vec::new(),
            initial_settings,
            color_map_texture_ids,
        })
    }

    pub fn init(&mut self, dev_num: usize) {
        self.visible = vec![true; dev_num];
        self.enable = vec![true; dev_num];
        self.thermal = vec![false; dev_num];
        self.show_mod_plot = vec![false; dev_num];
        self.mod_plot_size = vec![[200., 50.]; dev_num];
    }

    pub fn resized<T>(&mut self, window: &Window, event: &Event<T>) {
        self.platform
            .handle_event(self.imgui.io_mut(), window, event);
    }

    pub fn prepare_frame(&mut self, window: &Window) -> anyhow::Result<()> {
        Ok(self.platform.prepare_frame(self.imgui.io_mut(), window)?)
    }

    pub fn update_delta_time(&mut self) {
        let now = Instant::now();
        self.imgui
            .io_mut()
            .update_delta_time(now.duration_since(self.last_frame));
        self.last_frame = now;
    }

    pub fn handle_event<T>(&mut self, window: &Window, event: &Event<T>) {
        self.platform
            .handle_event(self.imgui.io_mut(), window, event);
    }

    pub fn update(
        &mut self,
        cpus: &mut [CPUEmulator],
        sources: &mut SoundSources,
        body_pointer: &[usize],
        render: &Renderer,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        settings: &mut ViewerSettings,
    ) -> anyhow::Result<UpdateFlag> {
        let mut update_flag = UpdateFlag::empty();

        self.update_font(render)?;

        let io = self.imgui.io_mut();

        let fps = io.framerate;

        self.platform.prepare_frame(io, render.window())?;

        let system_time = FPGAEmulator::systime(self.real_time);
        let ui = self.imgui.new_frame();

        {
            let rotation = Quaternion::from(Euler {
                x: Deg(settings.camera_rot_x),
                y: Deg(settings.camera_rot_y),
                z: Deg(settings.camera_rot_z),
            });

            let r = rotation * Vector3::unit_x();
            let u = rotation * Vector3::unit_y();
            let f = rotation * Vector3::unit_z();

            if !ui.io().want_capture_mouse {
                let mouse_wheel = ui.io().mouse_wheel;
                let trans = -f * mouse_wheel * settings.camera_move_speed * ZPARITY;
                settings.camera_pos_x += trans.x;
                settings.camera_pos_y += trans.y;
                settings.camera_pos_z += trans.z;
            }

            if !ui.io().want_capture_mouse {
                let mouse_delta = ui.io().mouse_delta;
                if ui.io().mouse_down[0] {
                    if ui.io().key_shift {
                        let delta_x = mouse_delta[0] * settings.camera_move_speed / 3000.;
                        let delta_y = mouse_delta[1] * settings.camera_move_speed / 3000.;
                        let to = -r * delta_x + u * delta_y + f;
                        let rot = Euler::from(quaternion_to(f, to) * rotation);
                        settings.camera_rot_x = Deg::from(rot.x).0;
                        settings.camera_rot_y = Deg::from(rot.y).0;
                        settings.camera_rot_z = Deg::from(rot.z).0;
                    } else {
                        let delta_x = mouse_delta[0] * settings.camera_move_speed / 10.;
                        let delta_y = mouse_delta[1] * settings.camera_move_speed / 10.;
                        let trans = -r * delta_x + u * delta_y;
                        settings.camera_pos_x += trans.x;
                        settings.camera_pos_y += trans.y;
                        settings.camera_pos_z += trans.z;
                    }
                }
            }
        }

        let mut font_size = self.font_size;
        let mut update_font = false;
        ui.window("Dear ImGui").build(|| {
            if let Some(tab_bar) = ui.tab_bar("Settings") {
                if let Some(tab) = ui.tab_item("Slice") {
                    ui.text("Position");
                    unsafe {
                        if igDragFloat(
                            CString::new("X##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_pos_x as _,
                            1. * MILLIMETER,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        if igDragFloat(
                            CString::new("Y##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_pos_y as _,
                            1. * MILLIMETER,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        if igDragFloat(
                            CString::new("Z##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_pos_z as _,
                            1. * MILLIMETER,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                    }
                    ui.separator();

                    ui.text("Rotation");
                    unsafe {
                        if igDragFloat(
                            CString::new("RX##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_rot_x as _,
                            1.,
                            -180.,
                            180.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        if igDragFloat(
                            CString::new("RY##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_rot_y as _,
                            1.,
                            -180.,
                            180.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        if igDragFloat(
                            CString::new("RZ##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_rot_z as _,
                            1.,
                            -180.,
                            180.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                    }
                    ui.separator();

                    ui.text("Size");
                    unsafe {
                        if igDragFloat(
                            CString::new("Width##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_width as _,
                            1. * MILLIMETER,
                            1. * MILLIMETER,
                            2000. * MILLIMETER,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            if settings.slice_width < 1. * MILLIMETER {
                                settings.slice_width = 1. * MILLIMETER;
                            }
                            update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                        }
                        if igDragFloat(
                            CString::new("Height##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_height as _,
                            1. * MILLIMETER,
                            1. * MILLIMETER,
                            2000. * MILLIMETER,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            if settings.slice_height < 1. * MILLIMETER {
                                settings.slice_height = 1. * MILLIMETER;
                            }
                            update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                        }
                        if igDragFloat(
                            CString::new("Pixel size##Slice")
                                .unwrap()
                                .as_c_str()
                                .as_ptr(),
                            &mut settings.slice_pixel_size as _,
                            1. * MILLIMETER,
                            0.1 * MILLIMETER,
                            8. * MILLIMETER,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            if settings.slice_pixel_size < 0.1 * MILLIMETER {
                                settings.slice_height = 0.1 * MILLIMETER;
                            }
                            update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                        }
                    }
                    ui.separator();

                    ui.text("Color settings");
                    let items = [
                        "Bluered", "Breeze", "Circle", "Earth", "Hell", "Inferno", "Magma", "Mist",
                        "Plasma", "Turbo", "Viridis",
                    ];
                    let selected_idx = match settings.color_map_type {
                        ColorMapType::Bluered => 0,
                        ColorMapType::Breeze => 1,
                        ColorMapType::Circle => 2,
                        ColorMapType::Earth => 3,
                        ColorMapType::Hell => 4,
                        ColorMapType::Inferno => 5,
                        ColorMapType::Magma => 6,
                        ColorMapType::Mist => 7,
                        ColorMapType::Plasma => 8,
                        ColorMapType::Turbo => 9,
                        ColorMapType::Viridis => 10,
                    };
                    let mut selected = &items[selected_idx];
                    if let Some(cb) = ui.begin_combo("##Coloring", selected) {
                        items.iter().for_each(|cur| {
                            if selected == cur {
                                ui.set_item_default_focus();
                            }
                            let clicked =
                                ui.selectable_config(cur).selected(selected == cur).build();
                            if clicked {
                                selected = cur;
                            }
                        });
                        match *selected {
                            "Bluered" => {
                                settings.color_map_type = ColorMapType::Bluered;
                            }
                            "Breeze" => {
                                settings.color_map_type = ColorMapType::Breeze;
                            }
                            "Circle" => {
                                settings.color_map_type = ColorMapType::Circle;
                            }
                            "Earth" => {
                                settings.color_map_type = ColorMapType::Earth;
                            }
                            "Hell" => {
                                settings.color_map_type = ColorMapType::Hell;
                            }
                            "Inferno" => {
                                settings.color_map_type = ColorMapType::Inferno;
                            }
                            "Magma" => {
                                settings.color_map_type = ColorMapType::Magma;
                            }
                            "Mist" => {
                                settings.color_map_type = ColorMapType::Mist;
                            }
                            "Plasma" => {
                                settings.color_map_type = ColorMapType::Plasma;
                            }
                            "Turbo" => {
                                settings.color_map_type = ColorMapType::Turbo;
                            }
                            "Viridis" => {
                                settings.color_map_type = ColorMapType::Viridis;
                            }
                            _ => {
                                settings.color_map_type = ColorMapType::Inferno;
                            }
                        }
                        update_flag.set(UpdateFlag::UPDATE_COLOR_MAP, true);
                        cb.end();
                    }
                    let w = ui.item_rect_size()[0];
                    ui.same_line();
                    ui.text("Coloring");
                    imgui::Image::new(
                        self.color_map_texture_ids[&settings.color_map_type],
                        [w, 10.0],
                    )
                    .build(ui);
                    unsafe {
                        igDragFloat(
                            CString::new("Max pressure [Pa]##Slice")
                                .unwrap()
                                .as_c_str()
                                .as_ptr(),
                            &mut settings.pressure_max as _,
                            1000.,
                            0.0,
                            std::f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                        if igDragFloat(
                            CString::new("Alpha##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_alpha as _,
                            0.01,
                            0.0,
                            1.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_COLOR_MAP, true);
                        }
                    }
                    ui.separator();

                    if ui.small_button("xy") {
                        settings.slice_rot_x = 0.;
                        settings.slice_rot_y = 0.;
                        settings.slice_rot_z = 0.;
                        update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                    }
                    ui.same_line();
                    if ui.small_button("yz") {
                        settings.slice_rot_x = 0.;
                        settings.slice_rot_y = 90.;
                        settings.slice_rot_z = 0.;
                        update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                    }
                    ui.same_line();
                    if ui.small_button("zx") {
                        settings.slice_rot_x = 90.;
                        settings.slice_rot_y = 0.;
                        settings.slice_rot_z = 0.;
                        update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                    }

                    tab.end();
                }

                if let Some(tab) = ui.tab_item("Camera") {
                    ui.text("Position");
                    unsafe {
                        igDragFloat(
                            CString::new("X##Camera").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_pos_x as _,
                            1. * MILLIMETER,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                        igDragFloat(
                            CString::new("Y##Camera").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_pos_y as _,
                            1. * MILLIMETER,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                        igDragFloat(
                            CString::new("Z##Camera").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_pos_z as _,
                            1. * MILLIMETER,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                    }
                    ui.separator();

                    ui.text("Rotation");
                    unsafe {
                        igDragFloat(
                            CString::new("RX##Camera").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_rot_x as _,
                            1.,
                            -180.,
                            180.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                        igDragFloat(
                            CString::new("RY##Camera").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_rot_y as _,
                            1.,
                            -180.,
                            180.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                        igDragFloat(
                            CString::new("RZ##Camera").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_rot_z as _,
                            1.,
                            -180.,
                            180.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                    }
                    ui.separator();

                    unsafe {
                        igDragFloat(
                            CString::new("Move speed").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_move_speed as _,
                            1.,
                            1. * MILLIMETER,
                            100. * MILLIMETER,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                    }

                    ui.separator();

                    ui.text("Perspective");
                    unsafe {
                        igDragFloat(
                            CString::new("FOV").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_fov as _,
                            1.,
                            0.,
                            180.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                        igDragFloat(
                            CString::new("Near clip").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_near_clip as _,
                            1. * MILLIMETER,
                            0.,
                            std::f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                        igDragFloat(
                            CString::new("Far clip").unwrap().as_c_str().as_ptr(),
                            &mut settings.camera_far_clip as _,
                            1. * MILLIMETER,
                            0.,
                            std::f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        );
                    }
                    tab.end();
                }

                if let Some(tab) = ui.tab_item("Config") {
                    unsafe {
                        if igDragFloat(
                            CString::new("Sound speed").unwrap().as_c_str().as_ptr(),
                            &mut settings.sound_speed as _,
                            1. * MILLIMETER,
                            0.,
                            std::f32::MAX / 2.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            cpus.iter().for_each(|cpu| {
                                sources
                                    .drives_mut()
                                    .skip(body_pointer[cpu.idx()])
                                    .for_each(|s| {
                                        s.set_wave_number(40e3, settings.sound_speed);
                                    });
                            });
                            update_flag.set(UpdateFlag::UPDATE_SOURCE_DRIVE, true);
                        }
                    }
                    ui.separator();

                    unsafe {
                        if igDragFloat(
                            CString::new("Font size").unwrap().as_c_str().as_ptr(),
                            &mut font_size as _,
                            1.,
                            1.,
                            256.,
                            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                            0,
                        ) {
                            update_font = true;
                        }
                    }
                    ui.separator();

                    ui.text("Device index: show/enable/overheat");
                    cpus.iter_mut().enumerate().for_each(|(i, cpu)| {
                        ui.text(format!("Device {}: ", i));
                        ui.same_line();
                        if ui.checkbox(format!("##show{}", i), &mut self.visible[i]) {
                            update_flag.set(UpdateFlag::UPDATE_SOURCE_FLAG, true);
                            let v = if self.visible[i] { 1. } else { 0. };
                            sources
                                .visibilities_mut()
                                .skip(body_pointer[i])
                                .for_each(|s| *s = v);
                        }
                        ui.same_line();
                        if ui.checkbox(format!("##enable{}", i), &mut self.enable[i]) {
                            update_flag.set(UpdateFlag::UPDATE_SOURCE_FLAG, true);
                            let v = if self.enable[i] { 1. } else { 0. };
                            sources
                                .drives_mut()
                                .skip(body_pointer[i])
                                .for_each(|s| s.enable = v);
                        }
                        ui.same_line();
                        if ui.checkbox(format!("##overheat{}", i), &mut self.thermal[i]) {
                            update_flag.set(UpdateFlag::UPDATE_DEVICE_INFO, true);
                            if self.thermal[i] {
                                cpu.fpga_mut().assert_thermal_sensor();
                            } else {
                                cpu.fpga_mut().deassert_thermal_sensor();
                            }
                        }
                    });
                    ui.separator();

                    if ui.checkbox(format!("View as device"), &mut settings.view_device) {}
                    if settings.view_device {
                        ui.text("Light position");
                        unsafe {
                            igDragFloat(
                                CString::new("X##Light").unwrap().as_c_str().as_ptr(),
                                &mut settings.light_pos_x as _,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                                CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                                0,
                            );
                            igDragFloat(
                                CString::new("Y##Light").unwrap().as_c_str().as_ptr(),
                                &mut settings.light_pos_y as _,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                                CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                                0,
                            );
                            igDragFloat(
                                CString::new("Z##Light").unwrap().as_c_str().as_ptr(),
                                &mut settings.light_pos_z as _,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                                CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                                0,
                            );
                        }
                        ui.separator();

                        ui.text("Light properties");
                        unsafe {
                            igDragFloat(
                                CString::new("Power").unwrap().as_c_str().as_ptr(),
                                &mut settings.light_power as _,
                                0.1,
                                0.,
                                std::f32::MAX / 2.,
                                CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                                0,
                            );
                            igDragFloat(
                                CString::new("Ambient").unwrap().as_c_str().as_ptr(),
                                &mut settings.ambient as _,
                                0.1,
                                0.,
                                std::f32::MAX / 2.,
                                CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                                0,
                            );
                            igDragFloat(
                                CString::new("Specular").unwrap().as_c_str().as_ptr(),
                                &mut settings.specular as _,
                                0.1,
                                0.,
                                std::f32::MAX / 2.,
                                CString::new("%.3f").unwrap().as_c_str().as_ptr(),
                                0,
                            );
                        }
                    }
                    ui.separator();

                    ui.color_picker4_config("Background", &mut settings.background)
                        .flags(ColorEditFlags::PICKER_HUE_WHEEL)
                        .build();

                    tab.end();
                }

                if let Some(tab) = ui.tab_item("Info") {
                    ui.text(format!("FPS: {:4.2}", fps));
                    ui.separator();

                    cpus.iter().for_each(|cpu| {
                        if ui.collapsing_header(
                            format!("Device {}", cpu.idx()),
                            TreeNodeFlags::DEFAULT_OPEN,
                        ) {
                            ui.text("Silencer");
                            if cpu.fpga().silencer_fixed_completion_steps_mode() {
                                ui.text(format!(
                                    "Completion steps intensity: {}",
                                    cpu.fpga().silencer_completion_steps_intensity()
                                ));
                                ui.text(format!(
                                    "Completion steps phase: {}",
                                    cpu.fpga().silencer_completion_steps_phase()
                                ));
                            } else {
                                ui.text(format!(
                                    "Update rate intensity: {}",
                                    cpu.fpga().silencer_update_rate_intensity()
                                ));
                                ui.text(format!(
                                    "Update rate phase: {}",
                                    cpu.fpga().silencer_update_rate_phase()
                                ));
                            }

                            {
                                ui.separator();
                                let m = cpu.fpga().modulation();
                                ui.text("Modulation");

                                let mod_size = m.len();
                                ui.text(format!("Size: {}", mod_size));
                                ui.text(format!(
                                    "Frequency division: {}",
                                    cpu.fpga().modulation_frequency_division()
                                ));
                                let sampling_freq = FPGA_CLK_FREQ as f32
                                    / cpu.fpga().modulation_frequency_division() as f32;
                                ui.text(format!("Sampling Frequency: {:.3} [Hz]", sampling_freq));
                                let sampling_period = 1000000.0
                                    * cpu.fpga().modulation_frequency_division() as f32
                                    / FPGA_CLK_FREQ as f32;
                                ui.text(format!("Sampling period: {:.3} [us]", sampling_period));
                                let period = sampling_period * mod_size as f32;
                                ui.text(format!("Period: {:.3} [us]", period));

                                ui.text(format!(
                                    "Current Index: {}",
                                    cpu.fpga().mod_idx_from_systime(system_time)
                                ));

                                if !m.is_empty() {
                                    ui.text(format!("mod[0]: {}", m[0]));
                                }
                                if mod_size == 2 || mod_size == 3 {
                                    ui.text(format!("mod[1]: {}", m[1]));
                                } else if mod_size > 3 {
                                    ui.text("...");
                                }
                                if mod_size >= 3 {
                                    ui.text(format!("mod[{}]: {}", mod_size - 1, m[mod_size - 1]));
                                }

                                if ui.radio_button_bool(
                                    format!("Show mod plot##{}", cpu.idx()),
                                    self.show_mod_plot[cpu.idx()],
                                ) {
                                    self.show_mod_plot[cpu.idx()] = !self.show_mod_plot[cpu.idx()];
                                }
                                if self.show_mod_plot[cpu.idx()] {
                                    let mod_v: Vec<f32> =
                                        m.iter().map(|&v| v as f32 / 255.0).collect();
                                    ui.plot_lines(format!("##mod plot{}", cpu.idx()), &mod_v)
                                        .graph_size(self.mod_plot_size[cpu.idx()])
                                        .scale_min(0.)
                                        .scale_max(1.)
                                        .build();
                                }

                                if self.show_mod_plot[cpu.idx()] {
                                    unsafe {
                                        igDragFloat2(
                                            CString::new(format!("plot size##{}", cpu.idx()))
                                                .unwrap()
                                                .as_c_str()
                                                .as_ptr(),
                                            self.mod_plot_size[cpu.idx()].as_mut_ptr(),
                                            1.,
                                            0.,
                                            std::f32::MAX / 2.,
                                            CString::new("%.0f").unwrap().as_c_str().as_ptr(),
                                            0,
                                        );
                                    }
                                }
                            }

                            if cpu.fpga().is_stm_mode() {
                                ui.separator();

                                if cpu.fpga().is_stm_gain_mode() {
                                    ui.text("Gain STM");
                                } else {
                                    ui.text("Focus STM");
                                    #[cfg(feature = "use_meter")]
                                    ui.text(format!(
                                        "Sound speed: {:.3} [m/s]",
                                        cpu.fpga().sound_speed() as f32 / 1024.0
                                    ));
                                    #[cfg(not(feature = "use_meter"))]
                                    ui.text(format!(
                                        "Sound speed: {:.3} [mm/s]",
                                        cpu.fpga().sound_speed() as f32 * 1000. / 1024.0
                                    ));
                                }

                                if let Some(start_idx) = cpu.fpga().stm_start_idx() {
                                    ui.text(format!("Start idx: {}", start_idx));
                                }
                                if let Some(finish_idx) = cpu.fpga().stm_finish_idx() {
                                    ui.text(format!("Finish idx: {}", finish_idx));
                                }

                                let stm_size = cpu.fpga().stm_cycle();
                                ui.text(format!("Size: {}", stm_size));
                                ui.text(format!(
                                    "Frequency division: {}",
                                    cpu.fpga().stm_frequency_division()
                                ));
                                let sampling_freq = FPGA_CLK_FREQ as f32
                                    / cpu.fpga().stm_frequency_division() as f32;
                                ui.text(format!("Sampling Frequency: {:.3} [Hz]", sampling_freq));
                                let sampling_period = 1000000.0
                                    * cpu.fpga().stm_frequency_division() as f32
                                    / FPGA_CLK_FREQ as f32;
                                ui.text(format!("Sampling period: {:.3} [us]", sampling_period));
                                let period = sampling_period / stm_size as f32;
                                ui.text(format!("Period: {:.3} [us]", period));

                                ui.text(format!(
                                    "Current Index: {}",
                                    cpu.fpga().stm_idx_from_systime(system_time)
                                ));
                            }
                        }
                    });

                    ui.separator();

                    if ui.checkbox("Mod enable", &mut settings.mod_enable) {
                        update_flag.set(UpdateFlag::UPDATE_SOURCE_DRIVE, true);
                    }

                    ui.checkbox("Auto play", &mut settings.auto_play);

                    ui.text(format!("System time: {} [ns]", self.real_time));
                    if settings.auto_play {
                        unsafe {
                            igDragFloat(
                                CString::new("Time scale").unwrap().as_c_str().as_ptr(),
                                &mut settings.time_scale as _,
                                0.1,
                                0.,
                                std::f32::MAX,
                                std::ptr::null(),
                                0,
                            );
                        }
                    } else {
                        ui.same_line();
                        if ui.button("+") {
                            self.real_time =
                                self.real_time.wrapping_add_signed(self.time_step as _);
                            update_flag.set(UpdateFlag::UPDATE_SOURCE_DRIVE, true);
                        }
                        ui.input_int("time step", &mut self.time_step)
                            .always_insert_mode(true)
                            .build();
                    }

                    tab.end();
                }

                tab_bar.end();
            }

            ui.separator();
            ui.text("Save image as file");
            ui.input_text("path to image", &mut settings.image_save_path)
                .build();
            if ui.small_button("save") {
                update_flag.set(UpdateFlag::SAVE_IMAGE, true);
            }

            ui.separator();

            if ui.small_button("Auto") {
                let rot = settings.slice_rotation();
                let sr = Matrix4::from(rot);
                let srf = sr * Vector4::new(0.0, 0.0, 1.0 * ZPARITY, 1.0);

                let camera_pos = settings.slice_pos() + srf.truncate() * 600.0 * MILLIMETER;
                settings.set_camera_pos(camera_pos);
                settings.set_camera_rot(settings.slice_rotation());
            }
            ui.same_line();
            if ui.small_button("Reset") {
                update_flag.set(UpdateFlag::all(), true);
                update_flag.remove(UpdateFlag::SAVE_IMAGE);
                *settings = self.initial_settings.clone();
            }
            ui.same_line();
            if ui.small_button("Default") {
                update_flag.set(UpdateFlag::all(), true);
                update_flag.remove(UpdateFlag::SAVE_IMAGE);
                *settings = Default::default();
            }
        });

        if settings.auto_play {
            update_flag.set(UpdateFlag::UPDATE_SOURCE_DRIVE, true);
            self.real_time = (FPGAEmulator::ec_time_now() as f64 * settings.time_scale as f64) as _;
        }

        self.font_size = font_size;
        self.do_update_font = update_font;

        self.platform.prepare_render(ui, render.window());
        let draw_data = self.imgui.render();
        self.imgui_renderer.draw_commands(
            builder,
            render.queue(),
            ImageView::new_default(render.image())?,
            draw_data,
        )?;

        Ok(update_flag)
    }

    pub(crate) fn waiting(
        &mut self,
        render: &Renderer,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) -> anyhow::Result<()> {
        self.update_font(render)?;

        let io = self.imgui.io_mut();

        self.platform.prepare_frame(io, render.window())?;

        let ui = self.imgui.new_frame();

        ui.window("Dear ImGui").build(|| {
            ui.text("Waiting for client connection...");
        });

        self.platform.prepare_render(ui, render.window());
        let draw_data = self.imgui.render();
        self.imgui_renderer.draw_commands(
            builder,
            render.queue(),
            ImageView::new_default(render.image())?,
            draw_data,
        )?;

        Ok(())
    }

    fn update_font(&mut self, render: &Renderer) -> anyhow::Result<()> {
        if self.do_update_font {
            self.imgui.fonts().clear();
            self.imgui.fonts().add_font(&[FontSource::TtfData {
                data: include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
                size_pixels: self.font_size * self.hidpi_factor,
                config: Some(FontConfig {
                    rasterizer_multiply: 1.,
                    glyph_ranges: FontGlyphRanges::default(),
                    ..FontConfig::default()
                }),
            }]);
            self.imgui_renderer.reload_font_texture(
                &mut self.imgui,
                render.device(),
                render.queue(),
            )?;
            self.do_update_font = false;
        }
        Ok(())
    }

    pub(crate) fn visible(&self) -> &[bool] {
        &self.visible
    }

    pub(crate) fn system_time(&self) -> u64 {
        FPGAEmulator::systime(self.real_time)
    }
}
