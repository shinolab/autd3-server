use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Sleeper {
    Std,
    Spin,
    SpinWait,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CpuBaseTime {
    #[serde(rename = "1ms")]
    T1ms,
    #[serde(rename = "500us")]
    T500us,
    #[serde(rename = "333us")]
    T333us,
    #[serde(rename = "250us")]
    T250us,
    #[serde(rename = "200us")]
    T200us,
    #[serde(rename = "125us")]
    T125us,
    #[serde(rename = "100us")]
    T100us,
    #[serde(rename = "83.3us")]
    T83p3us,
    #[serde(rename = "76.9us")]
    T76p9us,
    #[serde(rename = "71.4us")]
    T71p4us,
    #[serde(rename = "66.6us")]
    T66p6us,
    #[serde(rename = "62.5us")]
    T62p5us,
    #[serde(rename = "50us")]
    T50us,
}

impl std::fmt::Display for CpuBaseTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CpuBaseTime::T1ms => write!(f, "1ms"),
            CpuBaseTime::T500us => write!(f, "500us"),
            CpuBaseTime::T333us => write!(f, "333us"),
            CpuBaseTime::T250us => write!(f, "250us"),
            CpuBaseTime::T200us => write!(f, "200us"),
            CpuBaseTime::T125us => write!(f, "125us"),
            CpuBaseTime::T100us => write!(f, "100us"),
            CpuBaseTime::T83p3us => write!(f, "83.3us"),
            CpuBaseTime::T76p9us => write!(f, "76.9us"),
            CpuBaseTime::T71p4us => write!(f, "71.4us"),
            CpuBaseTime::T66p6us => write!(f, "66.6us"),
            CpuBaseTime::T62p5us => write!(f, "62.5us"),
            CpuBaseTime::T50us => write!(f, "50us"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TwinCATVersion {
    #[serde(rename = "4024")]
    Build4024,
    #[serde(rename = "4026")]
    Build4026,
}

impl std::fmt::Display for TwinCATVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TwinCATVersion::Build4024 => write!(f, "4024"),
            TwinCATVersion::Build4026 => write!(f, "4026"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwinCATOptions {
    pub client: String,
    pub device_name: String,
    pub version: TwinCATVersion,
    pub sync0: u32,
    pub task: u32,
    pub base: CpuBaseTime,
    pub keep: bool,
    pub debug: bool,
}

impl Default for TwinCATOptions {
    fn default() -> Self {
        Self {
            client: "".to_string(),
            device_name: "".to_string(),
            version: TwinCATVersion::Build4026,
            sync0: 2,
            task: 1,
            base: CpuBaseTime::T1ms,
            keep: true,
            debug: false,
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
    pub sleeper: Sleeper,
    pub state_check_interval: std::time::Duration,
    pub sync_tolerance: std::time::Duration,
    pub sync_timeout: std::time::Duration,
}

impl Default for SOEMOptions {
    fn default() -> Self {
        Self {
            ifname: "".to_string(),
            port: 8080,
            sync0: std::time::Duration::from_millis(1),
            send: std::time::Duration::from_millis(1),
            buf_size: 32,
            sleeper: Sleeper::Spin,
            state_check_interval: std::time::Duration::from_millis(100),
            sync_tolerance: std::time::Duration::from_micros(1),
            sync_timeout: std::time::Duration::from_secs(10),
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
}

impl Default for SimulatorOptions {
    fn default() -> Self {
        Self {
            vsync: true,
            port: 8080,
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
