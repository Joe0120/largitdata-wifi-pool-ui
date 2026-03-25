use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct SmsRow {
    pub id: i64,
    pub device_id: Option<String>,
    pub phone_number: Option<String>,
    pub sender: Option<String>,
    pub body: Option<String>,
    pub received_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct NewSms {
    pub device_id: Option<String>,
    pub phone_number: Option<String>,
    pub sender: Option<String>,
    pub body: Option<String>,
    pub received_at: Option<String>,
    #[serde(default)]
    pub raw_body: Option<String>,
}

impl Database {
    /// Insert a new SMS
    pub async fn insert_sms(&self, sms: &NewSms) -> Result<i64, AppError> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO sms_messages (device_id, phone_number, sender, body, raw_body, received_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![sms.device_id, sms.phone_number, sms.sender, sms.body, sms.raw_body, sms.received_at],
        )
        .map_err(|e| AppError::Adb(format!("DB insert sms failed: {e}")))?;
        Ok(conn.last_insert_rowid())
    }

    /// Get SMS by phone number, newest first
    pub async fn get_sms_by_phone(
        &self,
        phone: &str,
        limit: u32,
    ) -> Result<Vec<SmsRow>, AppError> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, device_id, phone_number, sender, body, received_at, created_at
                 FROM sms_messages
                 WHERE phone_number = ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?;

        let rows = stmt
            .query_map(params![phone, limit], |row| {
                Ok(SmsRow {
                    id: row.get(0)?,
                    device_id: row.get(1)?,
                    phone_number: row.get(2)?,
                    sender: row.get(3)?,
                    body: row.get(4)?,
                    received_at: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Adb(format!("DB row error: {e}")))?;

        Ok(rows)
    }
}
