/// scrcpy v2.x binary control protocol

/// Build a touch event packet (32 bytes)
///
/// action: 0=down, 1=up, 2=move
pub fn build_touch_event(action: u8, x: u32, y: u32, width: u16, height: u16) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[0] = 2; // SC_CONTROL_MSG_TYPE_INJECT_TOUCH_EVENT
    buf[1] = action;
    // pointer_id: u64 BE = 1
    buf[2..10].copy_from_slice(&1u64.to_be_bytes());
    // x: u32 BE
    buf[10..14].copy_from_slice(&x.to_be_bytes());
    // y: u32 BE
    buf[14..18].copy_from_slice(&y.to_be_bytes());
    // width: u16 BE
    buf[18..20].copy_from_slice(&width.to_be_bytes());
    // height: u16 BE
    buf[20..22].copy_from_slice(&height.to_be_bytes());
    // pressure: u16 BE = 1
    buf[22..24].copy_from_slice(&1u16.to_be_bytes());
    // action_button: u32 BE = 1 (AMOTION_EVENT_BUTTON_PRIMARY)
    buf[24..28].copy_from_slice(&1u32.to_be_bytes());
    // buttons: u32 BE = 1
    buf[28..32].copy_from_slice(&1u32.to_be_bytes());
    buf
}

/// Build a key event packet (14 bytes)
///
/// action: 0=down, 1=up
pub fn build_key_event(action: u8, keycode: u32) -> [u8; 14] {
    let mut buf = [0u8; 14];
    buf[0] = 0; // SC_CONTROL_MSG_TYPE_INJECT_KEYCODE
    buf[1] = action;
    // keycode: u32 BE
    buf[2..6].copy_from_slice(&keycode.to_be_bytes());
    // repeat: u32 BE = 0
    buf[6..10].copy_from_slice(&0u32.to_be_bytes());
    // metastate: u32 BE = 0
    buf[10..14].copy_from_slice(&0u32.to_be_bytes());
    buf
}
