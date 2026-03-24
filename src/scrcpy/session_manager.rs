use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{RwLock, Semaphore};

use crate::adb::client::AdbClient;
use crate::error::AppError;
use crate::scrcpy::server::ScrcpySession;

/// Max concurrent scrcpy startups
const MAX_CONCURRENT_STARTUPS: usize = 3;

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<ScrcpySession>>>>,
    startup_semaphore: Arc<Semaphore>,
    adb: AdbClient,
}

impl SessionManager {
    pub fn new(adb: AdbClient) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            startup_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_STARTUPS)),
            adb,
        }
    }

    pub async fn get_or_create(&self, serial: &str) -> Result<Arc<ScrcpySession>, AppError> {
        // Always create a fresh session — scrcpy only sends SPS/PPS once at the start,
        // so reusing a session means new clients never get the decoder config.
        self.remove(serial).await;

        // Limit concurrent startups to avoid overwhelming ADB
        let _permit = self.startup_semaphore.acquire().await
            .map_err(|e| AppError::Adb(e.to_string()))?;

        tracing::info!("Starting scrcpy session for {serial}");
        let session = ScrcpySession::start(&self.adb, serial).await?;
        let session = Arc::new(session);

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(serial.to_string(), session.clone());
        }

        Ok(session)
    }

    pub async fn remove(&self, serial: &str) {
        let session = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(serial)
        };
        if let Some(session) = session {
            session.shutdown().await;
        }
    }
}
