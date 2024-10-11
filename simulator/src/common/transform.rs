use crate::{Quaternion, Vector3};

pub fn to_gl_pos(v: Vector3) -> Vector3 {
    if cfg!(feature = "left_handed") {
        Vector3::new(v.x, v.y, -v.z)
    } else {
        v
    }
}

pub fn to_gl_rot(v: Quaternion) -> Quaternion {
    if cfg!(feature = "left_handed") {
        Quaternion::from_xyzw(-v.x, -v.y, v.z, v.w)
    } else {
        v
    }
}
