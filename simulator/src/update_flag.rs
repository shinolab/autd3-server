bitflags::bitflags! {
    pub struct UpdateFlag: u32 {
        const UPDATE_SOURCE_DRIVE = 1 << 0;
        const UPDATE_COLOR_MAP = 1 << 1;
        const UPDATE_SLICE_POS = 1 << 3;
        const UPDATE_SLICE_SIZE = 1 << 4;
        const UPDATE_SOURCE_ALPHA = 1 << 5;
        const UPDATE_SOURCE_FLAG = 1 << 6;
        const SAVE_IMAGE = 1 << 7;
        const UPDATE_DEVICE_INFO = 1 << 8;
    }
}
