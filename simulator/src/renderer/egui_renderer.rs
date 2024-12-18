use std::sync::Arc;
use std::time::Instant;

use autd3_driver::defined::{
    mm, METER, ULTRASOUND_FREQ, ULTRASOUND_PERIOD, ULTRASOUND_PERIOD_COUNT,
};
use autd3_driver::derive::Segment;
use autd3_driver::ethercat::DcSysTime;
use egui::ahash::HashSet;
use egui::color_picker::color_picker_color32;
use egui::epaint::textures;
use egui::{
    ClippedPrimitive, DragValue, FullOutput, InputState, PointerButton, Vec2b, ViewportId,
    ViewportIdMap, ViewportInfo, ViewportOutput,
};
use egui_plot::{GridMark, Line, PlotPoints};
use egui_wgpu::wgpu::{Color, CommandEncoder, LoadOp, StoreOp, TextureView};
use egui_wgpu::{wgpu, Renderer, ScreenDescriptor};
use egui_winit::winit::event::DeviceEvent;
use egui_winit::{winit, ActionRequested, EventResponse};
use glam::{EulerRot, Quat};
use strum::IntoEnumIterator;
use wgpu::{Device, Queue, SurfaceConfiguration};
use winit::window::Window;

use crate::common::color_map::ColorMap;
use crate::emulator::EmulatorWrapper;
use crate::event::EventResult;
use crate::state::Tab;
use crate::update_flag::UpdateFlag;
use crate::{error::SimulatorError, Vector3, ZPARITY};

const MIN_COL_WIDTH: f32 = 120.;
const SPACING: [f32; 2] = [2.0, 4.0];

pub struct EguiRenderer {
    beginning: Instant,
    egui_winit: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    info: ViewportInfo,
    deferred_commands: Vec<egui::viewport::ViewportCommand>,
    actions_requested: HashSet<ActionRequested>,
    pending_full_output: egui::FullOutput,
    close: bool,
    is_first_frame: bool,
    initial_state: String,
}

impl EguiRenderer {
    pub fn new(
        state: &crate::State,
        device: &Device,
        egui_ctx: egui::Context,
        window: Arc<Window>,
        surface_config: &SurfaceConfiguration,
    ) -> Self {
        let egui_winit = egui_winit::State::new(
            egui_ctx,
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(2 * 1024),
        );
        let renderer = Renderer::new(device, surface_config.format, None, 1, true);

        let mut info = ViewportInfo::default();
        egui_winit::update_viewport_info(&mut info, egui_winit.egui_ctx(), &window, true);

        Self {
            beginning: Instant::now(),
            egui_winit,
            renderer,
            info,
            deferred_commands: Default::default(),
            pending_full_output: Default::default(),
            actions_requested: Default::default(),
            close: false,
            is_first_frame: true,
            initial_state: serde_json::to_string(state).unwrap(),
        }
    }

    pub fn create_egui_context() -> egui::Context {
        let egui_ctx = egui::Context::default();
        egui_ctx.set_embed_viewports(false);
        egui_ctx.options_mut(|o| {
            o.max_passes = 2.try_into().unwrap();
        });
        egui_ctx
    }

    pub fn context(&self) -> &egui::Context {
        self.egui_winit.egui_ctx()
    }

    pub fn close(&self) -> bool {
        self.close
    }

    pub fn info(&mut self) -> &mut ViewportInfo {
        &mut self.info
    }

