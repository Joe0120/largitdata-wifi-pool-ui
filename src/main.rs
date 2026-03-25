mod adb;
mod api;
mod config;
mod error;
mod scrcpy;
mod screenshot_cache;
mod sim;

use std::net::SocketAddr;

use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

use adb::client::AdbClient;
use config::Config;
use scrcpy::session_manager::SessionManager;
use screenshot_cache::ScreenshotCache;
use sim::manager::SimManager;

#[derive(Clone)]
pub struct AppState {
    pub adb: AdbClient,
    pub scrcpy: SessionManager,
    pub sim: SimManager,
    pub screenshots: ScreenshotCache,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env();
    let port = config.port;
    let adb = AdbClient::new(config.adb_path);
    let screenshots = ScreenshotCache::new(adb.clone());

    // Start background screenshot polling
    screenshots.clone().start_polling();

    // Setup adb reverse on all devices so they can reach this server
    setup_adb_reverse(adb.clone(), port);

    let state = AppState {
        scrcpy: SessionManager::new(adb.clone()),
        adb,
        sim: SimManager::new(config.python_path, config.scripts_dir, config.device_phones_path),
        screenshots,
    };

    let app = api::router()
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Background task: set `adb reverse tcp:{port} tcp:{port}` on all devices.
/// Re-checks every 30s to catch newly connected devices.
fn setup_adb_reverse(adb: AdbClient, port: u16) {
    let local = format!("tcp:{port}");
    let remote = format!("tcp:{port}");
    tokio::spawn(async move {
        let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
        loop {
            if let Ok(devices) = adb.list_devices().await {
                for dev in &devices {
                    if known.contains(&dev.serial) {
                        continue;
                    }
                    let args = ["-s", &dev.serial, "reverse", &local, &remote];
                    match adb.run_raw(&args).await {
                        Ok(_) => {
                            tracing::info!("adb reverse set for {} → tcp:{}", dev.serial, port);
                            known.insert(dev.serial.clone());
                        }
                        Err(e) => {
                            tracing::warn!("adb reverse failed for {}: {e}", dev.serial);
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}
