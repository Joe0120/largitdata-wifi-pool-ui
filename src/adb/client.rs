use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::{RwLock, Semaphore};

use crate::adb::types::{DeviceInfo, WindowSize};
use crate::error::AppError;

const ADB_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONCURRENT: usize = 50;
const ATX_AGENT_PORT: u16 = 9008;

/// Port counter for atx-agent forwards
static NEXT_ATX_PORT: AtomicU16 = AtomicU16::new(17100);

fn next_atx_port() -> u16 {
    let port = NEXT_ATX_PORT.fetch_add(1, Ordering::Relaxed);
    if port > 18000 {
        NEXT_ATX_PORT.store(17100, Ordering::Relaxed);
    }
    port
}

#[derive(Clone)]
pub struct AdbClient {
    adb_path: String,
    semaphore: Arc<Semaphore>,
    http: reqwest::Client,
    /// serial -> local forwarded port for atx-agent
    atx_ports: Arc<RwLock<HashMap<String, u16>>>,
}

impl AdbClient {
    pub fn new(adb_path: String) -> Self {
        Self {
            adb_path,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT)),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            atx_ports: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn adb_path(&self) -> &str {
        &self.adb_path
    }

    /// Get or create a forwarded port to atx-agent on the device
    async fn get_atx_port(&self, serial: &str) -> Result<u16, AppError> {
        // Check existing
        {
            let ports = self.atx_ports.read().await;
            if let Some(&port) = ports.get(serial) {
                return Ok(port);
            }
        }
        // Create new forward
        let port = next_atx_port();
        let local = format!("tcp:{port}");
        let remote = format!("tcp:{ATX_AGENT_PORT}");
        self.run(&["-s", serial, "forward", &local, &remote]).await?;
        {
            let mut ports = self.atx_ports.write().await;
            ports.insert(serial.to_string(), port);
        }
        Ok(port)
    }

    async fn run(&self, args: &[&str]) -> Result<Vec<u8>, AppError> {
        let _permit = self.semaphore.acquire().await.map_err(|e| AppError::Adb(e.to_string()))?;

        let result = tokio::time::timeout(ADB_TIMEOUT, async {
            let output = Command::new(&self.adb_path)
                .args(args)
                .output()
                .await?;

            if output.status.success() {
                Ok(output.stdout)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(AppError::Adb(format!(
                    "adb {} failed: {}",
                    args.join(" "),
                    stderr.trim()
                )))
            }
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(_) => Err(AppError::Adb(format!(
                "adb {} timed out after {}s",
                args.join(" "),
                ADB_TIMEOUT.as_secs()
            ))),
        }
    }

    async fn run_text(&self, args: &[&str]) -> Result<String, AppError> {
        let bytes = self.run(args).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub async fn list_devices(&self) -> Result<Vec<DeviceInfo>, AppError> {
        let output = self.run_text(&["devices", "-l"]).await?;
        let mut devices = Vec::new();

        for line in output.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let serial = parts[0].to_string();
            let status = parts[1].to_string();

            // Skip unauthorized/offline devices
            if status != "device" {
                continue;
            }

            let mut model = None;
            let mut product = None;

            for part in &parts[2..] {
                if let Some(val) = part.strip_prefix("model:") {
                    model = Some(val.to_string());
                } else if let Some(val) = part.strip_prefix("product:") {
                    product = Some(val.to_string());
                }
            }

            devices.push(DeviceInfo {
                serial,
                model,
                product,
                status,
            });
        }

        Ok(devices)
    }

    pub async fn screenshot(&self, serial: &str) -> Result<Vec<u8>, AppError> {
        // Try atx-agent first (fast, returns JPEG directly)
        match self.screenshot_atx(serial).await {
            Ok(jpeg) => return Ok(jpeg),
            Err(e) => {
                tracing::debug!("atx-agent screenshot failed for {serial}, falling back to screencap: {e}");
            }
        }
        // Fallback to adb screencap (slower, returns PNG)
        self.run(&["-s", serial, "exec-out", "screencap", "-p"]).await
    }

    async fn screenshot_atx(&self, serial: &str) -> Result<Vec<u8>, AppError> {
        let port = self.get_atx_port(serial).await?;
        let url = format!("http://127.0.0.1:{port}/jsonrpc/0");

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "takeScreenshot",
            "params": [1, 80]  // scale=1, quality=80
        });

