mod adb;
mod api;
mod config;
mod db;
mod error;
mod scrcpy;
mod screenshot_cache;
mod sim;

use std::net::SocketAddr;

use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

use adb::client::AdbClient;
use config::Config;
use db::Database;
use scrcpy::session_manager::SessionManager;
use screenshot_cache::ScreenshotCache;
use sim::manager::SimManager;

#[derive(Clone)]
pub struct AppState {
    pub adb: AdbClient,
    pub scrcpy: SessionManager,
    pub sim: SimManager,
    pub screenshots: ScreenshotCache,
    pub db: Database,
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

    // Open database
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "data.db".to_string());
    let db = Database::open(&db_path).await.expect("Failed to open database");

    // Import from JSON on first run (if sim_cards table is empty)
    {
        let conn = db.conn().lock().await;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sim_cards", [], |row| row.get(0))
            .unwrap_or(0);
        drop(conn);
        if count == 0 {
            tracing::info!("Importing device_phones.json into database...");
            match db.import_from_json(&config.device_phones_path).await {
                Ok(n) => tracing::info!("Imported {n} SIM cards into database"),
                Err(e) => tracing::warn!("Failed to import JSON: {e}"),
            }
        }
    }

    // Start background screenshot polling
    screenshots.clone().start_polling();

    // Setup adb reverse on all devices
    setup_adb_reverse(adb.clone(), port);

    let state = AppState {
        scrcpy: SessionManager::new(adb.clone()),
        adb,
        sim: SimManager::new(config.python_path, config.scripts_dir, config.device_phones_path),
        screenshots,
        db,
    };

    let app = api::router()
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting server on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

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
