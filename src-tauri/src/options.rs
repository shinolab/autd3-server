use serde::{Deserialize, Serialize};

use autd3_driver::{sync_mode::SyncMode, timer_strategy::TimerStrategy};

#[derive(Debug, Serialize, Deserialize)]
pub struct TwinCATOptions {
    pub client: String,
    pub sync0: u32,
    pub task: u32,
    pub base: u32,
    pub mode: SyncMode,
    pub keep: bool,
}

impl Default for TwinCATOptions {
    fn default() -> Self {
        Self {
            client: "".to_string(),
            sync0: 2,
            task: 2,
            base: 1,
            mode: SyncMode::DC,
            keep: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SOEMOptions {
    pub ifname: String,
    pub port: u16,
    pub sync0: u32,
    pub send: u32,
    pub buf_size: usize,
    pub mode: SyncMode,
    pub timer_strategy: TimerStrategy,
    pub state_check_interval: std::time::Duration,
    pub timeout: std::time::Duration,
    pub debug: bool,
}

impl Default for SOEMOptions {
    fn default() -> Self {
        Self {
            ifname: "".to_string(),
            port: 8080,
            sync0: 2,
            send: 2,
            buf_size: 32,
            mode: SyncMode::FreeRun,
            timer_strategy: TimerStrategy::Sleep,
            state_check_interval: std::time::Duration::from_millis(500),
            timeout: std::time::Duration::from_millis(200),
            debug: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimulatorOptions {
    pub vsync: bool,
    pub port: u16,
    pub gpu_idx: i32,
    pub window_width: u32,
    pub window_height: u32,
    #[serde(default)]
    pub unity: bool,
}

impl Default for SimulatorOptions {
    fn default() -> Self {
        Self {
            vsync: true,
            port: 8080,
            gpu_idx: -1,
            window_width: 800,
            window_height: 600,
            unity: false,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Options {
    pub twincat: TwinCATOptions,
    pub soem: SOEMOptions,
    pub simulator: SimulatorOptions,
}
