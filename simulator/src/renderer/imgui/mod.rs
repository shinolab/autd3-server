mod components;

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Instant};

use autd3_driver::{
    defined::{mm, ULTRASOUND_FREQ, ULTRASOUND_PERIOD, ULTRASOUND_PERIOD_COUNT},
    derive::Segment,
    ethercat::DcSysTime,
};
use autd3_firmware_emulator::CPUEmulator;
use components::*;

use glam::EulerRot;
use imgui::Context as ImGuiContext;
use imgui::{
    ColorEditFlags, FontConfig, FontGlyphRanges, FontSource, TextureId, TreeNodeFlags, Ui,
};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use strum::IntoEnumIterator;
use wgpu::{Extent3d, RenderPass};
use winit::{event::Event, window::Window};

use crate::{
    common::{color_map::ColorMap, transform::quaternion_to},
    context::Context,
    imgui_wgpu,
    state::State,
    update_flag::UpdateFlag,
    SimulatorError, Vector3, ZPARITY,
};

pub struct ImGuiRenderer {
    imgui: ImGuiContext,
    platform: WinitPlatform,
    imgui_renderer: crate::imgui_wgpu::Renderer,
    last_frame: Instant,
    font_size: f32,
    hidpi_factor: f32,
    do_update_font: bool,
    visible: Vec<bool>,
    enable: Vec<bool>,
    thermal: Vec<bool>,
    show_mod_plot: Vec<bool>,
    mod_plot_size: Vec<[f32; 2]>,
    time_step: i32,
    color_map_texture_ids: HashMap<ColorMap, TextureId>,
}

impl ImGuiRenderer {
    pub fn new(
        state: &State,
        context: &Context,
        window: Arc<Window>,
    ) -> Result<Self, SimulatorError> {
        let mut imgui = ImGuiContext::create();

        let config_path = PathBuf::new().join(&state.settings_dir);
        if !config_path.exists() {
            std::fs::create_dir_all(&config_path)?;
        }
        imgui.set_ini_filename(config_path.join("imgui.ini"));

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

        imgui.io_mut().font_global_scale = (1.0 / platform.hidpi_factor()) as f32;

        let renderer_config = crate::imgui_wgpu::RendererConfig {
            texture_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            ..Default::default()
        };
        let mut imgui_renderer = crate::imgui_wgpu::Renderer::new(
            &mut imgui,
            context.device(),
            context.queue(),
            renderer_config,
        );

        Ok(Self {
            imgui,
            hidpi_factor: platform.hidpi_factor() as _,
            platform,
            color_map_texture_ids: Self::create_color_map_texture(&mut imgui_renderer, context),
            imgui_renderer,
            last_frame: Instant::now(),
            font_size: 16.,
            do_update_font: true,
            visible: Vec::new(),
            enable: Vec::new(),
            thermal: Vec::new(),
            time_step: 1000000,
            show_mod_plot: Vec::new(),
            mod_plot_size: Vec::new(),
        })
    }

