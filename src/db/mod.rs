pub mod migrations;
pub mod devices;
pub mod sms;

use std::sync::Arc;
use tokio::sync::Mutex;
use rusqlite::Connection;

use crate::error::AppError;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub async fn open(path: &str) -> Result<Self, AppError> {
        let conn = Connection::open(path)
            .map_err(|e| AppError::Adb(format!("Failed to open database: {e}")))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| AppError::Adb(format!("Failed to set pragmas: {e}")))?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        migrations::run(&db).await?;

        Ok(db)
    }

    pub fn conn(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
    }
}
