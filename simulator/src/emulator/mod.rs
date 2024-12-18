mod transducers;

use std::{f32::consts::PI, sync::Arc};

use autd3_driver::{
    defined::ULTRASOUND_PERIOD_COUNT,
    derive::Geometry,
    ethercat::DcSysTime,
    firmware::cpu::{RxMessage, TxMessage},
};
use autd3_firmware_emulator::CPUEmulator;
use parking_lot::RwLock;

pub struct Emulator<'a> {
    pub cpu: &'a mut CPUEmulator,
    pub transducers: &'a mut [transducers::TransState],
    pub visible: &'a mut bool,
    pub enable: &'a mut bool,
    pub thermal: &'a mut bool,
}

pub struct EmulatorWrapper {
    cpus: Vec<CPUEmulator>,
    transducers: transducers::Transducers,
    rx_buf: Arc<RwLock<Vec<RxMessage>>>,
    visible: Vec<bool>,
    enable: Vec<bool>,
    thermal: Vec<bool>,
}

impl EmulatorWrapper {
    pub fn new(rx_buf: Arc<RwLock<Vec<RxMessage>>>) -> Self {
        Self {
            cpus: Default::default(),
            transducers: transducers::Transducers::new(),
            rx_buf,
            visible: Default::default(),
            enable: Default::default(),
            thermal: Default::default(),
        }
    }

    pub fn initialized(&self) -> bool {
        !self.cpus.is_empty()
    }

    pub fn transducers(&self) -> &transducers::Transducers {
        &self.transducers
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = Emulator> {
        self.cpus
            .iter_mut()
            .zip(self.visible.iter_mut())
            .zip(self.enable.iter_mut())
            .zip(self.thermal.iter_mut())
            .zip(self.transducers.devices())
            .map(
                |((((cpu, visible), enable), thermal), transducers)| Emulator {
                    cpu,
                    transducers,
                    visible,
                    enable,
                    thermal,
                },
            )
    }

    pub fn update(&mut self, system_time: DcSysTime) {
        self.cpus.iter_mut().for_each(|cpu| {
            cpu.update_with_sys_time(system_time);
        });
        if self
            .cpus
            .iter()
            .any(autd3_firmware_emulator::CPUEmulator::should_update)
        {
            self.rx_buf
                .write()
                .iter_mut()
                .zip(self.cpus.iter())
                .for_each(|(d, s)| {
                    *d = s.rx();
                });
        }
    }

    pub fn update_transducers(&mut self, mod_enable: bool) {
        self.iter_mut().for_each(|emulator| {
            let cpu = emulator.cpu;
            let stm_segment = cpu.fpga().current_stm_segment();
            let idx = if cpu.fpga().stm_cycle(stm_segment) == 1 {
                0
            } else {
                cpu.fpga().current_stm_idx()
            };
            let drives = cpu.fpga().drives_at(stm_segment, idx);
            let mod_segment = cpu.fpga().current_mod_segment();
            let m = if mod_enable {
                let mod_idx = cpu.fpga().current_mod_idx();
                cpu.fpga().modulation_at(mod_segment, mod_idx)
            } else {
                u8::MAX
            };
            emulator
                .transducers
                .iter_mut()
                .zip(drives.iter())
                .for_each(|(tr, d)| {
                    tr.amp = (PI * cpu.fpga().to_pulse_width(d.intensity(), m) as f32
                        / ULTRASOUND_PERIOD_COUNT as f32)
                        .sin();
                    tr.phase = d.phase().radian();
                });
        });
    }

    pub fn initialize(&mut self, geometry: &Geometry) {
        self.cpus = geometry
            .iter()
            .map(|dev| CPUEmulator::new(dev.idx(), dev.num_transducers()))
            .collect();
        self.transducers.initialize(geometry);
        *self.rx_buf.write() = self.cpus.iter().map(|cpu| cpu.rx()).collect();
        self.visible = vec![true; self.cpus.len()];
        self.enable = vec![true; self.cpus.len()];
        self.thermal = vec![false; self.cpus.len()];
    }

    pub fn update_geometry(&mut self, geometry: &Geometry) {
        self.transducers.update_geometry(geometry);
    }

    pub fn send(&mut self, tx: &[TxMessage]) {
        self.cpus.iter_mut().for_each(|cpu| {
            cpu.send(tx);
        });
        self.rx_buf
            .write()
            .iter_mut()
            .zip(self.cpus.iter())
            .for_each(|(d, s)| {
                *d = s.rx();
            });
    }

    pub fn clear(&mut self) {
        self.cpus.clear();
        self.transducers.clear();
        self.visible.clear();
        self.enable.clear();
        self.thermal.clear();
    }
}
