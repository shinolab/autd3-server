use autd3_driver::{
    defined::mm,
    ethercat::{DcSysTime, ECAT_DC_SYS_TIME_BASE},
};

use glam::EulerRot;
use serde::{Deserialize, Serialize};

use crate::{common::color_map::ColorMap, Quaternion, Vector2, Vector3, ZPARITY};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CameraState {
    pub pos: Vector3,
    pub rot: Vector3,
    pub fov: f32,
    pub near_clip: f32,
    pub far_clip: f32,
    pub move_speed: f32,
}

impl CameraState {
    pub fn rotation(&self) -> Quaternion {
        Quaternion::from_euler(
            EulerRot::XYZ,
            self.rot.x.to_radians(),
            self.rot.y.to_radians(),
            self.rot.z.to_radians(),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SliceState {
    pub pos: Vector3,
    pub rot: Vector3,
    pub size: Vector2,
    pub color_map: ColorMap,
    pub pressure_max: f32,
}

impl SliceState {
    pub fn rotation(&self) -> Quaternion {
        Quaternion::from_euler(
            EulerRot::XYZ,
            self.rot.x.to_radians(),
            self.rot.y.to_radians(),
            self.rot.z.to_radians(),
        )
    }
}

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Slice,
    Camera,
    Config,
    Info,
}

#[derive(Serialize, Deserialize)]
pub struct State {
    pub window_size: (u32, u32),
    pub ui_scale: f32,
    pub camera: CameraState,
    pub slice: SliceState,
    pub sound_speed: f32,
    pub background: egui::Color32,
    pub mod_enable: bool,
    pub auto_play: bool,
    pub real_time: u64,
    pub time_scale: f32,
    pub port: u16,
    pub lightweight: bool,
    pub vsync: bool,
    pub settings_dir: String,
    pub time_step: i32,
    pub debug: bool,
    pub tab: Tab,
}

impl std::default::Default for State {
    fn default() -> Self {
        Self {
            window_size: (800, 600),
            ui_scale: 1.0,
            camera: CameraState {
                pos: Vector3::new(86.6252 * mm, -533.2867 * mm, 150.0 * mm * ZPARITY),
                rot: Vector3::new(90.0 * ZPARITY, 0., 0.),
                fov: 45.,
                near_clip: 0.1 * mm,
                far_clip: 1000. * mm,
                move_speed: 1. * mm,
            },
            slice: SliceState {
                pos: Vector3::new(86.6252 * mm, 66.7133 * mm, 150.0 * mm * ZPARITY),
                rot: Vector3::new(90.0 * ZPARITY, 0., 0.),
                size: Vector2::new(300.0 * mm, 300.0 * mm),
                color_map: ColorMap::Inferno,
                pressure_max: 5000.,
            },
            background: egui::Color32::from_rgb(60, 60, 60),
            sound_speed: 340.0e3 * mm,
            mod_enable: false,
            auto_play: true,
            real_time: DcSysTime::now().sys_time(),
            time_scale: 1.0,
            port: 8080,
            lightweight: false,
            vsync: true,
            settings_dir: String::new(),
            time_step: 1000000,
            debug: false,
            tab: Tab::default(),
        }
    }
}

impl State {
    pub fn system_time(&self) -> DcSysTime {
        DcSysTime::from_utc(ECAT_DC_SYS_TIME_BASE + std::time::Duration::from_nanos(self.real_time))
            .unwrap()
    }

    pub fn background(&self) -> wgpu::Color {
        wgpu::Color {
            r: self.background[0] as f64 / 255.,
            g: self.background[1] as f64 / 255.,
            b: self.background[2] as f64 / 255.,
            a: self.background[3] as f64 / 255.,
        }
    }

    pub fn merge(&mut self, state: State) {
        self.window_size = state.window_size;
        self.ui_scale = state.ui_scale;
        self.camera = state.camera;
        self.slice = state.slice;
        self.sound_speed = state.sound_speed;
        self.background = state.background;
        self.mod_enable = state.mod_enable;
        self.auto_play = state.auto_play;
        self.time_scale = state.time_scale;
        self.port = state.port;
        self.lightweight = state.lightweight;
        self.vsync = state.vsync;
        self.settings_dir = state.settings_dir;
        self.debug = state.debug;
    }
}