    pub fn init(&mut self, dev_num: usize) {
        self.visible = vec![true; dev_num];
        self.enable = vec![true; dev_num];
        self.thermal = vec![false; dev_num];
        self.show_mod_plot = vec![false; dev_num];
        self.mod_plot_size = vec![[200., 50.]; dev_num];
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

    fn update_camera(ui: &mut Ui, state: &mut State, update_flag: &mut UpdateFlag) {
        let rotation = state.camera.rotation();

        let r = rotation * Vector3::X;
        let u = rotation * Vector3::Y;
        let f = rotation * Vector3::Z;

        if !ui.io().want_capture_mouse {
            let mouse_wheel = ui.io().mouse_wheel;
            let trans = -f * mouse_wheel * state.camera.move_speed * ZPARITY;
            state.camera.pos += trans;
            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
        }

        if !ui.io().want_capture_mouse {
            let mouse_delta = ui.io().mouse_delta;
            if ui.io().mouse_down[0] {
                if ui.io().key_shift {
                    let delta_x = mouse_delta[0] * state.camera.move_speed / 3000.;
                    let delta_y = mouse_delta[1] * state.camera.move_speed / 3000.;
                    let to = -r * delta_x + u * delta_y + f;
                    let (rx, ry, rz) = (quaternion_to(f, to) * rotation).to_euler(EulerRot::XYZ);
                    state.camera.rot.x = rx.to_degrees();
                    state.camera.rot.y = ry.to_degrees();
                    state.camera.rot.z = rz.to_degrees();
                    update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                } else {
                    let delta_x = mouse_delta[0] * state.camera.move_speed / 10.;
                    let delta_y = mouse_delta[1] * state.camera.move_speed / 10.;
                    let trans = -r * delta_x + u * delta_y;
                    state.camera.pos.x += trans.x;
                    state.camera.pos.y += trans.y;
                    state.camera.pos.z += trans.z;
                    update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                }
            }
        }
    }

    pub fn waiting(&mut self, context: &Context, window: &Window) -> Result<(), SimulatorError> {
        self.update_font(context);

        self.platform.prepare_frame(self.imgui.io_mut(), window)?;

        let ui = self.imgui.new_frame();
        ui.window("Dear ImGui").build(|| {
            ui.text("Waiting for client connection...");
        });

        self.platform.prepare_render(ui, window);

        Ok(())
    }

    pub fn update(
        &mut self,
        state: &mut State,
        context: &Context,
        window: &Window,
        update_flag: &mut UpdateFlag,
    ) -> Result<(), SimulatorError> {
        self.update_font(context);

        self.platform.prepare_frame(self.imgui.io_mut(), window)?;

        let io = self.imgui.io_mut();
        let fps = io.framerate;

        let ui = self.imgui.new_frame();
        Self::update_camera(ui, state, update_flag);

        let mut font_size = self.font_size;
        let mut update_font = false;
        ui.window("Dear ImGui").build(|| {
            if let Some(tab_bar) = ui.tab_bar("Settings") {
                let mut update_slice_tab = |update_flag: &mut UpdateFlag| {
                    if let Some(tab) = ui.tab_item("Slice") {
                        ui.text("Position");
                        if drag_float(
                            "X##Slice",
                            &mut state.slice.pos.x,
                            1. * mm,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        if drag_float(
                            "Y##Slice",
                            &mut state.slice.pos.y,
                            1. * mm,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        if drag_float(
                            "Z##Slice",
                            &mut state.slice.pos.z,
                            1. * mm,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        ui.separator();

                        ui.text("Rotation");
                        if drag_float("RX##Slice", &mut state.slice.rot.x, 1., -180., 180.) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        if drag_float("RY##Slice", &mut state.slice.rot.y, 1., -180., 180.) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        if drag_float("RZ##Slice", &mut state.slice.rot.z, 1., -180., 180.) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        ui.separator();

                        ui.text("Size");
                        if drag_float(
                            "Width##Slice",
                            &mut state.slice.size[0],
                            1. * mm,
                            1. * mm,
                            1024. * mm,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                        }
                        if drag_float(
                            "Height##Slice",
                            &mut state.slice.size[1],
                            1. * mm,
                            1. * mm,
                            1024. * mm,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                        }
                        ui.separator();

                        ui.text("Color state");
                        let items = [
                            "Bluered", "Breeze", "Circle", "Earth", "Hell", "Inferno", "Magma",
                            "Mist", "Plasma", "Turbo", "Viridis",
                        ];
                        let selected_idx = match state.slice.color_map {
                            ColorMap::Bluered => 0,
                            ColorMap::Breeze => 1,
                            ColorMap::Circle => 2,
                            ColorMap::Earth => 3,
                            ColorMap::Hell => 4,
                            ColorMap::Inferno => 5,
                            ColorMap::Magma => 6,
                            ColorMap::Mist => 7,
                            ColorMap::Plasma => 8,
                            ColorMap::Turbo => 9,
                            ColorMap::Viridis => 10,
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
                                    state.slice.color_map = ColorMap::Bluered;
                                }
                                "Breeze" => {
                                    state.slice.color_map = ColorMap::Breeze;
                                }
                                "Circle" => {
                                    state.slice.color_map = ColorMap::Circle;
                                }
                                "Earth" => {
                                    state.slice.color_map = ColorMap::Earth;
                                }
                                "Hell" => {
                                    state.slice.color_map = ColorMap::Hell;
                                }
                                "Inferno" => {
                                    state.slice.color_map = ColorMap::Inferno;
                                }
                                "Magma" => {
                                    state.slice.color_map = ColorMap::Magma;
                                }
                                "Mist" => {
                                    state.slice.color_map = ColorMap::Mist;
                                }
                                "Plasma" => {
                                    state.slice.color_map = ColorMap::Plasma;
                                }
                                "Turbo" => {
                                    state.slice.color_map = ColorMap::Turbo;
                                }
                                "Viridis" => {
                                    state.slice.color_map = ColorMap::Viridis;
                                }
                                _ => {
                                    state.slice.color_map = ColorMap::Inferno;
                                }
                            }
                            update_flag.set(UpdateFlag::UPDATE_SLICE_COLOR_MAP, true);
                            cb.end();
                        }
                        let w = ui.item_rect_size()[0];
                        ui.same_line();
                        ui.text("Coloring");
                        imgui::Image::new(
                            self.color_map_texture_ids[&state.slice.color_map],
                            [w, 10.0],
                        )
                        .build(ui);
                        if drag_float(
                            "Max pressure [Pa]##Slice",
                            &mut state.slice.pressure_max,
                            1000.,
                            0.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_CONFIG, true);
                        }
                        ui.separator();

                        if ui.small_button("xy") {
                            state.slice.rot.x = 0.;
                            state.slice.rot.y = 0.;
                            state.slice.rot.z = 0.;
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        ui.same_line();
                        if ui.small_button("yz") {
                            state.slice.rot.x = 0.;
                            state.slice.rot.y = 90.;
                            state.slice.rot.z = 0.;
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }
                        ui.same_line();
                        if ui.small_button("zx") {
                            state.slice.rot.x = 90.;
                            state.slice.rot.y = 0.;
                            state.slice.rot.z = 0.;
                            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                        }

                        tab.end();
                    }
                };

                let mut update_camera_tab = |update_flag: &mut UpdateFlag| {
                    if let Some(tab) = ui.tab_item("Camera") {
                        ui.text("Position");
                        if drag_float(
                            "X##Camera",
                            &mut state.camera.pos.x,
                            1. * mm,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        if drag_float(
                            "Y##Camera",
                            &mut state.camera.pos.y,
                            1. * mm,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        if drag_float(
                            "Z##Camera",
                            &mut state.camera.pos.z,
                            1. * mm,
                            f32::MIN / 2.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        ui.separator();

                        ui.text("Rotation");
                        if drag_float("RX##Camera", &mut state.camera.rot.x, 1., -180., 180.) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        if drag_float("RY##Camera", &mut state.camera.rot.y, 1., -180., 180.) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        if drag_float("RZ##Camera", &mut state.camera.rot.z, 1., -180., 180.) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        ui.separator();

                        drag_float(
                            "Move speed",
                            &mut state.camera.move_speed,
                            1.,
                            1. * mm,
                            100. * mm,
                        );
                        ui.separator();

                        ui.text("Perspective");
                        if drag_float("FOV", &mut state.camera.fov, 1., 0., 180.) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        if drag_float(
                            "Near clip",
                            &mut state.camera.near_clip,
                            1. * mm,
                            0.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        if drag_float(
                            "Far clip",
                            &mut state.camera.far_clip,
                            1. * mm,
                            0.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                        }
                        tab.end();
                    }
                };

                let mut update_config_tab = |update_flag: &mut UpdateFlag| {
                    if let Some(tab) = ui.tab_item("Config") {
                        if drag_float(
                            "Sound speed",
                            &mut state.sound_speed,
                            1. * mm,
                            0.,
                            f32::MAX / 2.,
                        ) {
                            update_flag.set(UpdateFlag::UPDATE_CONFIG, true);
                        }
                        ui.separator();

                        if drag_float("Font size", &mut font_size, 1., 1., 256.) {
                            update_font = true;
                        }
                        ui.separator();

                        ui.text("Device index: show/enable/overheat");
                        state.cpus.iter_mut().enumerate().for_each(|(i, cpu)| {
                            ui.text(format!("Device {}: ", i));
                            ui.same_line();
                            if ui.checkbox(format!("##show{}", i), &mut self.visible[i]) {
                                update_flag.set(UpdateFlag::UPDATE_TRANS_ALPHA, true);
                                let v = if self.visible[i] { 1. } else { 0. };
                                state
                                    .transducers
                                    .device(i)
                                    .states
                                    .iter_mut()
                                    .for_each(|s| s.alpha = v);
                            }
                            ui.same_line();
                            if ui.checkbox(format!("##enable{}", i), &mut self.enable[i]) {
                                update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
                                let v = if self.enable[i] { 1. } else { 0. };
                                state
                                    .transducers
                                    .device(i)
                                    .states
                                    .iter_mut()
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

                        ui.color_picker4_config("Background", &mut state.background)
                            .flags(ColorEditFlags::PICKER_HUE_WHEEL)
                            .build();

                        tab.end();
                    }
                };

                let mut update_info_tab = |update_flag: &mut UpdateFlag, cpus: &[CPUEmulator]| {
                    if let Some(tab) = ui.tab_item("Info") {
                        ui.text(format!("FPS: {:4.2}", fps));
                        ui.separator();

                        cpus.iter().for_each(|cpu| {
                            if ui.collapsing_header(
                                format!("Device {}", cpu.idx()),
                                TreeNodeFlags::DEFAULT_OPEN,
                            ) {
                                ui.dummy([10.0, 0.0]);
                                ui.same_line();
                                let g = ui.begin_group();
                                if ui.collapsing_header(
                                    format!("Silencer##{}", cpu.idx()),
                                    TreeNodeFlags::DEFAULT_OPEN,
                                ) {
                                    if cpu.fpga().silencer_fixed_completion_steps_mode() {
                                        ui.text(format!(
                                            "Completion time intensity: {:?}",
                                            cpu.fpga().silencer_completion_steps().intensity()
                                        ));
                                        ui.text(format!(
                                            "Completion time phase: {:?}",
                                            cpu.fpga().silencer_completion_steps().phase()
                                        ));
                                    } else {
                                        ui.text(format!(
                                            "Update rate intensity: {}",
                                            cpu.fpga().silencer_update_rate().intensity()
                                        ));
                                        ui.text(format!(
                                            "Update rate phase: {}",
                                            cpu.fpga().silencer_update_rate().phase()
                                        ));
                                    }
                                }

                                if ui.collapsing_header(
                                    format!("Modulation##{}", cpu.idx()),
                                    TreeNodeFlags::DEFAULT_OPEN,
                                ) {
                                    ui.separator();

                                    let segment = cpu.fpga().current_mod_segment();

                                    let m = cpu.fpga().modulation_buffer(segment);

                                    let mod_size = m.len();
                                    ui.text(format!("Size: {}", mod_size));
                                    ui.text(format!(
                                        "Frequency division: {}",
                                        cpu.fpga().modulation_freq_division(segment)
                                    ));
                                    let sampling_freq = ULTRASOUND_FREQ.hz() as f32
                                        / cpu.fpga().modulation_freq_division(segment) as f32;
                                    ui.text(format!("Sampling Frequency: {:.3}Hz", sampling_freq));
                                    let sampling_period = ULTRASOUND_PERIOD
                                        * cpu.fpga().modulation_freq_division(segment) as u32;
                                    ui.text(format!("Sampling period: {:?}", sampling_period));
                                    let period = sampling_period * mod_size as u32;
                                    ui.text(format!("Period: {:?}", period));

                                    ui.text(format!(
                                        "Current Index: {}",
                                        cpu.fpga().current_mod_idx()
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
                                        ui.text(format!(
                                            "mod[{}]: {}",
                                            mod_size - 1,
                                            m[mod_size - 1]
                                        ));
                                    }

                                    if ui.radio_button_bool(
                                        format!("Show mod plot##{}", cpu.idx()),
                                        self.show_mod_plot[cpu.idx()],
                                    ) {
                                        self.show_mod_plot[cpu.idx()] =
                                            !self.show_mod_plot[cpu.idx()];
                                    }
                                    if self.show_mod_plot[cpu.idx()] {
                                        let mod_v: Vec<f32> =
                                            m.into_iter().map(|v| v as f32 / 255.0).collect();
                                        ui.plot_lines(format!("##mod plot{}", cpu.idx()), &mod_v)
                                            .graph_size(self.mod_plot_size[cpu.idx()])
                                            .scale_min(0.)
                                            .scale_max(1.)
                                            .build();
                                    }

                                    if self.show_mod_plot[cpu.idx()] {
                                        drag_float2(
                                            format!("plot size##{}", cpu.idx()),
                                            &mut self.mod_plot_size[cpu.idx()],
                                            1.,
                                            0.,
                                            f32::MAX / 2.,
                                        );
                                    }
                                }

                                if ui.collapsing_header(
                                    format!("STM##{}", cpu.idx()),
                                    TreeNodeFlags::DEFAULT_OPEN,
                                ) {
                                    ui.separator();

                                    let segment = cpu.fpga().current_stm_segment();

                                    let stm_cycle = cpu.fpga().stm_cycle(segment);

                                    let is_gain_mode = stm_cycle == 1;

                                    if is_gain_mode {
                                        ui.text("Gain");
                                    } else if cpu.fpga().is_stm_gain_mode(segment) {
                                        ui.text("Gain STM");
                                    } else {
                                        ui.text("Focus STM");
                                        #[cfg(feature = "use_meter")]
                                        ui.text(format!(
                                            "Sound speed: {:.3}m/s",
                                            cpu.fpga().sound_speed(segment) as f32 / 64.0
                                        ));
                                        #[cfg(not(feature = "use_meter"))]
                                        ui.text(format!(
                                            "Sound speed: {:.3}mm/s",
                                            cpu.fpga().sound_speed(segment) as f32 * 1000. / 64.0
                                        ));
                                    }

                                    ui.text(format!("Segment: {:?}", segment));

                                    if !is_gain_mode {
                                        ui.text(format!(
                                            "LoopBehavior: {:?}",
                                            cpu.fpga().stm_loop_behavior(segment)
                                        ));

                                        let stm_size = cpu.fpga().stm_cycle(segment);
                                        ui.text(format!("Size: {}", stm_size));
                                        ui.text(format!(
                                            "Frequency division: {}",
                                            cpu.fpga().stm_freq_division(segment)
                                        ));
                                        let sampling_freq = ULTRASOUND_FREQ.hz() as f32
                                            / cpu.fpga().stm_freq_division(segment) as f32;
                                        ui.text(format!(
                                            "Sampling Frequency: {:.3}Hz",
                                            sampling_freq
                                        ));
                                        let sampling_period = ULTRASOUND_PERIOD
                                            * cpu.fpga().stm_freq_division(segment) as u32;
                                        ui.text(format!("Sampling period: {:?}", sampling_period));
                                        let period = sampling_period * stm_size as u32;
                                        ui.text(format!("Period: {:?}", period));

                                        ui.text(format!(
                                            "Current Index: {}",
                                            cpu.fpga().current_stm_idx()
                                        ));
                                    }
                                }

                                if ui.collapsing_header(
                                    format!("GPIO OUT##{}", cpu.idx()),
                                    TreeNodeFlags::empty(),
                                ) {
                                    let debug_types = cpu.fpga().debug_types();
                                    let debug_values = cpu.fpga().debug_values();
                                    let gpio_out = |ty, value| match ty {
                                        autd3_firmware_emulator::fpga::params::DBG_NONE => {
                                            vec![0.0; ULTRASOUND_PERIOD_COUNT]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_BASE_SIG => [
                                            vec![0.0; ULTRASOUND_PERIOD_COUNT / 2],
                                            vec![1.0; ULTRASOUND_PERIOD_COUNT / 2],
                                        ]
                                        .concat(),
                                        autd3_firmware_emulator::fpga::params::DBG_THERMO => {
                                            vec![
                                                if cpu.fpga().is_thermo_asserted() {
                                                    1.0
                                                } else {
                                                    0.0
                                                };
                                                ULTRASOUND_PERIOD_COUNT
                                            ]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_FORCE_FAN => {
                                            vec![
                                                if cpu.fpga().is_force_fan() { 1.0 } else { 0.0 };
                                                ULTRASOUND_PERIOD_COUNT
                                            ]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_SYNC => {
                                            vec![0.0; ULTRASOUND_PERIOD_COUNT]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_MOD_SEGMENT => {
                                            vec![
                                                match cpu.fpga().current_mod_segment() {
                                                    Segment::S0 => 0.0,
                                                    Segment::S1 => 1.0,
                                                    _ => unimplemented!(),
                                                };
                                                ULTRASOUND_PERIOD_COUNT
                                            ]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_MOD_IDX => {
                                            vec![
                                                if cpu.fpga().current_mod_idx() == 0 {
                                                    1.0
                                                } else {
                                                    0.0
                                                };
                                                ULTRASOUND_PERIOD_COUNT
                                            ]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_STM_SEGMENT => {
                                            vec![
                                                match cpu.fpga().current_stm_segment() {
                                                    Segment::S0 => 0.0,
                                                    Segment::S1 => 1.0,
                                                    _ => unimplemented!(),
                                                };
                                                ULTRASOUND_PERIOD_COUNT
                                            ]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_STM_IDX => {
                                            vec![
                                                if cpu.fpga().current_mod_idx() == 0 {
                                                    1.0
                                                } else {
                                                    0.0
                                                };
                                                ULTRASOUND_PERIOD_COUNT
                                            ]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_IS_STM_MODE => {
                                            vec![
                                                if cpu
                                                    .fpga()
                                                    .stm_cycle(cpu.fpga().current_stm_segment())
                                                    != 1
                                                {
                                                    1.0
                                                } else {
                                                    0.0
                                                };
                                                ULTRASOUND_PERIOD_COUNT
                                            ]
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_PWM_OUT => {
                                            let d = cpu.fpga().drives_at(
                                                cpu.fpga().current_stm_segment(),
                                                cpu.fpga().current_stm_idx(),
                                            )[value as usize];
                                            let m = cpu.fpga().modulation_at(
                                                cpu.fpga().current_mod_segment(),
                                                cpu.fpga().current_mod_idx(),
                                            );
                                            let phase = d.phase().value() as u32;
                                            let pulse_width =
                                                cpu.fpga().to_pulse_width(d.intensity(), m) as u32;
                                            const T: u32 = ULTRASOUND_PERIOD_COUNT as u32;
                                            let rise = (phase - pulse_width / 2 + T) % T;
                                            let fall = (phase + (pulse_width + 1) / 2 + T) % T;
                                            #[allow(clippy::collapsible_else_if)]
                                            (0..T)
                                                .map(|t| {
                                                    if rise <= fall {
                                                        if (rise <= t) && (t < fall) {
                                                            1.0
                                                        } else {
                                                            0.0
                                                        }
                                                    } else {
                                                        if (t < fall) || (rise <= t) {
                                                            1.0
                                                        } else {
                                                            0.0
                                                        }
                                                    }
                                                })
                                                .collect()
                                        }
                                        autd3_firmware_emulator::fpga::params::DBG_DIRECT => {
                                            vec![value as f32; ULTRASOUND_PERIOD_COUNT]
                                        }
                                        _ => unreachable!(),
                                    };

                                    (0..4).for_each(|i| {
                                        let gpio_out = gpio_out(debug_types[i], debug_values[i]);
                                        ui.plot_lines(
                                            format!("GPIO_OUT[{}]##{}", i, cpu.idx()),
                                            &gpio_out,
                                        )
                                        .graph_size(self.mod_plot_size[cpu.idx()])
                                        .scale_min(0.)
                                        .scale_max(1.)
                                        .build();
                                    });
                                }

                                g.end();
                            }
                        });

                        ui.separator();

                        if ui.checkbox("Mod enable", &mut state.mod_enable) {
                            update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
                        }

                        ui.checkbox("Auto play", &mut state.auto_play);

                        ui.text(format!("System time: {}ns", state.real_time));
                        if state.auto_play {
                            drag_float(
                                "Time scale",
                                &mut state.time_scale,
                                0.1,
                                0.,
                                f32::MAX / 2.0,
                            );
                        } else {
                            ui.same_line();
                            if ui.button("+") {
                                state.real_time =
                                    state.real_time.wrapping_add_signed(self.time_step as _);
                                update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
                            }
                            ui.input_int("time step", &mut self.time_step)
                                .always_insert_mode(true)
                                .build();
                        }

                        tab.end();
                    }
                };

                update_slice_tab(update_flag);
                update_camera_tab(update_flag);
                update_config_tab(update_flag);
                update_info_tab(update_flag, &state.cpus);

                tab_bar.end();
            }
        });

        if state.auto_play {
            update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
            state.real_time = (DcSysTime::now().sys_time() as f64 * state.time_scale as f64) as _;
        }

        self.font_size = font_size;
        self.do_update_font = update_font;

        self.platform.prepare_render(ui, window);

        Ok(())
    }

    pub fn render<'a>(
        &'a mut self,
        context: &Context,
        pass: &'a mut RenderPass<'a>,
    ) -> Result<(), SimulatorError> {
        let draw_data = self.imgui.render();
        self.imgui_renderer
            .render(draw_data, context.queue(), context.device(), pass)?;
        Ok(())
    }

    fn update_font(&mut self, context: &Context) {
        if self.do_update_font {
            self.imgui.fonts().clear();
            self.imgui.fonts().add_font(&[FontSource::TtfData {
                data: include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf"),
                size_pixels: self.font_size * self.hidpi_factor,
                config: Some(FontConfig {
                    rasterizer_multiply: 1.,
                    glyph_ranges: FontGlyphRanges::default(),
                    ..FontConfig::default()
                }),
            }]);
            self.imgui_renderer.reload_font_texture(
                &mut self.imgui,
                context.device(),
                context.queue(),
            );
            self.do_update_font = false;
        }
    }

    fn create_color_map_texture(
        imgui_renderer: &mut imgui_wgpu::Renderer,
        context: &Context,
    ) -> HashMap<ColorMap, TextureId> {
        const COLOR_MAP_SIZE: u32 = 100;
        ColorMap::iter()
            .map(|color| -> (ColorMap, TextureId) {
                let texture_extent = Extent3d {
                    width: COLOR_MAP_SIZE,
                    height: 1,
                    depth_or_array_layers: 1,
                };

                let iter = (0..COLOR_MAP_SIZE).map(|x| x as f64 / COLOR_MAP_SIZE as f64);
                let color_map = color.color_map(iter);
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

                let texture = imgui_wgpu::Texture::new(
                    context.device(),
                    imgui_renderer,
                    imgui_wgpu::TextureConfig {
                        size: texture_extent,
                        label: None,
                        format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        ..Default::default()
                    },
                );
                texture.write(
                    context.queue(),
                    bytemuck::cast_slice(&texels),
                    COLOR_MAP_SIZE,
                    1,
                );
                let id = imgui_renderer.textures.insert(texture);
                (color, id)
            })
            .collect()
    }
}
