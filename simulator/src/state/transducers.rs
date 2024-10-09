use autd3_driver::derive::Geometry;
use bytemuck::{Pod, Zeroable};

use crate::{
    common::transform::{to_gl_pos, to_gl_rot},
    Quaternion, Vector3, Vector4,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct TransState {
    pub amp: f32,
    pub phase: f32,
    pub enable: f32,
    pub alpha: f32,
}

#[derive(Debug, Default)]
pub struct Transducers {
    pub positions: Vec<Vector4>,
    pub rotations: Vec<Quaternion>,
    pub states: Vec<TransState>,
    pub body_pointer: Vec<usize>,
}

pub struct SubTransducers<'a> {
    pub states: &'a mut [TransState],
}

impl Transducers {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            rotations: Vec::new(),
            states: Vec::new(),
            body_pointer: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.positions.clear();
        self.rotations.clear();
        self.states.clear();
        self.body_pointer.clear();
    }

    pub fn device(&mut self, dev_idx: usize) -> SubTransducers {
        let end = self.body_pointer[dev_idx + 1];
        let start = self.body_pointer[dev_idx];
        SubTransducers {
            states: &mut self.states[start..end],
        }
    }

    pub fn init(&mut self, geometry: Geometry) {
        self.positions.clear();
        self.rotations.clear();
        self.states.clear();
        self.body_pointer.clear();

        let mut body_cursor = 0;
        self.body_pointer.push(body_cursor);
        geometry.into_iter().for_each(|dev| {
            body_cursor += dev.num_transducers();
            self.body_pointer.push(body_cursor);
            let rot = dev.rotation();
            let rot = to_gl_rot(Quaternion::from_xyzw(rot.i, rot.j, rot.k, rot.w));
            dev.into_iter().for_each(|tr| {
                let pos = tr.position();
                let pos = to_gl_pos(Vector3 {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                });
                self.positions.push(pos.extend(0.));
                self.rotations.push(rot);
                self.states.push(TransState {
                    amp: 0.0,
                    phase: 0.0,
                    enable: 1.0,
                    alpha: 1.0,
                });
            });
        });
    }

    pub fn update_geometry(&mut self, geometry: Geometry) {
        let mut cursor = 0;
        geometry.into_iter().for_each(|dev| {
            let rot = to_gl_rot(Quaternion::from_xyzw(
                dev.rotation().i,
                dev.rotation().j,
                dev.rotation().k,
                dev.rotation().w,
            ));
            dev.into_iter().for_each(|tr| {
                let pos = tr.position();
                let pos = to_gl_pos(Vector3 {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                });
                self.positions[cursor] = pos.extend(0.);
                self.rotations[cursor] = rot;
                cursor += 1;
            });
        });
    }
}
