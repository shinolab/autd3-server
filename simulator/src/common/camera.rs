pub use camera_controllers::{Camera, CameraPerspective};
use camera_controllers::{FirstPerson, FirstPersonSettings};
use glam::EulerRot;

use crate::{
    common::transform::{to_gl_pos, to_gl_rot},
    Quaternion, Vector3,
};

pub fn create_camera() -> Camera<f32> {
    FirstPerson::new([0., 0., 0.], FirstPersonSettings::keyboard_wasd()).camera(0.)
}

pub fn set_camera(camera: &mut Camera<f32>, pos: Vector3, angle: Vector3) {
    camera.position = to_gl_pos(pos).into();

    let rotation = Quaternion::from_euler(
        EulerRot::XYZ,
        angle.x.to_radians(),
        angle.y.to_radians(),
        angle.z.to_radians(),
    );
    let rotation = to_gl_rot(rotation);
    camera.right = (rotation * Vector3::X).into();
    camera.up = (rotation * Vector3::Y).into();
    camera.forward = (rotation * Vector3::Z).into();
}
