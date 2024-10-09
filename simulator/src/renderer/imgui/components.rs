use std::ffi::CString;

use imgui::sys::{igDragFloat, igDragFloat2};

pub fn drag_float(label: impl Into<Vec<u8>>, v: &mut f32, speed: f32, min: f32, max: f32) -> bool {
    unsafe {
        igDragFloat(
            CString::new(label).unwrap().as_c_str().as_ptr(),
            v as _,
            speed,
            min,
            max,
            CString::new("%.3f").unwrap().as_c_str().as_ptr(),
            0,
        )
    }
}

pub fn drag_float2(
    label: impl Into<Vec<u8>>,
    v: &mut [f32],
    speed: f32,
    min: f32,
    max: f32,
) -> bool {
    unsafe {
        igDragFloat2(
            CString::new(label).unwrap().as_c_str().as_ptr(),
            v.as_mut_ptr(),
            speed,
            min,
            max,
            CString::new("%.0f").unwrap().as_c_str().as_ptr(),
            0,
        )
    }
}
