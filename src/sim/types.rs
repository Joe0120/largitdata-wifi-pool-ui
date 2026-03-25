use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimDevice {
    pub device_id: String,
    pub card: Vec<SimCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimCard {
    pub no: serde_json::Value,
    pub sim_no: serde_json::Value,
    pub phone_number: String,
    pub app_lable: String,
    pub sim_number: serde_json::Value,
    pub app_order: serde_json::Value,
}
