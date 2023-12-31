/*
 * File: lib.rs
 * Project: src
 * Created Date: 17/12/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 14/10/2023
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2022 Hapis Lab. All rights reserved.
 *
 */

mod camera_helper;
mod common;
mod device_viewer;
mod field_compute_pipeline;
mod imgui_renderer;
mod log_formatter;
mod patch;
mod renderer;
mod simulator;
mod slice_viewer;
mod sound_sources;
mod trans_viewer;
mod update_flag;
mod viewer_settings;

pub use log_formatter::LogFormatter;
pub use simulator::Simulator;
pub use viewer_settings::ViewerSettings;

pub use renderer::available_gpus;

pub type Vector2 = cgmath::Vector2<f32>;
pub type Vector3 = cgmath::Vector3<f32>;
pub type Vector4 = cgmath::Vector4<f32>;
pub type Quaternion = cgmath::Quaternion<f32>;
pub type Matrix3 = cgmath::Matrix3<f32>;
pub type Matrix4 = cgmath::Matrix4<f32>;

const METER: f32 = autd3::prelude::METER as f32;
const MILLIMETER: f32 = autd3::prelude::MILLIMETER as f32;

#[cfg(feature = "left_handed")]
pub(crate) const ZPARITY: f32 = -1.;
#[cfg(not(feature = "left_handed"))]
pub(crate) const ZPARITY: f32 = 1.;
