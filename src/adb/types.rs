use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub serial: String,
    pub model: Option<String>,
    pub product: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}
