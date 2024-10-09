mod common;
mod context;
mod error;
// `imgui_wgpu` mod is forked from [Yatekii/imgui-wgpu-rs](https://github.com/Yatekii/imgui-wgpu-rs)
mod imgui_wgpu;
mod renderer;
mod server;
mod simulator;
mod state;
mod surface;
mod update_flag;

pub use error::SimulatorError;
pub use simulator::Simulator;
pub use state::State;

pub type Vector2 = glam::Vec2;
pub type Vector3 = glam::Vec3;
pub type Vector4 = glam::Vec4;
pub type Quaternion = glam::Quat;
pub type Matrix3 = glam::Mat3;
pub type Matrix4 = glam::Mat4;

const METER: f32 = autd3_driver::defined::METER;
const MILLIMETER: f32 = autd3_driver::defined::MILLIMETER;

#[cfg(feature = "left_handed")]
pub(crate) const ZPARITY: f32 = -1.;
#[cfg(not(feature = "left_handed"))]
pub(crate) const ZPARITY: f32 = 1.;
