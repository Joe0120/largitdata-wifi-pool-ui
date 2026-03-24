use std::env;

pub struct Config {
    pub port: u16,
    pub adb_path: String,
    pub python_path: String,
    pub scripts_dir: String,
    pub device_phones_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5678),
            adb_path: env::var("ADB_PATH").unwrap_or_else(|_| "adb".into()),
            python_path: env::var("PYTHON_PATH").unwrap_or_else(|_| "python3".into()),
            scripts_dir: env::var("SCRIPTS_DIR").unwrap_or_else(|_| ".".into()),
            device_phones_path: env::var("DEVICE_JSON")
                .unwrap_or_else(|_| "device_phones.json".into()),
        }
    }
}
