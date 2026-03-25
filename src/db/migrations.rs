use crate::db::Database;
use crate::error::AppError;

pub async fn run(db: &Database) -> Result<(), AppError> {
    let conn = db.conn().lock().await;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS devices (
            serial          TEXT PRIMARY KEY,
            model           TEXT,
            product         TEXT,
            current_phone   TEXT,
            current_app_order INTEGER,
            last_checked_at TEXT,
            created_at      TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS sim_cards (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            device_serial   TEXT NOT NULL,
            app_order       INTEGER NOT NULL,
            phone_number    TEXT,
            app_lable       TEXT,
            no              TEXT,
            sim_no          TEXT,
            sim_number      TEXT,
            UNIQUE(device_serial, app_order)
        );

        CREATE TABLE IF NOT EXISTS sms_messages (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            device_serial   TEXT NOT NULL,
            phone_number    TEXT,
            sender          TEXT,
            body            TEXT,
            received_at     TEXT,
            created_at      TEXT DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_sms_phone ON sms_messages(phone_number);
        CREATE INDEX IF NOT EXISTS idx_sms_device ON sms_messages(device_serial);
        CREATE INDEX IF NOT EXISTS idx_sim_device ON sim_cards(device_serial);
        ",
    )
    .map_err(|e| AppError::Adb(format!("Migration failed: {e}")))?;

    Ok(())
}
