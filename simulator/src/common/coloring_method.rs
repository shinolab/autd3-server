use super::color::Color;
use super::color::Hsv;

pub type ColoringMethod = fn(f32, f32, f32) -> [f32; 4];

pub fn coloring_hsv(h: f32, v: f32, a: f32) -> [f32; 4] {
    let hsv = Hsv { h, s: 1., v, a };
    hsv.rgba()
}
