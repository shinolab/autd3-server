use camera_controllers::Camera;
use cgmath::{Deg, Euler};

use crate::{
    common::transform::{to_gl_pos, to_gl_rot},
    Quaternion, Vector3,
};

pub fn set_camera(camera: &mut Camera<f32>, pos: Vector3, angle: Vector3) {
    let pos = to_gl_pos(pos);
    camera.position = [pos.x, pos.y, pos.z];

    let rotation = Quaternion::from(Euler {
        x: Deg(angle.x),
        y: Deg(angle.y),
        z: Deg(angle.z),
    });
    let rotation = to_gl_rot(rotation);
    camera.right = (rotation * Vector3::unit_x()).into();
    camera.up = (rotation * Vector3::unit_y()).into();
    camera.forward = (rotation * Vector3::unit_z()).into();
}
