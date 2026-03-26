use crate::db::Database;
use crate::error::AppError;

pub async fn run(db: &Database) -> Result<(), AppError> {
    let conn = db.conn().lock().await;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS devices (
            device_id       TEXT PRIMARY KEY,
            model           TEXT,
            product         TEXT,
            mobile_tag      TEXT,
            current_phone   TEXT,
            current_app_order INTEGER,
            last_checked_at TEXT,
            created_at      TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS sim_cards (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id       TEXT NOT NULL,
            app_order       INTEGER NOT NULL,
            phone_number    TEXT,
            app_lable       TEXT,
            no              TEXT,
            sim_no          TEXT,
            sim_number      TEXT,
            available       INTEGER DEFAULT 1,
            UNIQUE(device_id, app_order)
        );

        CREATE TABLE IF NOT EXISTS sms_messages (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id       TEXT,
            phone_number    TEXT,
            sender          TEXT,
            body            TEXT,
            raw_body        TEXT,
            received_at     TEXT,
            created_at      TEXT DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_sms_phone ON sms_messages(phone_number);
        CREATE INDEX IF NOT EXISTS idx_sms_device ON sms_messages(device_id);
        CREATE INDEX IF NOT EXISTS idx_sim_device ON sim_cards(device_id);
        CREATE INDEX IF NOT EXISTS idx_devices_mobile_tag ON devices(mobile_tag);
        ",
    )
    .map_err(|e| AppError::Adb(format!("Migration failed: {e}")))?;

    Ok(())
}
