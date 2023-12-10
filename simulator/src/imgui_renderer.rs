/*
 * File: imgui_renderer.rs
 * Project: src
 * Created Date: 23/05/2023
 * Author: Shun Suzuki
 * -----
 * Last Modified: 06/12/2023
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2023 Shun Suzuki. All rights reserved.
 *
 */

use std::{ffi::CString, path::PathBuf, time::Instant};

use crate::patch::imgui_winit_support::{HiDpiMode, WinitPlatform};
use autd3_driver::fpga::FPGA_CLK_FREQ;
use autd3_firmware_emulator::CPUEmulator;
use cgmath::{Deg, Euler};
use chrono::{Local, TimeZone, Utc};
use imgui::{
    sys::{igDragFloat, igDragFloat2},
    Context, FontConfig, FontGlyphRanges, FontSource, TreeNodeFlags,
};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    image::view::ImageView,
};
use winit::{event::Event, window::Window};

use crate::{
    common::transform::quaternion_to,
    patch::imgui_vulkano_renderer,
    renderer::Renderer,
    sound_sources::SoundSources,
    update_flag::UpdateFlag,
    viewer_settings::{ColorMapType, ViewerSettings},
    Matrix4, Quaternion, Vector3, Vector4, MILLIMETER, ZPARITY,
};

fn get_current_ec_time() -> u64 {
    (Local::now().time() - Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap().time())
        .num_nanoseconds()
        .unwrap() as _
}

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

        let imgui_renderer = imgui_vulkano_renderer::Renderer::init(
            &mut imgui,
            renderer.device(),
            renderer.queue(),
            renderer.image_format(),
        )?;

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
            real_time: get_current_ec_time(),
            time_step: 1000000,
            show_mod_plot: Vec::new(),
            mod_plot_size: Vec::new(),
            initial_settings,
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

        let system_time = self.system_time();
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

                    if ui.radio_button_bool("Acoustic", !settings.show_radiation_pressure) {
                        settings.show_radiation_pressure = false;
                    }
                    if ui.radio_button_bool("Radiation", settings.show_radiation_pressure) {
                        settings.show_radiation_pressure = true;
                    }
                    ui.separator();

                    ui.text("Color settings");
                    let items = ["Viridis", "Magma", "Inferno", "Plasma"];
                    let selected_idx = match settings.color_map_type {
                        ColorMapType::Viridis => 0,
                        ColorMapType::Magma => 1,
                        ColorMapType::Inferno => 2,
                        ColorMapType::Plasma => 3,
                    };
                    let mut selected = &items[selected_idx];
                    if let Some(cb) = ui.begin_combo("Coloring", selected) {
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
                            "Viridis" => {
                                settings.color_map_type = ColorMapType::Viridis;
                                update_flag.set(UpdateFlag::UPDATE_COLOR_MAP, true);
                            }
                            "Magma" => {
                                settings.color_map_type = ColorMapType::Magma;
                                update_flag.set(UpdateFlag::UPDATE_COLOR_MAP, true);
                            }
                            "Inferno" => {
                                settings.color_map_type = ColorMapType::Inferno;
                                update_flag.set(UpdateFlag::UPDATE_COLOR_MAP, true);
                            }
                            "Plasma" => {
                                settings.color_map_type = ColorMapType::Magma;
                                update_flag.set(UpdateFlag::UPDATE_COLOR_MAP, true);
                            }
                            _ => {
                                settings.color_map_type = ColorMapType::Inferno;
                                update_flag.set(UpdateFlag::UPDATE_COLOR_MAP, true);
                            }
                        }
                        cb.end();
                    }
                    unsafe {
                        igDragFloat(
                            CString::new("Scale##Slice").unwrap().as_c_str().as_ptr(),
                            &mut settings.slice_color_scale as _,
                            0.1,
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

                    ui.color_picker4("Background", &mut settings.background);

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
                            ui.text(format!(
                                "Step intensity: {}",
                                cpu.fpga().silencer_step_intensity()
                            ));
                            ui.text(format!("Step phase: {}", cpu.fpga().silencer_step_phase()));

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
                                    Self::mod_idx(system_time, cpu)
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
                                    Self::stm_idx(system_time, cpu)
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
            self.real_time = (get_current_ec_time() as f64 * settings.time_scale as f64) as _;
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

    pub(crate) const fn system_time(&self) -> u64 {
        ((self.real_time as u128 * autd3_driver::fpga::FPGA_CLK_FREQ as u128) / 1000000000) as _
    }

    pub(crate) fn stm_idx(system_time: u64, cpu: &CPUEmulator) -> usize {
        (system_time / cpu.fpga().stm_frequency_division() as u64) as usize % cpu.fpga().stm_cycle()
    }

    pub(crate) fn mod_idx(system_time: u64, cpu: &CPUEmulator) -> usize {
        (system_time / cpu.fpga().modulation_frequency_division() as u64) as usize
            % cpu.fpga().modulation_cycle()
    }

    pub(crate) fn visible(&self) -> &[bool] {
        &self.visible
    }
}
