use std::path::PathBuf;

use tokio::process::Command;

use crate::error::AppError;
use crate::sim::types::SimDevice;

#[derive(Clone)]
pub struct SimManager {
    python_path: String,
    scripts_dir: PathBuf,
    device_phones_path: PathBuf,
}

impl SimManager {
    pub fn new(python_path: String, scripts_dir: String, device_phones_path: String) -> Self {
        Self {
            python_path,
            scripts_dir: PathBuf::from(scripts_dir),
            device_phones_path: PathBuf::from(device_phones_path),
        }
    }

    pub async fn load_devices(&self) -> Result<Vec<SimDevice>, AppError> {
        let content = tokio::fs::read_to_string(&self.device_phones_path)
            .await
            .map_err(|e| AppError::Sim(format!("Failed to read device_phones.json: {e}")))?;
        let devices: Vec<SimDevice> = serde_json::from_str(&content)
            .map_err(|e| AppError::Sim(format!("Failed to parse device_phones.json: {e}")))?;
        Ok(devices)
    }

    pub async fn get_current(&self) -> Result<String, AppError> {
        let script = self.scripts_dir.join("switch_all_devices.py");
        let output = Command::new(&self.python_path)
            .arg(&script)
            .arg("--current")
            .current_dir(&self.scripts_dir)
            .output()
            .await
            .map_err(|e| AppError::Sim(format!("Failed to run script: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(AppError::Sim(format!("Script failed: {stderr}")));
        }

        Ok(stdout)
    }

    pub async fn switch_all(&self, sim_order: u32) -> Result<String, AppError> {
        let script = self.scripts_dir.join("switch_all_devices.py");
        let output = Command::new(&self.python_path)
            .arg(&script)
            .arg(sim_order.to_string())
            .current_dir(&self.scripts_dir)
            .output()
            .await
            .map_err(|e| AppError::Sim(format!("Failed to run script: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(AppError::Sim(format!("Script failed: {stderr}")));
        }

        Ok(stdout)
    }

    pub async fn switch_device(&self, device_id: &str, sim_order: u32) -> Result<String, AppError> {
        let script = self.scripts_dir.join("switch_phone_number.py");
        let output = Command::new(&self.python_path)
            .arg(&script)
            .arg(device_id)
            .arg("--index")
            .arg(sim_order.to_string())
            .current_dir(&self.scripts_dir)
            .output()
            .await
            .map_err(|e| AppError::Sim(format!("Failed to run script: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(AppError::Sim(format!("Script failed: {stderr}")));
        }

        Ok(stdout)
    }
}
