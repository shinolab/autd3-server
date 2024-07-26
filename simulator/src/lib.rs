mod common;
mod field_compute_pipeline;
mod patch;
mod prelude;
mod renderer;
mod simulator;
mod sound_sources;
mod update_flag;
mod view;
mod viewer_settings;

pub use common::log_formatter::LogFormatter;
pub use simulator::Simulator;
pub use viewer_settings::ViewerSettings;

pub use renderer::available_gpus;

pub type Vector3 = cgmath::Vector3<f32>;
pub type Vector4 = cgmath::Vector4<f32>;
pub type Quaternion = cgmath::Quaternion<f32>;
pub type Matrix3 = cgmath::Matrix3<f32>;
pub type Matrix4 = cgmath::Matrix4<f32>;

const METER: f32 = autd3::driver::defined::METER;
const MILLIMETER: f32 = autd3::driver::defined::MILLIMETER;

#[cfg(feature = "left_handed")]
pub(crate) const ZPARITY: f32 = -1.;
#[cfg(not(feature = "left_handed"))]
pub(crate) const ZPARITY: f32 = 1.;