    fn update(
        &mut self,
        mut raw_input: egui::RawInput,
        waiting: bool,
        state: &mut crate::State,
        emulator: &mut EmulatorWrapper,
        update_flag: &mut UpdateFlag,
    ) -> FullOutput {
        raw_input.time = Some(self.beginning.elapsed().as_secs_f64());

        let close_requested = raw_input.viewport().close_requested();

        let full_output = self.egui_winit.egui_ctx().run(raw_input, |egui_ctx| {
            if waiting {
                self._waiting(egui_ctx);
            } else {
                self._update(egui_ctx, state, emulator, update_flag);
            }
        });

        if close_requested {
            let canceled = full_output.viewport_output[&ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::CancelClose);
            if !canceled {
                self.close = true;
            }
        }

        self.pending_full_output.append(full_output);
        std::mem::take(&mut self.pending_full_output)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_ui_and_paint(
        &mut self,
        device: &Device,
        queue: &Queue,
        load: LoadOp<Color>,
        encoder: &mut CommandEncoder,
        window_surface_view: &TextureView,
        screen_descriptor: ScreenDescriptor,
        window: &Window,
        state: &mut crate::State,
        emulator: &mut EmulatorWrapper,
        update_flag: &mut UpdateFlag,
    ) -> Result<EventResult, SimulatorError> {
        let raw_input = {
            egui_winit::update_viewport_info(
                &mut self.info,
                self.egui_winit.egui_ctx(),
                window,
                false,
            );

            let mut raw_input = self.egui_winit.take_egui_input(window);

            raw_input.time = Some(self.beginning.elapsed().as_secs_f64());
            raw_input
                .viewports
                .insert(ViewportId::ROOT, self.info.clone());
            raw_input
        };

        let full_output = self.update(
            raw_input,
            !emulator.initialized(),
            state,
            emulator,
            update_flag,
        );

        let FullOutput {
            platform_output,
            shapes,
            pixels_per_point,
            viewport_output,
            textures_delta,
        } = full_output;

        self.info.events.clear();

        self.egui_winit
            .handle_platform_output(window, platform_output);

        let clipped_primitives = self
            .egui_winit
            .egui_ctx()
            .tessellate(shapes, pixels_per_point);

        self.paint_and_update_textures(
            device,
            queue,
            load,
            encoder,
            window_surface_view,
            screen_descriptor,
            clipped_primitives,
            textures_delta,
        );

        for action in self.actions_requested.drain() {
            match action {
                ActionRequested::Cut => {
                    self.egui_winit
                        .egui_input_mut()
                        .events
                        .push(egui::Event::Cut);
                }
                ActionRequested::Copy => {
                    self.egui_winit
                        .egui_input_mut()
                        .events
                        .push(egui::Event::Copy);
                }
                ActionRequested::Paste => {
                    if let Some(contents) = self.egui_winit.clipboard_text() {
                        let contents = contents.replace("\r\n", "\n");
                        if !contents.is_empty() {
                            self.egui_winit
                                .egui_input_mut()
                                .events
                                .push(egui::Event::Paste(contents));
                        }
                    }
                }
                _ => {}
            }
        }

        if std::mem::take(&mut self.is_first_frame) {
            window.set_visible(true);
        }

        self.handle_viewport_output(&viewport_output, window);

        if window.is_minimized() == Some(true) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        if self.close {
            Ok(EventResult::Exit)
        } else {
            Ok(EventResult::Wait)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint_and_update_textures(
        &mut self,
        device: &Device,
        queue: &Queue,
        load: LoadOp<Color>,
        encoder: &mut CommandEncoder,
        window_surface_view: &TextureView,
        screen_descriptor: ScreenDescriptor,
        clipped_primitives: Vec<ClippedPrimitive>,
        textures_delta: textures::TexturesDelta,
    ) {
        self.egui_winit
            .egui_ctx()
            .set_pixels_per_point(screen_descriptor.pixels_per_point);

        for (id, image_delta) in &textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );
        let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: egui_wgpu::wgpu::Operations {
                    load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            label: Some("egui main render pass"),
            occlusion_query_set: None,
        });

        self.renderer.render(
            &mut rpass.forget_lifetime(),
            &clipped_primitives,
            &screen_descriptor,
        );
        for x in &textures_delta.free {
            self.renderer.free_texture(x)
        }
    }

    fn handle_viewport_output(
        &mut self,
        viewport_output: &ViewportIdMap<ViewportOutput>,
        window: &Window,
    ) {
        for (
            _,
            ViewportOutput {
                parent: _,
                class: _,
                builder: _,
                viewport_ui_cb: _,
                mut commands,
                repaint_delay: _,
            },
        ) in viewport_output.clone()
        {
            self.deferred_commands.append(&mut commands);
            egui_winit::process_viewport_commands(
                self.egui_winit.egui_ctx(),
                &mut self.info,
                std::mem::take(&mut self.deferred_commands),
                window,
                &mut self.actions_requested,
            );
        }
    }

    fn update_camera_by_mouse(
        input: &InputState,
        state: &mut crate::State,
        update_flag: &mut UpdateFlag,
    ) {
        let rotation = state.camera.rotation();

        let r = rotation * Vector3::X;
        let u = rotation * Vector3::Y;
        let f = rotation * Vector3::Z;

        if let Some(mouse_wheel) = input.events.iter().find_map(|e| match e {
            egui::Event::MouseWheel { delta, .. } => Some(*delta),
            _ => None,
        }) {
            let trans = -f * mouse_wheel.y * state.camera.move_speed * 10. * ZPARITY;
            state.camera.pos += trans;
            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
        }

        {
            let mouse_delta = input.pointer.delta();
            if input.pointer.button_down(PointerButton::Middle) {
                if input.modifiers.shift {
                    let delta_x = mouse_delta[0] * state.camera.move_speed;
                    let delta_y = mouse_delta[1] * state.camera.move_speed;
                    let trans = -r * delta_x + u * delta_y;
                    state.camera.pos.x += trans.x;
                    state.camera.pos.y += trans.y;
                    state.camera.pos.z += trans.z;
                    update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                } else {
                    let delta_x = -mouse_delta[0] * state.camera.move_speed / METER * ZPARITY;
                    let delta_y = -mouse_delta[1] * state.camera.move_speed / METER * ZPARITY;

                    let rot = Quat::from_euler(glam::EulerRot::XYZ, delta_y, delta_x, 0.0);

                    let (rx, ry, rz) = (rotation * rot).to_euler(EulerRot::XYZ);
                    state.camera.rot.x = rx.to_degrees();
                    state.camera.rot.y = ry.to_degrees();
                    state.camera.rot.z = rz.to_degrees();
                    update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
                }
            }
        }
    }

    pub(crate) fn _update(
        &self,
        ctx: &egui::Context,
        state: &mut crate::State,
        emulator: &mut EmulatorWrapper,
        update_flag: &mut crate::update_flag::UpdateFlag,
    ) {
        egui::Window::new("Control panel")
            .resizable(true)
            .vscroll(true)
            .default_open(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.tab, Tab::SLICE, "Slice");
                    ui.selectable_value(&mut state.tab, Tab::CAMERA, "Camera");
                    ui.selectable_value(&mut state.tab, Tab::CONFIG, "Config");
                    ui.selectable_value(&mut state.tab, Tab::INFO, "Info");
                });
                ui.separator();
                match state.tab {
                    Tab::SLICE => Self::slice_tab(ui, state, update_flag),
                    Tab::CAMERA => Self::camera_tab(ui, state, update_flag),
                    Tab::CONFIG => Self::config_tab(ui, state, emulator, update_flag),
                    Tab::INFO => Self::info_tab(ui, state, emulator, update_flag),
                }

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.small_button("Default").clicked() {
                        state.merge(crate::State::default());
                        *update_flag = UpdateFlag::all();
                    }

