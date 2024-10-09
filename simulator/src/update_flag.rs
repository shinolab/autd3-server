bitflags::bitflags! {
    pub struct UpdateFlag: u32 {
        const UPDATE_SLICE_COLOR_MAP = 1 << 0;
        const UPDATE_SLICE_POS = 1 << 1;
        const UPDATE_SLICE_SIZE = 1 << 2;

        const UPDATE_CAMERA = 1 << 3;

        const UPDATE_TRANS_STATE = 1 << 4;
        const UPDATE_TRANS_ALPHA = 1 << 5;
        const UPDATE_TRANS_POS = 1 << 6;

        const UPDATE_CONFIG = 1 << 7;

        const UPDATE_DEVICE_INFO = 1 << 9;
    }
}
