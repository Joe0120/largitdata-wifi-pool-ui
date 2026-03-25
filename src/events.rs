use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum Event {
    Sms(SmsPayload),
}

#[derive(Clone, Debug, Serialize)]
pub struct SmsPayload {
    pub id: i64,
    pub device_id: String,
    pub phone_number: Option<String>,
    pub sender: Option<String>,
    pub body: Option<String>,
    pub received_at: Option<String>,
}
