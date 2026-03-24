use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::adb::client::AdbClient;
use crate::error::AppError;

/// Embedded scrcpy server JAR
const SCRCPY_JAR: &[u8] = include_bytes!("../../assets/scrcpy-server-v2.7.jar");

/// Port counter for ADB forward (each session gets a unique local port)
static NEXT_PORT: AtomicU16 = AtomicU16::new(27183);

fn next_port() -> u16 {
    let port = NEXT_PORT.fetch_add(1, Ordering::Relaxed);
    if port > 30000 {
        NEXT_PORT.store(27183, Ordering::Relaxed);
    }
    port
}

pub struct ScrcpySession {
    pub serial: String,
    pub video_stream: Mutex<TcpStream>,
    pub control_stream: Mutex<TcpStream>,
    pub resolution_width: u32,
    pub resolution_height: u32,
    local_port: u16,
    adb: AdbClient,
}

impl ScrcpySession {
    pub async fn start(adb: &AdbClient, serial: &str) -> Result<Self, AppError> {
        let local_port = next_port();

        // 0. Kill any lingering scrcpy on this device
        let _ = adb.shell(serial, "pkill -f scrcpy").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // 1. Write JAR to a temp file and push to device
        let tmp_jar = format!("/tmp/scrcpy-server-{serial}.jar");
        tokio::fs::write(&tmp_jar, SCRCPY_JAR).await?;
        adb.push_file(serial, &tmp_jar, "/data/local/tmp/scrcpy_server.jar")
            .await?;
        let _ = tokio::fs::remove_file(&tmp_jar).await;

        // 2. Set up ADB forward
        let local_addr = format!("tcp:{local_port}");
        let remote_addr = "localabstract:scrcpy";
        adb.forward(serial, &local_addr, remote_addr).await?;

        // 3. Start scrcpy server on device (non-blocking — runs in background)
        let adb_path = adb.adb_path().to_string();
        let serial_owned = serial.to_string();
        tokio::spawn(async move {
            let _ = tokio::process::Command::new(&adb_path)
                .args([
                    "-s",
                    &serial_owned,
                    "shell",
                    "CLASSPATH=/data/local/tmp/scrcpy_server.jar",
                    "app_process",
                    "/",
                    "com.genymobile.scrcpy.Server",
                    "2.7",
                    "log_level=info",
                    "max_size=1024",
                    "max_fps=30",
                    "video_bit_rate=8000000",
                    "tunnel_forward=true",
                    "send_frame_meta=false",
                    "control=true",
                    "audio=false",
                    "show_touches=false",
                    "stay_awake=false",
                    "power_off_on_close=false",
                    "clipboard_autosync=false",
                ])
                .output()
                .await;
        });

        // 4. Wait for scrcpy to start listening
        tokio::time::sleep(Duration::from_secs(2)).await;

        // 5. Connect TWO sockets to scrcpy:
        //    - First connect = video socket
        //    - Second connect = control socket
        //    scrcpy waits for both before sending handshake on video
        let connect_addr = format!("127.0.0.1:{local_port}");

        tracing::debug!("Connecting video socket for {serial} on port {local_port}...");
        let mut video_stream = Self::connect_with_retry(&connect_addr, 50).await.map_err(
            |e| AppError::Adb(format!("Failed to connect video socket for {serial}: {e}")),
        )?;

        // Small delay before second connection
        tokio::time::sleep(Duration::from_millis(100)).await;

        tracing::debug!("Connecting control socket for {serial}...");
        let control_stream = Self::connect_with_retry(&connect_addr, 20).await.map_err(
            |e| AppError::Adb(format!("Failed to connect control socket for {serial}: {e}")),
        )?;

        // 5. Parse handshake from video socket (sent after both sockets connect)
        let (resolution_width, resolution_height) =
            Self::parse_handshake(&mut video_stream).await?;

        tracing::info!(
            "scrcpy session started for {serial}: {resolution_width}x{resolution_height} on port {local_port}"
        );

        Ok(Self {
            serial: serial.to_string(),
            video_stream: Mutex::new(video_stream),
            control_stream: Mutex::new(control_stream),
            resolution_width,
            resolution_height,
            local_port,
            adb: adb.clone(),
        })
    }

    async fn connect_with_retry(addr: &str, max_retries: u32) -> Result<TcpStream, String> {
        for i in 0..max_retries {
            match TcpStream::connect(addr).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    if i == max_retries - 1 {
                        return Err(e.to_string());
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
        Err("max retries reached".into())
    }

    async fn parse_handshake(stream: &mut TcpStream) -> Result<(u32, u32), AppError> {
        let mut buf = [0u8; 77];

        // Read exactly 77 bytes with timeout
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            stream.read_exact(&mut buf).await
        })
        .await;

        match result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => return Err(AppError::Adb(format!("Handshake read error: {e}"))),
            Err(_) => return Err(AppError::Adb("Handshake timed out".into())),
        }

        // Verify dummy byte
        if buf[0] != 0x00 {
            return Err(AppError::Adb(format!(
                "Invalid dummy byte: {:#04x}",
                buf[0]
            )));
        }

        let name = String::from_utf8_lossy(&buf[1..65])
            .trim_end_matches('\0')
            .to_string();
        tracing::debug!("scrcpy device name: {name}");

        // Parse resolution from bytes 69..77 (after 1+64+4)
        let width = u32::from_be_bytes([buf[69], buf[70], buf[71], buf[72]]);
        let height = u32::from_be_bytes([buf[73], buf[74], buf[75], buf[76]]);

        Ok((width, height))
    }

    pub async fn shutdown(&self) {
        // Kill scrcpy on device
        let _ = self.adb.shell(&self.serial, "pkill -f scrcpy").await;
        // Remove ADB forward
        let local_addr = format!("tcp:{}", self.local_port);
        let _ = self.adb.remove_forward(&self.serial, &local_addr).await;
        tracing::info!("scrcpy session closed for {}", self.serial);
    }
}

impl Drop for ScrcpySession {
    fn drop(&mut self) {
        tracing::debug!("ScrcpySession dropped for {}", self.serial);
    }
}
