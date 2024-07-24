mod components;

use autd3::{derive::AUTDInternalError, prelude::ULTRASOUND_FREQ};
use autd3_driver::{
    defined::ULTRASOUND_PERIOD_COUNT,
    ethercat::{DcSysTime, ECAT_DC_SYS_TIME_BASE},
};
use components::*;

use crate::prelude::*;

use imgui::{
    ColorEditFlags, Context, FontConfig, FontGlyphRanges, FontSource, TextureId, TreeNodeFlags, Ui,
};

pub struct ImGuiViewer {
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

impl ImGuiViewer {
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

        imgui.io_mut().font_global_scale = (1.0 / platform.hidpi_factor()) as f32;

        let mut imgui_renderer = imgui_vulkano_renderer::Renderer::init(
            &mut imgui,
            renderer.device(),
            renderer.queue(),
            renderer.image_format(),
        )?;

        Ok(Self {
            imgui,
            hidpi_factor: platform.hidpi_factor() as _,
            platform,
            color_map_texture_ids: Self::create_color_map_texture(&mut imgui_renderer, renderer)?,
            imgui_renderer,
            last_frame: Instant::now(),
            font_size: 16.,
            do_update_font: true,
            visible: Vec::new(),
            enable: Vec::new(),
            thermal: Vec::new(),
            real_time: DcSysTime::now().sys_time(),
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

    fn update_camera(ui: &mut Ui, settings: &mut ViewerSettings) {
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

    pub fn update(
        &mut self,
        cpus: &mut [CPUEmulator],
        sources: &mut SoundSources,
        body_pointer: &[usize],
        render: &Renderer,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        settings: &mut ViewerSettings,
    ) -> anyhow::Result<UpdateFlag> {
        self.update_font(render)?;

        let io = self.imgui.io_mut();
        let fps = io.framerate;

        self.platform.prepare_frame(io, render.window())?;

        let ui = self.imgui.new_frame();

        Self::update_camera(ui, settings);

        let mut font_size = self.font_size;
        let mut update_font = false;
        let mut update_flag = ui
            .window("Dear ImGui")
            .build(|| -> UpdateFlag {
                let mut update_flag = UpdateFlag::empty();

                if let Some(tab_bar) = ui.tab_bar("Settings") {
                    let mut update_slice_tab = |mut update_flag: UpdateFlag| -> UpdateFlag {
                        if let Some(tab) = ui.tab_item("Slice") {
                            ui.text("Position");
                            if drag_float(
                                "X##Slice",
                                &mut settings.slice_pos_x,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                            ) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                            }
                            if drag_float(
                                "Y##Slice",
                                &mut settings.slice_pos_y,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                            ) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                            }
                            if drag_float(
                                "Z##Slice",
                                &mut settings.slice_pos_z,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                            ) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                            }
                            ui.separator();

                            ui.text("Rotation");
                            if drag_float("RX##Slice", &mut settings.slice_rot_x, 1., -180., 180.) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                            }
                            if drag_float("RY##Slice", &mut settings.slice_rot_y, 1., -180., 180.) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                            }
                            if drag_float("RZ##Slice", &mut settings.slice_rot_z, 1., -180., 180.) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
                            }
                            ui.separator();

                            ui.text("Size");
                            if drag_float(
                                "Width##Slice",
                                &mut settings.slice_width,
                                1. * MILLIMETER,
                                1. * MILLIMETER,
                                2000. * MILLIMETER,
                            ) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                            }
                            if drag_float(
                                "Height##Slice",
                                &mut settings.slice_height,
                                1. * MILLIMETER,
                                1. * MILLIMETER,
                                2000. * MILLIMETER,
                            ) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                            }
                            if drag_float(
                                "Pixel size##Slice",
                                &mut settings.slice_pixel_size,
                                1. * MILLIMETER,
                                0.1 * MILLIMETER,
                                8. * MILLIMETER,
                            ) {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
                            }
                            ui.separator();

                            ui.text("Color settings");
                            let items = [
                                "Bluered", "Breeze", "Circle", "Earth", "Hell", "Inferno", "Magma",
                                "Mist", "Plasma", "Turbo", "Viridis",
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
                            drag_float(
                                "Max pressure [Pa]##Slice",
                                &mut settings.pressure_max,
                                1000.,
                                0.,
                                f32::MAX / 2.,
                            );
                            ui.checkbox("Show##Slice", &mut settings.slice_show);
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
                        update_flag
                    };

                    let mut update_camera_tab = || {
                        if let Some(tab) = ui.tab_item("Camera") {
                            ui.text("Position");
                            drag_float(
                                "X##Camera",
                                &mut settings.camera_pos_x,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                            );
                            drag_float(
                                "Y##Camera",
                                &mut settings.camera_pos_y,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                            );
                            drag_float(
                                "Z##Camera",
                                &mut settings.camera_pos_z,
                                1. * MILLIMETER,
                                f32::MIN / 2.,
                                f32::MAX / 2.,
                            );
                            ui.separator();

                            ui.text("Rotation");
                            drag_float("RX##Camera", &mut settings.camera_rot_x, 1., -180., 180.);
                            drag_float("RY##Camera", &mut settings.camera_rot_y, 1., -180., 180.);
                            drag_float("RZ##Camera", &mut settings.camera_rot_z, 1., -180., 180.);
                            ui.separator();

                            drag_float(
                                "Move speed",
                                &mut settings.camera_move_speed,
                                1.,
                                1. * MILLIMETER,
                                100. * MILLIMETER,
                            );
                            ui.separator();

                            ui.text("Perspective");
                            drag_float("FOV", &mut settings.camera_fov, 1., 0., 180.);
                            drag_float(
                                "Near clip",
                                &mut settings.camera_near_clip,
                                1. * MILLIMETER,
                                0.,
                                std::f32::MAX / 2.,
                            );
                            drag_float(
                                "Far clip",
                                &mut settings.camera_far_clip,
                                1. * MILLIMETER,
                                0.,
                                std::f32::MAX / 2.,
                            );
                            tab.end();
                        }
                    };

                    let mut update_config_tab = |mut update_flag: UpdateFlag| -> UpdateFlag {
                        if let Some(tab) = ui.tab_item("Config") {
                            if drag_float(
                                "Sound speed",
                                &mut settings.sound_speed,
                                1. * MILLIMETER,
                                0.,
                                std::f32::MAX / 2.,
                            ) {
                                cpus.iter().for_each(|cpu| {
                                    sources.drives_mut().skip(body_pointer[cpu.idx()]).take(cpu.num_transducers()).for_each(
                                        |s| {
                                            s.set_wave_number(
                                                settings.sound_speed,
                                            );
                                        }
                                    );
                                });
                                update_flag.set(UpdateFlag::UPDATE_SOURCE_DRIVE, true);
                            }
                            ui.separator();

                            if drag_float("Font size", &mut font_size, 1., 1., 256.) {
                                update_font = true;
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
                                        .take(cpu.num_transducers())
                                        .for_each(|s| *s = v);
                                }
                                ui.same_line();
                                if ui.checkbox(format!("##enable{}", i), &mut self.enable[i]) {
                                    update_flag.set(UpdateFlag::UPDATE_SOURCE_FLAG, true);
                                    let v = if self.enable[i] { 1. } else { 0. };
                                    sources
                                        .drives_mut()
                                        .skip(body_pointer[i])
                                        .take(cpu.num_transducers())
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

                            ui.checkbox("View as device", &mut settings.view_device);
                            ui.separator();

                            ui.color_picker4_config("Background", &mut settings.background)
                                .flags(ColorEditFlags::PICKER_HUE_WHEEL)
                                .build();

                            tab.end();
                        }
                        update_flag
                    };

                    let mut update_info_tab = |mut update_flag: UpdateFlag,
                                               cpus: &[CPUEmulator]|
                     -> UpdateFlag {
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
                                    }

                                    if ui.collapsing_header(
                                        format!("Modulation##{}", cpu.idx()),
                                        TreeNodeFlags::DEFAULT_OPEN,
                                    ) {
                                        ui.separator();

                                        let segment = cpu.fpga().current_mod_segment();

                                        let m = cpu.fpga().modulation(segment);

                                        let mod_size = m.len();
                                        ui.text(format!("Size: {}", mod_size));
                                        ui.text(format!(
                                            "Frequency division: {}",
                                            cpu.fpga().modulation_freq_division(segment)
                                        ));
                                        let sampling_freq = ULTRASOUND_FREQ.hz() as f32
                                            / cpu.fpga().modulation_freq_division(segment) as f32;
                                        ui.text(format!(
                                            "Sampling Frequency: {:.3} [Hz]",
                                            sampling_freq
                                        ));
                                        let sampling_period = 1000000.0
                                            * cpu.fpga().modulation_freq_division(segment) as f32
                                            /ULTRASOUND_FREQ.hz() as f32;
                                        ui.text(format!(
                                            "Sampling period: {:.3} [us]",
                                            sampling_period
                                        ));
                                        let period = sampling_period * mod_size as f32;
                                        ui.text(format!("Period: {:.3} [us]", period));

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
                                            ui.plot_lines(
                                                format!("##mod plot{}", cpu.idx()),
                                                &mod_v,
                                            )
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
                                                std::f32::MAX / 2.,
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
                                                "Sound speed: {:.3} [m/s]",
                                                cpu.fpga().sound_speed(segment) as f32 / 1024.0
                                            ));
                                            #[cfg(not(feature = "use_meter"))]
                                            ui.text(format!(
                                                "Sound speed: {:.3} [mm/s]",
                                                cpu.fpga().sound_speed(segment) as f32 * 1000.
                                                    / 1024.0
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
                                            let sampling_freq =ULTRASOUND_FREQ.hz()
                                                as f32
                                                / cpu.fpga().stm_freq_division(segment) as f32;
                                            ui.text(format!(
                                                "Sampling Frequency: {:.3} [Hz]",
                                                sampling_freq
                                            ));
                                            let sampling_period = 1000000.0
                                                * cpu.fpga().stm_freq_division(segment) as f32
                                                / ULTRASOUND_FREQ.hz() as f32;
                                            ui.text(format!(
                                                "Sampling period: {:.3} [us]",
                                                sampling_period
                                            ));
                                            let period = sampling_period / stm_size as f32;
                                            ui.text(format!("Period: {:.3} [us]", period));

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
                                            autd3_firmware_emulator::fpga::params::DBG_BASE_SIG => {
                                                [
                                                    vec![0.0; ULTRASOUND_PERIOD_COUNT / 2],
                                                    vec![1.0; ULTRASOUND_PERIOD_COUNT / 2],
                                                ]
                                                .concat()
                                            }
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
                                                    if cpu.fpga().is_force_fan() {
                                                        1.0
                                                    } else {
                                                        0.0
                                                    };
                                                    ULTRASOUND_PERIOD_COUNT
                                                ]
                                            }
                                            autd3_firmware_emulator::fpga::params::DBG_SYNC => {
                                                vec![0.0; ULTRASOUND_PERIOD_COUNT]
                                            }
                                            autd3_firmware_emulator::fpga::params::DBG_MOD_SEGMENT => {
                                                vec![match cpu.fpga().current_mod_segment() {
                                                    autd3::derive::Segment::S0 => 0.0,
                                                    autd3::derive::Segment::S1 => 1.0,
                                                    _ => unimplemented!(),
                                                }; ULTRASOUND_PERIOD_COUNT]

                                            }
                                            autd3_firmware_emulator::fpga::params::DBG_MOD_IDX => {
                                                vec![if cpu.fpga().current_mod_idx() == 0 {
                                                    1.0
                                                } else {
                                                    0.0
                                                }; ULTRASOUND_PERIOD_COUNT]
                                            }
                                            autd3_firmware_emulator::fpga::params::DBG_STM_SEGMENT => {
                                                vec![match cpu.fpga().current_stm_segment() {
                                                    autd3::derive::Segment::S0 => 0.0,
                                                    autd3::derive::Segment::S1 => 1.0,
                                                    _ => unimplemented!(),
                                                }; ULTRASOUND_PERIOD_COUNT]

                                            }
                                            autd3_firmware_emulator::fpga::params::DBG_STM_IDX => {
                                                vec![if cpu.fpga().current_mod_idx() == 0 {
                                                    1.0
                                                } else {
                                                    0.0
                                                }; ULTRASOUND_PERIOD_COUNT]
                                            }
                                            autd3_firmware_emulator::fpga::params::DBG_IS_STM_MODE => {
                                                vec![if cpu.fpga().stm_cycle(cpu.fpga().current_stm_segment()) != 1 {
                                                    1.0
                                                } else {
                                                    0.0
                                                }; ULTRASOUND_PERIOD_COUNT]
                                            }
                                            autd3_firmware_emulator::fpga::params::DBG_PWM_OUT => {
                                                let d = cpu.fpga().drives(cpu.fpga().current_stm_segment(),  cpu.fpga().current_stm_idx())[value as usize];
                                                let m = cpu.fpga().modulation_at(cpu.fpga().current_mod_segment(), cpu.fpga().current_mod_idx());
                                                let phase = d.phase().value() as u32;
                                                let pulse_width = cpu.fpga().to_pulse_width(d.intensity(), m.into()) as u32;
                                                const T:u32 = ULTRASOUND_PERIOD_COUNT as u32;
                                                let rise = (T-phase*2-pulse_width/2+T)%T;
                                                let fall = (T-phase*2+(pulse_width+1)/2+T)%T;
                                                (0..T).map(|t|
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
                                                ).collect()
                                            }
                                            autd3_firmware_emulator::fpga::params::DBG_DIRECT => {
                                                vec![value as f32; ULTRASOUND_PERIOD_COUNT]
                                            }
                                            _ => unreachable!()
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

                            if ui.checkbox("Mod enable", &mut settings.mod_enable) {
                                update_flag.set(UpdateFlag::UPDATE_SOURCE_DRIVE, true);
                            }

                            ui.checkbox("Auto play", &mut settings.auto_play);

                            ui.text(format!("System time: {} [ns]", self.real_time));
                            if settings.auto_play {
                                drag_float(
                                    "Time scale",
                                    &mut settings.time_scale,
                                    0.1,
                                    0.,
                                    f32::MAX / 2.0,
                                );
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
                        update_flag
                    };

                    update_flag = update_slice_tab(update_flag);
                    update_camera_tab();
                    update_flag = update_config_tab(update_flag);
                    update_flag = update_info_tab(update_flag, cpus);

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

                update_flag
            })
            .unwrap_or(UpdateFlag::empty());

        if settings.auto_play {
            update_flag.set(UpdateFlag::UPDATE_SOURCE_DRIVE, true);
            self.real_time = (DcSysTime::now().sys_time() as f64 * settings.time_scale as f64) as _;
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

        self.platform
            .prepare_frame(self.imgui.io_mut(), render.window())?;

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

    pub(crate) fn system_time(&self) -> Result<DcSysTime, AUTDInternalError> {
        DcSysTime::from_utc(ECAT_DC_SYS_TIME_BASE + std::time::Duration::from_nanos(self.real_time))
    }

    fn create_color_map_texture(
        imgui_renderer: &mut imgui_vulkano_renderer::Renderer,
        renderer: &Renderer,
    ) -> anyhow::Result<HashMap<ColorMapType, TextureId>> {
        const COLOR_MAP_SIZE: u32 = 100;
        ColorMapType::iter()
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
            .collect::<anyhow::Result<HashMap<_, _>>>()
    }
}