                    if ui.small_button("Reset").clicked() {
                        let initial_state: crate::State =
                            serde_json::from_str(&self.initial_state).unwrap();
                        state.merge(initial_state);
                        *update_flag = UpdateFlag::all();
                    }
                });
            });

        if !ctx.wants_pointer_input() {
            ctx.input(|input| {
                Self::update_camera_by_mouse(input, state, update_flag);
            });
        }

        if state.auto_play {
            update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
            state.real_time = (DcSysTime::now().sys_time() as f64 * state.time_scale as f64) as _;
        }
    }

    fn slice_tab(ui: &mut egui::Ui, state: &mut crate::State, update_flag: &mut UpdateFlag) {
        ui.label("Position");
        if egui::Grid::new("slice_pos_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("X:");
                let response = ui.add(DragValue::new(&mut state.slice.pos.x).speed(1. * mm));
                ui.end_row();

                ui.label("Y:");
                let response =
                    response.union(ui.add(DragValue::new(&mut state.slice.pos.y).speed(1. * mm)));
                ui.end_row();

                ui.label("Z:");
                let response =
                    response.union(ui.add(DragValue::new(&mut state.slice.pos.z).speed(1. * mm)));
                ui.end_row();

                response
            })
            .inner
            .changed()
        {
            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
        }

        ui.separator();
        ui.label("Rotation");
        if egui::Grid::new("slice_rot_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("RX:");
                let response = ui.add(
                    DragValue::new(&mut state.slice.rot.x)
                        .speed(1.)
                        .range(-180.0..=180.0)
                        .suffix("°"),
                );
                ui.end_row();

                ui.label("RY:");
                let response = response.union(
                    ui.add(
                        DragValue::new(&mut state.slice.rot.y)
                            .speed(1.)
                            .range(-180.0..=180.0)
                            .suffix("°"),
                    ),
                );
                ui.end_row();

                ui.label("RZ:");
                let response = response.union(
                    ui.add(
                        DragValue::new(&mut state.slice.rot.z)
                            .speed(1.)
                            .range(-180.0..=180.0)
                            .suffix("°"),
                    ),
                );
                ui.end_row();

                response
            })
            .inner
            .changed()
        {
            update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
        }

        ui.separator();
        ui.label("Size");
        if egui::Grid::new("slice_size_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Width:");
                let response = ui.add(
                    DragValue::new(&mut state.slice.size.x)
                        .speed(1.)
                        .range(1.0..=1024.),
                );
                ui.end_row();

                ui.label("Height:");
                let response = response.union(
                    ui.add(
                        DragValue::new(&mut state.slice.size.y)
                            .speed(1.)
                            .range(1.0..=1024.),
                    ),
                );
                ui.end_row();

                response
            })
            .inner
            .changed()
        {
            update_flag.set(UpdateFlag::UPDATE_SLICE_SIZE, true);
        }

        ui.separator();
        ui.label("Color state");

        egui::Grid::new("slice_color_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Coloring:");
                egui::ComboBox::from_label("")
                    .selected_text(format!("{:?}", state.slice.color_map))
                    .show_ui(ui, |ui| {
                        ColorMap::iter().for_each(|c| {
                            if ui
                                .selectable_value(&mut state.slice.color_map, c, format!("{:?}", c))
                                .changed()
                            {
                                update_flag.set(UpdateFlag::UPDATE_SLICE_COLOR_MAP, true);
                            }
                        });
                    });
                ui.end_row();

                ui.label("Max pressure [Pa]:");
                if ui
                    .add(
                        DragValue::new(&mut state.slice.pressure_max)
                            .speed(100.)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    update_flag.set(UpdateFlag::UPDATE_CONFIG, true);
                }
                ui.end_row();
            });

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("xy").clicked() {
                state.slice.rot.x = 0.;
                state.slice.rot.y = 0.;
                state.slice.rot.z = 0.;
                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
            }

            if ui.button("yz").clicked() {
                state.slice.rot.x = 0.;
                state.slice.rot.y = 90.;
                state.slice.rot.z = 0.;
                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
            }

            if ui.button("zx").clicked() {
                state.slice.rot.x = 90.;
                state.slice.rot.y = 0.;
                state.slice.rot.z = 0.;
                update_flag.set(UpdateFlag::UPDATE_SLICE_POS, true);
            }
        });
    }

    fn camera_tab(ui: &mut egui::Ui, state: &mut crate::State, update_flag: &mut UpdateFlag) {
        ui.label("Position");
        if egui::Grid::new("camera_pos_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("X:");
                let response = ui.add(DragValue::new(&mut state.camera.pos.x).speed(1. * mm));
                ui.end_row();

                ui.label("Y:");
                let response =
                    response.union(ui.add(DragValue::new(&mut state.camera.pos.y).speed(1. * mm)));
                ui.end_row();

                ui.label("Z:");
                let response =
                    response.union(ui.add(DragValue::new(&mut state.camera.pos.z).speed(1. * mm)));
                ui.end_row();

                response
            })
            .inner
            .changed()
        {
            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
        }

        ui.separator();
        ui.label("Rotation");
        if egui::Grid::new("camera_rot_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("RX:");
                let response = ui.add(
                    DragValue::new(&mut state.camera.rot.x)
                        .speed(1.)
                        .range(-180.0..=180.0)
                        .suffix("°"),
                );
                ui.end_row();

                ui.label("RY:");
                let response = response.union(
                    ui.add(
                        DragValue::new(&mut state.camera.rot.y)
                            .speed(1.)
                            .range(-180.0..=180.0)
                            .suffix("°"),
                    ),
                );
                ui.end_row();

                ui.label("RZ:");
                let response = response.union(
                    ui.add(
                        DragValue::new(&mut state.camera.rot.z)
                            .speed(1.)
                            .range(-180.0..=180.0)
                            .suffix("°"),
                    ),
                );
                ui.end_row();

                response
            })
            .inner
            .changed()
        {
            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
        }

        ui.separator();
        egui::Grid::new("camera_set_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Move speed:");
                ui.add(
                    DragValue::new(&mut state.camera.move_speed)
                        .speed(0.1 * mm)
                        .range(1. * mm..=10.0 * mm),
                );
                ui.end_row();
            });

        ui.separator();
        ui.label("Perspective");
        if egui::Grid::new("camera_pers_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("FOV:");
                let response = ui.add(
                    DragValue::new(&mut state.camera.fov)
                        .speed(1.)
                        .range(0.0..=180.0)
                        .suffix("°"),
                );
                ui.end_row();

                ui.label("Near clip:");
                let response = response.union(
                    ui.add(
                        DragValue::new(&mut state.camera.near_clip)
                            .speed(1. * mm)
                            .range(0.0..=f32::MAX),
                    ),
                );
                ui.end_row();

                ui.label("Far clip:");
                let response = response.union(
                    ui.add(
                        DragValue::new(&mut state.camera.far_clip)
                            .speed(1. * mm)
                            .range(0.0..=f32::MAX),
                    ),
                );
                ui.end_row();

                response
            })
            .inner
            .changed()
        {
            update_flag.set(UpdateFlag::UPDATE_CAMERA, true);
        }
    }

    fn config_tab(
        ui: &mut egui::Ui,
        state: &mut crate::State,
        emulator: &mut EmulatorWrapper,
        update_flag: &mut UpdateFlag,
    ) {
        egui::Grid::new("config_env_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Sound speed:");
                if ui
                    .add(DragValue::new(&mut state.sound_speed).speed(100. * mm))
                    .changed()
                {
                    update_flag.set(UpdateFlag::UPDATE_CONFIG, true);
                }
                ui.end_row();
            });

        ui.label("Device index: show/enable/overheat");
        egui::Grid::new("config_device_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                emulator.iter_mut().enumerate().for_each(|(i, emulator)| {
                    ui.label(format!("Device {}: ", i));
                    ui.horizontal(|ui| {
                        if ui.checkbox(emulator.visible, "").changed() {
                            update_flag.set(UpdateFlag::UPDATE_TRANS_ALPHA, true);
                            let v = if *emulator.visible { 1. } else { 0. };
                            emulator.transducers.iter_mut().for_each(|s| s.alpha = v);
                        }

                        if ui.checkbox(emulator.enable, "").changed() {
                            update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
                            let v = if *emulator.enable { 1. } else { 0. };
                            emulator.transducers.iter_mut().for_each(|s| s.enable = v);
                        }

                        if ui.checkbox(emulator.thermal, "").changed() {
                            update_flag.set(UpdateFlag::UPDATE_DEVICE_INFO, true);
                            if *emulator.thermal {
                                emulator.cpu.fpga_mut().assert_thermal_sensor();
                            } else {
                                emulator.cpu.fpga_mut().deassert_thermal_sensor();
                            }
                        }
                    });
                    ui.end_row();
                });
            });

        ui.separator();

        egui::Grid::new("config_ui_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("UI scale:");
                ui.add(
                    DragValue::new(&mut state.ui_scale)
                        .speed(0.01)
                        .range(1.0..=10.0),
                );
                ui.end_row();

                ui.label("Background:");
                color_picker_color32(ui, &mut state.background, egui::color_picker::Alpha::Opaque);
            });
    }

    fn info_tab(
        ui: &mut egui::Ui,
        state: &mut crate::State,
        emulator: &mut EmulatorWrapper,
        update_flag: &mut UpdateFlag,
    ) {
        emulator.iter_mut().for_each(|emulator| {
            let cpu = emulator.cpu;
            ui.collapsing(format!("Device {}", cpu.idx()), |ui| {
                ui.collapsing("Silencer", |ui| {
                    if cpu.fpga().silencer_fixed_completion_steps_mode() {
                        ui.label(format!(
                            "Completion time intensity: {:?}",
                            cpu.fpga().silencer_completion_steps().intensity()
                        ));
                        ui.label(format!(
                            "Completion time phase: {:?}",
                            cpu.fpga().silencer_completion_steps().phase()
                        ));
                    } else {
                        ui.label(format!(
                            "Update rate intensity: {}",
                            cpu.fpga().silencer_update_rate().intensity()
                        ));
                        ui.label(format!(
                            "Update rate phase: {}",
                            cpu.fpga().silencer_update_rate().phase()
                        ));
                    }
                });

                ui.collapsing("Modulation", |ui| {
                    let segment = cpu.fpga().current_mod_segment();

                    let m = cpu.fpga().modulation_buffer(segment);

                    let mod_size = m.len();
                    ui.label(format!("Size: {}", mod_size));
                    ui.label(format!(
                        "Frequency division: {}",
                        cpu.fpga().modulation_freq_division(segment)
                    ));
                    let sampling_freq = ULTRASOUND_FREQ.hz() as f32
                        / cpu.fpga().modulation_freq_division(segment) as f32;
                    ui.label(format!("Sampling Frequency: {:.3}Hz", sampling_freq));
                    let sampling_period =
                        ULTRASOUND_PERIOD * cpu.fpga().modulation_freq_division(segment) as u32;
                    ui.label(format!("Sampling period: {:?}", sampling_period));
                    let period = sampling_period * mod_size as u32;
                    ui.label(format!("Period: {:?}", period));

                    ui.label(format!("Current Index: {}", cpu.fpga().current_mod_idx()));

                    if !m.is_empty() {
                        ui.label(format!("mod[0]: {}", m[0]));
                    }
                    if mod_size == 2 || mod_size == 3 {
                        ui.label(format!("mod[1]: {}", m[1]));
                    } else if mod_size > 3 {
                        ui.label("...");
                    }
                    if mod_size >= 3 {
                        ui.label(format!("mod[{}]: {}", mod_size - 1, m[mod_size - 1]));
                    }

                    ui.collapsing("Plot", |ui| {
                        egui_plot::Plot::new("plot")
                            .x_axis_label("Index")
                            .y_grid_spacer(|_g| {
                                vec![
                                    GridMark {
                                        value: 0.,
                                        step_size: 255.0,
                                    },
                                    GridMark {
                                        value: 255.,
                                        step_size: 255.0,
                                    },
                                ]
                            })
                            .width(ui.max_rect().width() * 0.8)
                            .height(200.)
                            .show(ui, |plot_ui| {
                                plot_ui.line(Line::new(PlotPoints::from_iter(
                                    m.into_iter().enumerate().map(|(i, v)| [i as f64, v as _]),
                                )));
                            });
                    });
                });

                ui.collapsing("STM", |ui| {
                    let segment = cpu.fpga().current_stm_segment();

                    let stm_cycle = cpu.fpga().stm_cycle(segment);

                    let is_gain_mode = stm_cycle == 1;

                    if is_gain_mode {
                        ui.label("Gain");
                    } else if cpu.fpga().is_stm_gain_mode(segment) {
                        ui.label("Gain STM");
                    } else {
                        ui.label("Focus STM");
                        #[cfg(feature = "use_meter")]
                        ui.label(format!(
                            "Sound speed: {:.3}m/s",
                            cpu.fpga().sound_speed(segment) as f32 / 64.0
                        ));
                        #[cfg(not(feature = "use_meter"))]
                        ui.label(format!(
                            "Sound speed: {:.3}mm/s",
                            cpu.fpga().sound_speed(segment) as f32 * 1000. / 64.0
                        ));
                    }

                    ui.label(format!("Segment: {:?}", segment));

                    if !is_gain_mode {
                        ui.label(format!(
                            "LoopBehavior: {:?}",
                            cpu.fpga().stm_loop_behavior(segment)
                        ));

                        let stm_size = cpu.fpga().stm_cycle(segment);
                        ui.label(format!("Size: {}", stm_size));
                        ui.label(format!(
                            "Frequency division: {}",
                            cpu.fpga().stm_freq_division(segment)
                        ));
                        let sampling_freq = ULTRASOUND_FREQ.hz() as f32
                            / cpu.fpga().stm_freq_division(segment) as f32;
                        ui.label(format!("Sampling Frequency: {:.3}Hz", sampling_freq));
                        let sampling_period =
                            ULTRASOUND_PERIOD * cpu.fpga().stm_freq_division(segment) as u32;
                        ui.label(format!("Sampling period: {:?}", sampling_period));
                        let period = sampling_period * stm_size as u32;
                        ui.label(format!("Period: {:?}", period));

                        ui.label(format!("Current Index: {}", cpu.fpga().current_stm_idx()));
                    }
                });

                ui.collapsing("GPIO", |ui| {
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
                                if cpu.fpga().stm_cycle(cpu.fpga().current_stm_segment()) != 1 {
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
                            let pulse_width = cpu.fpga().to_pulse_width(d.intensity(), m) as u32;
                            const T: u32 = ULTRASOUND_PERIOD_COUNT as u32;
                            let rise = (phase + T - pulse_width / 2) % T;
                            let fall = (phase + (pulse_width + 1) / 2) % T;
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
                        autd3_firmware_emulator::fpga::params::DBG_SYS_TIME_EQ => {
                            let now = (((cpu.dc_sys_time().sys_time() / 25000) << 8)
                                & 0x00FF_FFFF_FFFF_FFFF)
                                >> 8;
                            let value = value >> 8;
                            let v = if now == value { 1.0 } else { 0.0 };
                            vec![v; ULTRASOUND_PERIOD_COUNT]
                        }
                        autd3_firmware_emulator::fpga::params::DBG_DIRECT => {
                            vec![value as f32; ULTRASOUND_PERIOD_COUNT]
                        }
                        _ => unreachable!(),
                    };

                    (0..4).for_each(|i| {
                        let gpio_out = gpio_out(debug_types[i], debug_values[i]);
                        egui_plot::Plot::new(format!("gpio_{}", i))
                            .auto_bounds(Vec2b::new(true, false))
                            .y_grid_spacer(|_g| {
                                vec![
                                    GridMark {
                                        value: 0.,
                                        step_size: 1.0,
                                    },
                                    GridMark {
                                        value: 1.,
                                        step_size: 1.0,
                                    },
                                ]
                            })
                            .width(ui.max_rect().width() * 0.8)
                            .height(100.)
                            .show(ui, |plot_ui| {
                                plot_ui.line(Line::new(PlotPoints::from_iter(
                                    gpio_out
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, v)| [i as f64, v as _]),
                                )));
                            });
                    });
                });
            });
        });

        ui.separator();

        if ui.checkbox(&mut state.mod_enable, "Mod enable").changed() {
            update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
        }

        if ui.checkbox(&mut state.auto_play, "Auto play").changed() {
            update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
        }

        egui::Grid::new("info_systime_grid")
            .num_columns(2)
            .min_col_width(MIN_COL_WIDTH)
            .spacing(SPACING)
            .striped(true)
            .show(ui, |ui| {
                ui.label("System time [ns]:");
                ui.label(format!("{}", state.real_time));
                ui.end_row();

                if state.auto_play {
                    ui.label("Time scale:");
                    ui.add(
                        DragValue::new(&mut state.time_scale)
                            .speed(0.001)
                            .range(0.0..=f32::MAX),
                    );
                } else {
                    ui.label("");
                    ui.horizontal(|ui| {
                        if ui.button("+").clicked() {
                            state.real_time =
                                state.real_time.wrapping_add_signed(state.time_step as _);
                            update_flag.set(UpdateFlag::UPDATE_TRANS_STATE, true);
                        }
                        ui.add(
                            DragValue::new(&mut state.time_step)
                                .speed(1000)
                                .range(1..=i32::MAX),
                        );
                    });
                }
                ui.end_row();
            });
    }

    pub(crate) fn _waiting(&self, ctx: &egui::Context) {
        egui::Window::new("Control panel")
            .resizable(true)
            .vscroll(true)
            .default_open(true)
            .show(ctx, |ui| ui.label("Waiting for client connection..."));
    }

    pub fn on_window_event(
        &mut self,
        window: &Window,
        event: &egui_winit::winit::event::WindowEvent,
    ) -> EventResponse {
        self.egui_winit.on_window_event(window, event)
    }

    pub(crate) fn on_device_event(&mut self, event: DeviceEvent) -> EventResult {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            self.egui_winit.on_mouse_motion(delta);
            return EventResult::RepaintNext;
        }
        EventResult::Wait
    }
}
