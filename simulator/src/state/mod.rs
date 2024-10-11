use std::f32::consts::PI;

use autd3_driver::{
    defined::{mm, ULTRASOUND_PERIOD_COUNT},
    ethercat::{DcSysTime, ECAT_DC_SYS_TIME_BASE},
    geometry::{self},
};
use autd3_firmware_emulator::CPUEmulator;
use glam::EulerRot;
use serde::{Deserialize, Serialize};

use crate::{common::color_map::ColorMap, Quaternion, Vector2, Vector3, ZPARITY};

mod transducers;

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

#[derive(Serialize, Deserialize)]
pub struct State {
    pub window_size: (u32, u32),
    #[serde(skip)]
    pub cpus: Vec<CPUEmulator>,
    #[serde(skip)]
    pub transducers: transducers::Transducers,
    pub camera: CameraState,
    pub slice: SliceState,
    pub sound_speed: f32,
    pub background: [f32; 4],
    pub mod_enable: bool,
    pub auto_play: bool,
    #[serde(skip)]
    pub real_time: u64,
    pub time_scale: f32,
    pub port: u16,
    pub lightweight: bool,
    pub lightweight_port: u16,
    pub vsync: bool,
    pub settings_dir: String,
}

impl std::default::Default for State {
    fn default() -> Self {
        Self {
            window_size: (800, 600),
            transducers: transducers::Transducers::new(),
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
            background: [0.3, 0.3, 0.3, 1.0],
            sound_speed: 340.0e3 * mm,
            mod_enable: false,
            auto_play: false,
            real_time: DcSysTime::now().sys_time(),
            time_scale: 1.0,
            cpus: Vec::new(),
            port: 8080,
            lightweight: false,
            lightweight_port: 8081,
            vsync: true,
            settings_dir: String::new(),
        }
    }
}

impl State {
    pub fn num_devices(&self) -> usize {
        self.cpus.len()
    }

    pub fn init(&mut self, geometry: geometry::Geometry) {
        self.cpus = geometry
            .iter()
            .map(|dev| CPUEmulator::new(dev.idx(), dev.num_transducers()))
            .collect();
        self.transducers.init(geometry);
    }

    pub fn update(&mut self) {
        let system_time = self.system_time();
        self.cpus.iter_mut().for_each(|cpu| {
            cpu.update_with_sys_time(system_time);
        });
    }

    pub fn update_geometry(&mut self, geometry: geometry::Geometry) {
        self.transducers.update_geometry(geometry);
    }

    pub fn update_trans(&mut self) {
        self.cpus.iter().for_each(|cpu| {
            let stm_segment = cpu.fpga().current_stm_segment();
            let idx = if cpu.fpga().stm_cycle(stm_segment) == 1 {
                0
            } else {
                cpu.fpga().current_stm_idx()
            };
            let drives = cpu.fpga().drives_at(stm_segment, idx);
            let mod_segment = cpu.fpga().current_mod_segment();
            let m = if self.mod_enable {
                let mod_idx = cpu.fpga().current_mod_idx();
                cpu.fpga().modulation_at(mod_segment, mod_idx)
            } else {
                u8::MAX
            };
            self.transducers
                .device(cpu.idx())
                .states
                .iter_mut()
                .enumerate()
                .for_each(|(i, d)| {
                    d.amp = (PI * cpu.fpga().to_pulse_width(drives[i].intensity(), m) as f32
                        / ULTRASOUND_PERIOD_COUNT as f32)
                        .sin();
                    d.phase = drives[i].phase().radian();
                });
        });
    }

    pub fn clear(&mut self) {
        self.transducers.clear();
        self.cpus.clear();
    }

    pub fn initialized(&self) -> bool {
        !self.cpus.is_empty()
    }

    pub fn system_time(&self) -> DcSysTime {
        DcSysTime::from_utc(ECAT_DC_SYS_TIME_BASE + std::time::Duration::from_nanos(self.real_time))
            .unwrap()
    }

    pub fn background(&self) -> wgpu::Color {
        wgpu::Color {
            r: self.background[0] as _,
            g: self.background[1] as _,
            b: self.background[2] as _,
            a: self.background[3] as _,
        }
    }
}
