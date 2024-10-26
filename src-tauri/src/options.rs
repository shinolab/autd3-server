use serde::{Deserialize, Serialize};

use autd3_link_soem::TimerStrategy;

#[derive(Debug, Serialize, Deserialize)]
pub struct TwinCATOptions {
    pub client: String,
    pub sync0: u32,
    pub task: u32,
    pub base: u32,
    pub keep: bool,
    pub lightweight: bool,
    pub lightweight_port: u16,
}

impl Default for TwinCATOptions {
    fn default() -> Self {
        Self {
            client: "".to_string(),
            sync0: 2,
            task: 2,
            base: 1,
            keep: false,
            lightweight: false,
            lightweight_port: 8080,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SOEMOptions {
    pub ifname: String,
    pub port: u16,
    pub sync0: std::time::Duration,
    pub send: std::time::Duration,
    pub buf_size: usize,
    pub timer_strategy: TimerStrategy,
    pub state_check_interval: std::time::Duration,
    pub sync_tolerance: std::time::Duration,
    pub sync_timeout: std::time::Duration,
    pub lightweight: bool,
}

impl Default for SOEMOptions {
    fn default() -> Self {
        Self {
            ifname: "".to_string(),
            port: 8080,
            sync0: std::time::Duration::from_millis(1),
            send: std::time::Duration::from_millis(1),
            buf_size: 32,
            timer_strategy: TimerStrategy::SpinSleep,
            state_check_interval: std::time::Duration::from_millis(100),
            sync_tolerance: std::time::Duration::from_micros(1),
            sync_timeout: std::time::Duration::from_secs(10),
            lightweight: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimulatorOptions {
    pub vsync: bool,
    pub port: u16,
    pub window_width: u32,
    pub window_height: u32,
    pub unity: bool,
    pub lightweight: bool,
    pub lightweight_port: u16,
}

impl Default for SimulatorOptions {
    fn default() -> Self {
        Self {
            vsync: true,
            port: 8080,
            window_width: 800,
            window_height: 600,
            unity: false,
            lightweight: false,
            lightweight_port: 8081,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Options {
    pub twincat: TwinCATOptions,
    pub soem: SOEMOptions,
    pub simulator: SimulatorOptions,
}
