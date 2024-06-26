use crate::{Quaternion, Vector3, MILLIMETER, ZPARITY};
use cgmath::{Deg, Euler};
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use strum::EnumIter;

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq, EnumIter)]
pub enum ColorMapType {
    Bluered,
    Breeze,
    Circle,
    Earth,
    Hell,
    Inferno,
    Magma,
    Mist,
    Plasma,
    Turbo,
    Viridis,
}

/// Viewer settings
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ViewerSettings {
    pub window_width: u32,
    pub window_height: u32,
    pub vsync: bool,
    pub gpu_idx: i32,
    pub slice_pos_x: f32,
    pub slice_pos_y: f32,
    pub slice_pos_z: f32,
    pub slice_width: f32,
    pub slice_height: f32,
    pub slice_pixel_size: f32,
    pub camera_pos_x: f32,
    pub camera_pos_y: f32,
    pub camera_pos_z: f32,
    pub camera_near_clip: f32,
    pub camera_far_clip: f32,
    pub sound_speed: f32,
    pub slice_rot_x: f32,
    pub slice_rot_y: f32,
    pub slice_rot_z: f32,
    #[serde(default = "default_pressure_max")]
    pub pressure_max: f32,
    #[serde(default = "default_slice_show")]
    pub slice_show: bool,
    pub color_map_type: ColorMapType,
    pub show_radiation_pressure: bool,
    pub camera_rot_x: f32,
    pub camera_rot_y: f32,
    pub camera_rot_z: f32,
    pub camera_fov: f32,
    pub font_size: f32,
    pub background: [f32; 4],
    pub mod_enable: bool,
    pub auto_play: bool,
    pub image_save_path: String,
    pub port: u16,
    #[serde(default = "default_lightweight_port")]
    pub lighweight_port: u16,
    pub camera_move_speed: f32,
    #[serde(default)]
    pub view_device: bool,
    #[serde(default = "default_time_scale")]
    pub time_scale: f32,
}

fn default_pressure_max() -> f32 {
    5e3
}

fn default_time_scale() -> f32 {
    1.
}

fn default_slice_show() -> bool {
    true
}

fn default_lightweight_port() -> u16 {
    8081
}

impl ViewerSettings {
    pub fn new() -> ViewerSettings {
        Self::default()
    }

    pub(crate) const fn slice_pos(&self) -> Vector3 {
        Vector3::new(self.slice_pos_x, self.slice_pos_y, self.slice_pos_z)
    }

    pub(crate) fn slice_rotation(&self) -> Quaternion {
        Quaternion::from(Euler {
            x: Deg(self.slice_rot_x),
            y: Deg(self.slice_rot_y),
            z: Deg(self.slice_rot_z),
        })
    }

    pub(crate) fn set_camera_pos(&mut self, v: Vector3) {
        self.camera_pos_x = v.x;
        self.camera_pos_y = v.y;
        self.camera_pos_z = v.z;
    }

    pub(crate) fn set_camera_rot(&mut self, rot: Quaternion) {
        let euler = Euler::from(rot);
        self.camera_rot_x = Deg::from(euler.x).0;
        self.camera_rot_y = Deg::from(euler.y).0;
        self.camera_rot_z = Deg::from(euler.z).0;
    }
}

impl Default for ViewerSettings {
    fn default() -> Self {
        let image_save_path = if let Some(user_dirs) = directories::UserDirs::new() {
            if let Some(p) = user_dirs.picture_dir() {
                let mut path = p.to_path_buf();
                path.push("image.png");
                path.to_str().unwrap().to_string()
            } else {
                "image.png".to_string()
            }
        } else {
            "image.png".to_string()
        };
        ViewerSettings {
            window_width: 800,
            window_height: 600,
            vsync: true,
            gpu_idx: 0,
            slice_pos_x: 86.6252 * MILLIMETER,
            slice_pos_y: 66.7133 * MILLIMETER,
            slice_pos_z: 150.0 * MILLIMETER * ZPARITY,
            slice_width: 300.0 * MILLIMETER,
            slice_height: 300.0 * MILLIMETER,
            slice_pixel_size: 1.0 * MILLIMETER,
            camera_pos_x: 86.6252 * MILLIMETER,
            camera_pos_y: -533.2867 * MILLIMETER,
            camera_pos_z: 150.0 * MILLIMETER * ZPARITY,
            camera_near_clip: 0.1 * MILLIMETER,
            camera_far_clip: 1000. * MILLIMETER,
            sound_speed: 340.0e3 * MILLIMETER,
            slice_rot_x: 90.0 * ZPARITY,
            slice_rot_y: 0.,
            slice_rot_z: 0.,
            pressure_max: 5e3,
            slice_show: true,
            color_map_type: ColorMapType::Inferno,
            show_radiation_pressure: false,
            camera_rot_x: 90.0 * ZPARITY,
            camera_rot_y: 0.,
            camera_rot_z: 0.,
            camera_fov: 45.,
            font_size: 16.,
            background: [0.3, 0.3, 0.3, 1.],
            mod_enable: false,
            auto_play: false,
            image_save_path,
            port: 8080,
            lighweight_port: 8081,
            camera_move_speed: 10. * MILLIMETER,
            view_device: false,
            time_scale: 1.0,
        }
    }
}
