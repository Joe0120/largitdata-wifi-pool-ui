use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::adb::client::AdbClient;

struct CachedScreenshot {
    data: Vec<u8>,
    timestamp: Instant,
}

#[derive(Clone)]
pub struct ScreenshotCache {
    cache: Arc<RwLock<HashMap<String, CachedScreenshot>>>,
    adb: AdbClient,
    /// Set to true when any client requests a screenshot, resets each cycle
    active: Arc<AtomicBool>,
}

impl ScreenshotCache {
    pub fn new(adb: AdbClient) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            adb,
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get cached screenshot. Also signals that someone is watching.
    pub async fn get(&self, serial: &str) -> Option<Vec<u8>> {
        self.active.store(true, Ordering::Relaxed);
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(serial) {
            if entry.timestamp.elapsed() < Duration::from_secs(5) {
                return Some(entry.data.clone());
            }
        }
        None
    }

    pub fn start_polling(self) {
        tokio::spawn(async move {
            loop {
                // If no client has requested screenshots recently, sleep and check again
                if !self.active.swap(false, Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                let start = Instant::now();

                let devices = match self.adb.list_devices().await {
                    Ok(d) => d,
                    Err(_) => {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                };

                let mut handles = Vec::new();
                for dev in &devices {
                    let adb = self.adb.clone();
                    let serial = dev.serial.clone();
                    let cache = self.cache.clone();
                    handles.push(tokio::spawn(async move {
                        if let Ok(data) = adb.screenshot(&serial).await {
                            let mut c = cache.write().await;
                            c.insert(serial, CachedScreenshot {
                                data,
                                timestamp: Instant::now(),
                            });
                        }
                    }));
                }

                for h in handles {
                    let _ = h.await;
                }

                let elapsed = start.elapsed();
                tracing::info!("Screenshot cache refresh: {} devices in {:?}", devices.len(), elapsed);
            }
        });
    }
}
