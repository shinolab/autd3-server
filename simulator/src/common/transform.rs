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

pub fn quaternion_to(v: Vector3, to: Vector3) -> Quaternion {
    let a = v.normalize();
    let b = to.normalize();
    let c = b.cross(a).normalize();
    if c.x.is_nan() || c.y.is_nan() || c.z.is_nan() {
        return Quaternion::IDENTITY;
    }
    let ip = a.dot(b);
    const EPS: f32 = 1e-4;
    if c.length() < EPS || 1. < ip {
        if ip < EPS - 1. {
            let a2 = Vector3::new(-a.y, a.z, a.x);
            let c2 = a2.cross(a).normalize();
            return Quaternion::from_xyzw(c2.x, c2.y, c2.z, 0.);
        }
        return Quaternion::IDENTITY;
    }
    let e = c * (0.5 * (1. - ip)).sqrt();
    Quaternion::from_xyzw(e.x, e.y, e.z, (0.5 * (1. + ip)).sqrt())
}
