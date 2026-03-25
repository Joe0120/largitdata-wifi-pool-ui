use rusqlite::params;
use serde::Serialize;

use crate::db::Database;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct DeviceRow {
    pub device_id: String,
    pub model: Option<String>,
    pub product: Option<String>,
    pub current_phone: Option<String>,
    pub current_app_order: Option<i32>,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SimCardRow {
    pub id: i64,
    pub device_id: String,
    pub app_order: i32,
    pub phone_number: Option<String>,
    pub app_lable: Option<String>,
    pub no: Option<String>,
    pub sim_no: Option<String>,
    pub sim_number: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeviceDetail {
    #[serde(flatten)]
    pub device: DeviceRow,
    pub sim_cards: Vec<SimCardRow>,
}

#[derive(Debug, Serialize)]
pub struct PhoneStatus {
    pub phone_number: String,
    pub device_id: String,
    pub app_order: i32,
    pub is_active: bool,
    pub current_phone: Option<String>,
}

impl Database {
    /// List all devices with current status
    pub async fn list_devices(&self) -> Result<Vec<DeviceRow>, AppError> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT device_id, model, product, current_phone, current_app_order, last_checked_at FROM devices ORDER BY device_id")
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(DeviceRow {
                    device_id: row.get(0)?,
                    model: row.get(1)?,
                    product: row.get(2)?,
                    current_phone: row.get(3)?,
                    current_app_order: row.get(4)?,
                    last_checked_at: row.get(5)?,
                })
            })
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Adb(format!("DB row error: {e}")))?;

        Ok(rows)
    }

    /// Get single device with all sim cards
    pub async fn get_device(&self, device_id: &str) -> Result<Option<DeviceDetail>, AppError> {
        let conn = self.conn.lock().await;

        let device = conn
            .query_row(
                "SELECT device_id, model, product, current_phone, current_app_order, last_checked_at FROM devices WHERE device_id = ?1",
                params![device_id],
                |row| {
                    Ok(DeviceRow {
                        device_id: row.get(0)?,
                        model: row.get(1)?,
                        product: row.get(2)?,
                        current_phone: row.get(3)?,
                        current_app_order: row.get(4)?,
                        last_checked_at: row.get(5)?,
                    })
                },
            )
            .ok();

        let device = match device {
            Some(d) => d,
            None => return Ok(None),
        };

        let mut stmt = conn
            .prepare("SELECT id, device_id, app_order, phone_number, app_lable, no, sim_no, sim_number FROM sim_cards WHERE device_id = ?1 ORDER BY app_order")
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?;

        let sim_cards = stmt
            .query_map(params![device_id], |row| {
                Ok(SimCardRow {
                    id: row.get(0)?,
                    device_id: row.get(1)?,
                    app_order: row.get(2)?,
                    phone_number: row.get(3)?,
                    app_lable: row.get(4)?,
                    no: row.get(5)?,
                    sim_no: row.get(6)?,
                    sim_number: row.get(7)?,
                })
            })
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Adb(format!("DB row error: {e}")))?;

        Ok(Some(DeviceDetail { device, sim_cards }))
    }

    /// Update device current phone after switch or sync
    pub async fn update_device_current(
        &self,
        device_id: &str,
        phone: &str,
        app_order: Option<i32>,
    ) -> Result<(), AppError> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE devices SET current_phone = ?1, current_app_order = ?2, last_checked_at = datetime('now') WHERE device_id = ?3",
            params![phone, app_order, device_id],
        )
        .map_err(|e| AppError::Adb(format!("DB update failed: {e}")))?;
        Ok(())
    }

    /// Lookup a phone number — which device, is it active?
    pub async fn get_phone_status(&self, phone: &str) -> Result<Option<PhoneStatus>, AppError> {
        let conn = self.conn.lock().await;

        let result = conn
            .query_row(
                "SELECT sc.phone_number, sc.device_id, sc.app_order, d.current_phone
                 FROM sim_cards sc
                 JOIN devices d ON d.device_id = sc.device_id
                 WHERE sc.phone_number = ?1",
                params![phone],
                |row| {
                    let phone_number: String = row.get(0)?;
                    let device_id: String = row.get(1)?;
                    let app_order: i32 = row.get(2)?;
                    let current_phone: Option<String> = row.get(3)?;
                    let is_active = current_phone.as_deref() == Some(phone_number.as_str());
                    Ok(PhoneStatus {
                        phone_number,
                        device_id,
                        app_order,
                        is_active,
                        current_phone,
                    })
                },
            )
            .ok();

        Ok(result)
    }

    /// Import from device_phones.json into DB
    pub async fn import_from_json(&self, json_path: &str) -> Result<usize, AppError> {
        let content = tokio::fs::read_to_string(json_path)
            .await
            .map_err(|e| AppError::Adb(format!("Failed to read JSON: {e}")))?;

        let devices: Vec<serde_json::Value> = serde_json::from_str(&content)
            .map_err(|e| AppError::Adb(format!("Failed to parse JSON: {e}")))?;

        let conn = self.conn.lock().await;
        let mut count = 0;

        for dev in &devices {
            let device_id = dev["device_id"].as_str().unwrap_or_default();
            if device_id.is_empty() {
                continue;
            }

            // Upsert device
            conn.execute(
                "INSERT INTO devices (device_id) VALUES (?1) ON CONFLICT(device_id) DO NOTHING",
                params![device_id],
            )
            .map_err(|e| AppError::Adb(format!("DB insert device failed: {e}")))?;

            // Upsert sim cards
            if let Some(cards) = dev["card"].as_array() {
                for card in cards {
                    let app_order = card["app_order"].as_i64().unwrap_or(0) as i32;
                    let phone_number = card["phone_number"].as_str().unwrap_or_default();
                    let app_lable = card["app_lable"].as_str().unwrap_or_default();
                    let sim_number = card["sim_number"].as_str().unwrap_or_default();
                    let no_str = if let Some(s) = card["no"].as_str() { s.to_string() } else if let Some(n) = card["no"].as_i64() { n.to_string() } else { String::new() };
                    let sim_no_str = if let Some(s) = card["sim_no"].as_str() { s.to_string() } else if let Some(n) = card["sim_no"].as_i64() { n.to_string() } else { String::new() };

                    conn.execute(
                        "INSERT INTO sim_cards (device_id, app_order, phone_number, app_lable, no, sim_no, sim_number)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                         ON CONFLICT(device_id, app_order) DO UPDATE SET
                           phone_number = excluded.phone_number,
                           app_lable = excluded.app_lable,
                           no = excluded.no,
                           sim_no = excluded.sim_no,
                           sim_number = excluded.sim_number",
                        params![device_id, app_order, phone_number, app_lable, no_str, sim_no_str, sim_number],
                    )
                    .map_err(|e| AppError::Adb(format!("DB insert sim_card failed: {e}")))?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// List all devices with their SIM cards (same format as device_phones.json)
    pub async fn list_sim_devices(&self) -> Result<Vec<serde_json::Value>, AppError> {
        let conn = self.conn.lock().await;

        // Get all device_ids that have sim cards
        let mut device_stmt = conn
            .prepare("SELECT DISTINCT device_id FROM sim_cards ORDER BY device_id")
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?;
        let device_ids: Vec<String> = device_stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Adb(format!("DB row error: {e}")))?;

        let mut card_stmt = conn
            .prepare("SELECT app_order, phone_number, app_lable, no, sim_no, sim_number FROM sim_cards WHERE device_id = ?1 ORDER BY app_order")
            .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?;

        let mut result = Vec::new();
        for did in &device_ids {
            let cards: Vec<serde_json::Value> = card_stmt
                .query_map(params![did], |row| {
                    Ok(serde_json::json!({
                        "app_order": row.get::<_, i32>(0)?,
                        "phone_number": row.get::<_, String>(1).unwrap_or_default(),
                        "app_lable": row.get::<_, String>(2).unwrap_or_default(),
                        "no": row.get::<_, String>(3).unwrap_or_default(),
                        "sim_no": row.get::<_, String>(4).unwrap_or_default(),
                        "sim_number": row.get::<_, String>(5).unwrap_or_default(),
                    }))
                })
                .map_err(|e| AppError::Adb(format!("DB query failed: {e}")))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::Adb(format!("DB row error: {e}")))?;

            result.push(serde_json::json!({
                "device_id": did,
                "card": cards,
            }));
        }

        Ok(result)
    }
}