        let resp = self.http.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Adb(format!("atx-agent request failed: {e}")))?;

        let data: serde_json::Value = resp.json().await
            .map_err(|e| AppError::Adb(format!("atx-agent response parse failed: {e}")))?;

        let base64_str = data["result"]
            .as_str()
            .ok_or_else(|| AppError::Adb("atx-agent returned no screenshot data".into()))?;

        // atx-agent returns base64 with newlines — strip them before decoding
        let cleaned: String = base64_str.chars().filter(|c| !c.is_whitespace()).collect();
        use base64::Engine;
        let jpeg = base64::engine::general_purpose::STANDARD
            .decode(&cleaned)
            .map_err(|e| AppError::Adb(format!("base64 decode failed: {e}")))?;

        Ok(jpeg)
    }

    pub async fn tap(&self, serial: &str, x: f64, y: f64) -> Result<(), AppError> {
        self.run(&[
            "-s", serial, "shell", "input", "tap",
            &format!("{}", x as i32),
            &format!("{}", y as i32),
        ]).await?;
        Ok(())
    }

    pub async fn swipe(
        &self,
        serial: &str,
        x1: f64, y1: f64,
        x2: f64, y2: f64,
        duration_ms: u64,
    ) -> Result<(), AppError> {
        self.run(&[
            "-s", serial, "shell", "input", "swipe",
            &format!("{}", x1 as i32),
            &format!("{}", y1 as i32),
            &format!("{}", x2 as i32),
            &format!("{}", y2 as i32),
            &format!("{}", duration_ms),
        ]).await?;
        Ok(())
    }

    pub async fn key_event(&self, serial: &str, keycode: u32) -> Result<(), AppError> {
        self.run(&[
            "-s", serial, "shell", "input", "keyevent",
            &keycode.to_string(),
        ]).await?;
        Ok(())
    }

    pub async fn input_text(&self, serial: &str, text: &str) -> Result<(), AppError> {
        // Escape special characters for adb shell input text
        let escaped = text
            .replace(' ', "%s")
            .replace('\'', "\\'")
            .replace('"', "\\\"")
            .replace('&', "\\&")
            .replace('<', "\\<")
            .replace('>', "\\>")
            .replace('|', "\\|")
            .replace(';', "\\;")
            .replace('(', "\\(")
            .replace(')', "\\)");

        self.run(&["-s", serial, "shell", "input", "text", &escaped]).await?;
        Ok(())
    }

    pub async fn shell(&self, serial: &str, command: &str) -> Result<String, AppError> {
        self.run_text(&["-s", serial, "shell", command]).await
    }

    pub async fn window_size(&self, serial: &str) -> Result<WindowSize, AppError> {
        let output = self.shell(serial, "wm size").await?;
        // Parse "Physical size: 1080x1920"
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("Physical size: ") {
                let parts: Vec<&str> = rest.trim().split('x').collect();
                if parts.len() == 2 {
                    if let (Ok(w), Ok(h)) = (parts[0].parse(), parts[1].parse()) {
                        return Ok(WindowSize { width: w, height: h });
                    }
                }
            }
        }
        Err(AppError::Adb(format!("Failed to parse window size: {output}")))
    }

    pub async fn force_portrait(&self, serial: &str) -> Result<(), AppError> {
        self.shell(serial, "settings put system accelerometer_rotation 0").await?;
        self.shell(serial, "settings put system user_rotation 0").await?;
        Ok(())
    }

    pub async fn push_file(&self, serial: &str, local: &str, remote: &str) -> Result<(), AppError> {
        self.run(&["-s", serial, "push", local, remote]).await?;
        Ok(())
    }

    pub async fn forward(&self, serial: &str, local: &str, remote: &str) -> Result<(), AppError> {
        self.run(&["-s", serial, "forward", local, remote]).await?;
        Ok(())
    }

    pub async fn remove_forward(&self, serial: &str, local: &str) -> Result<(), AppError> {
        self.run(&["-s", serial, "forward", "--remove", local]).await?;
        Ok(())
    }
}
