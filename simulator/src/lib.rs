mod common;
mod emulator;
mod error;
mod event;
mod renderer;
mod server;
mod simulator;
mod state;
mod update_flag;

pub use simulator::Simulator;
pub use state::State;

pub type Vector2 = glam::Vec2;
pub type Vector3 = glam::Vec3;
pub type Vector4 = glam::Vec4;
pub type Quaternion = glam::Quat;
pub type Matrix3 = glam::Mat3;
pub type Matrix4 = glam::Mat4;

#[cfg(feature = "left_handed")]
pub(crate) const ZPARITY: f32 = -1.;
#[cfg(not(feature = "left_handed"))]
pub(crate) const ZPARITY: f32 = 1.;
