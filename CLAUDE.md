# CLAUDE.md — Largitdata WiFi Pool UI

## 專案概述

單一 Rust binary 的 Web 應用，用於監看/操控 38+ 台 Android 手機 + SIM 卡切換。取代原本三個獨立服務。前端是 vanilla JS，內嵌在 binary 中（rust-embed）。

## 技術棧

| 層 | 技術 |
|-----|------|
| **Backend** | Rust, axum 0.8, tokio (async runtime) |
| **Frontend** | Vanilla HTML/CSS/JS, 無框架無 build step |
| **嵌入前端** | rust-embed 8 (compile-time 嵌入 `frontend/` 目錄) |
| **ADB 通訊** | tokio::process::Command 呼叫 `adb` CLI |
| **截圖** | atx-agent (uiautomator2) jsonrpc 直接取 JPEG，fallback 到 `adb exec-out screencap -p` |
| **SIM 切換** | subprocess 呼叫 Python 腳本 (switch_phone_number.py, switch_all_devices.py) |
| **scrcpy** | 內建但目前前端未啟用（需要 WebCodecs / Secure Context） |
| **HTTP client** | reqwest (用於呼叫 atx-agent) |
| **序列化** | serde / serde_json |
| **日誌** | tracing + tracing-subscriber |
| **CORS** | tower-http CorsLayer::permissive() |

## 專案結構

```
src/
├── main.rs                    # 進入點：Config → AppState → Router → serve
├── config.rs                  # 環境變數設定 (PORT, ADB_PATH, SCRIPTS_DIR 等)
├── error.rs                   # AppError enum → axum IntoResponse (JSON error)
├── adb/
│   ├── client.rs              # AdbClient — 所有 ADB 操作的封裝
│   │                            - Semaphore(50) 並發控制
│   │                            - 10s timeout per command
│   │                            - atx-agent JPEG 截圖 (優先) + screencap fallback
│   │                            - adb forward 管理 (atx_ports cache)
│   └── types.rs               # DeviceInfo, WindowSize
├── api/
│   ├── mod.rs                 # 組裝所有 Router
│   ├── devices.rs             # /api/devices/* handlers
│   ├── sim.rs                 # /api/sim/* handlers
│   ├── stream.rs              # WS /api/devices/{serial}/stream (scrcpy)
│   └── frontend.rs            # rust-embed 靜態檔案服務 (/, /style.css, /app.js)
├── scrcpy/
│   ├── protocol.rs            # scrcpy v2 二進制觸控/按鍵封包建構
│   ├── server.rs              # ScrcpySession: push JAR, launch, 2-socket connect, handshake
│   └── session_manager.rs     # SessionManager: get_or_create, Semaphore(3)
└── sim/
    ├── manager.rs             # SimManager: 呼叫 Python 腳本
    └── types.rs               # SimDevice, SimCard (對應 device_phones.json)

frontend/
├── index.html                 # 單頁 dashboard
├── style.css                  # 深色主題
└── app.js                     # 所有前端邏輯 (~530 行)

assets/
└── scrcpy-server-v2.7.jar     # include_bytes! 內嵌進 binary
```

## 開發流程

### 環境需求
- **本機 (Mac)**：Rust 1.94+, cargo
- **遠端 (largitdata-wifi-pool / 192.168.88.191)**：Rust 1.94+, ADB, Python 3, uiautomator2, atx-agent on devices
- SSH 免密碼登入 `largitdata-wifi-pool`

### 開發循環

```bash
cd ~/project/largitdata-wifi-pool-ui

# 1. 改 code (src/ 或 frontend/)

# 2. 本機確認編譯
cargo build

# 3. 部署到遠端
./build.sh  # rsync + remote cargo build --release

# 4. 重啟服務
ssh largitdata-wifi-pool 'systemctl --user restart largitdata-wifi-pool-ui'
```

### 只改前端

前端改完也要重新 build（因為 rust-embed 在 compile-time 嵌入），不能單獨更新前端檔案。

### 本機測試

```bash
cargo run  # 會起在 localhost:5678
# 但本機沒有手機，/api/devices 會回空陣列
# 截圖和操控需要在遠端測試
```

## Coding Patterns

### AppState
```rust
#[derive(Clone)]
pub struct AppState {
    pub adb: AdbClient,     // ADB 操作
    pub scrcpy: SessionManager,  // scrcpy 串流管理
    pub sim: SimManager,    // SIM 切換
}
```
所有 handler 透過 `State(state): State<AppState>` 取得。

### 錯誤處理
```rust
enum AppError { Adb(String), NotFound(String), Sim(String), Io(io::Error) }
```
impl `IntoResponse` → JSON `{"error": "..."}` + HTTP status。所有 handler 回傳 `Result<impl IntoResponse, AppError>`。

### ADB 呼叫
全部透過 `AdbClient::run(args)` 和 `AdbClient::run_text(args)`：
- 自動 acquire semaphore permit
- 自動 timeout 10s
- 自動檢查 exit code

### 截圖策略
`AdbClient::screenshot(serial)` 的邏輯：
1. 嘗試 `screenshot_atx()` — 透過 adb forward 呼叫裝置上的 atx-agent jsonrpc `takeScreenshot(1, 80)`，回傳 JPEG base64
2. 失敗則 fallback 到 `adb -s {serial} exec-out screencap -p`，回傳 PNG

atx-agent 快 3 倍 (0.16s vs 0.4s)，小 3 倍 (120KB vs 380KB)。

### atx-agent forward 管理
- 每個 device 第一次截圖時自動 `adb forward tcp:{port} tcp:9008`
- port 從 17100 開始遞增
- 快取在 `AdbClient::atx_ports: RwLock<HashMap<String, u16>>`

### 前端截圖輪詢
- 交錯輪詢：每 200ms 更新一台，round-robin
- 操作後 (tap/swipe) 立即連截 3 張 (0ms, 400ms, 900ms)
- 操作中的裝置暫時跳過背景輪詢

### 前端座標計算
因為 `<img>` 用 `object-fit: contain`，黑邊不算在內：
```javascript
function getImageContentRect(img)  // 算出圖片實際顯示區域
function mouseToRatio(e, img)      // 滑鼠位置 → 0~1 比例座標
```

## 外部依賴

### ADB (Android Debug Bridge)
- 手機透過 USB 接在遠端主機
- `adb devices -l` 列出裝置
- `adb -s {serial} exec-out screencap -p` 截圖 (PNG)
- `adb -s {serial} shell input tap/swipe/keyevent/text` 操控
- `adb -s {serial} shell wm size` 取得螢幕尺寸
- `adb -s {serial} forward tcp:PORT tcp:9008` 轉發到 atx-agent
- `adb -s {serial} push file /path` 推送檔案到裝置

### atx-agent (uiautomator2)
- 跑在每台手機上的 HTTP server (port 9008)
- 透過 adb forward 存取
- jsonrpc endpoint: `POST /jsonrpc/0`
- 截圖: `{"method": "takeScreenshot", "params": [1, 80]}` → base64 JPEG（注意：回傳的 base64 含換行符，需 strip）

### scrcpy server v2.7
- JAR 內嵌在 binary 中 (`assets/scrcpy-server-v2.7.jar`)
- Push 到裝置 `/data/local/tmp/scrcpy_server.jar`
- 啟動: `CLASSPATH=... app_process / com.genymobile.scrcpy.Server 2.7 ...`
- 參數: `max_size=1024 max_fps=30 video_bit_rate=8000000 tunnel_forward=true send_frame_meta=false control=true audio=false`
- 連線方式: `adb forward tcp:PORT localabstract:scrcpy`，然後 TCP connect **兩次**（第一次=video，第二次=control）
- Handshake (77 bytes): 1(dummy `0x00`) + 64(device name) + 4(codec) + 8(resolution W+H, BE u32)
- Video: raw H.264 Annex B byte stream
- Control: 32 bytes per touch event, 14 bytes per key event
- **注意**: 多台同時啟動會衝突，使用 Semaphore(3) 序列化
- **注意**: scrcpy 只在啟動時送 SPS/PPS 一次，session 不能複用，每次 WebSocket 連接需重啟 session
- **注意**: 前端 WebCodecs H.264 解碼需要 Secure Context (localhost 或 HTTPS)，非 localhost 的 HTTP 無法使用

### SIM 切換 Python 腳本
- 位置: `/home/largitdata/project/sim_switch_api/`
- `switch_phone_number.py {device_id} --index {sim_order}` — 單台切換
- `switch_all_devices.py {sim_order}` — 全部切換
- `switch_all_devices.py --current` — 查詢目前號碼
- 透過 uiautomator2 操控 STK app UI 完成切換
- `device_phones.json` — 裝置與號碼對照表

## 部署環境

### 遠端主機
- **Host**: `largitdata-wifi-pool` / `192.168.88.191`
- **OS**: Ubuntu 24.04 x86_64
- **CPU**: 4 cores
- **RAM**: 7.5 GB
- **User**: `largitdata`

### systemd service
- 檔案: `~/.config/systemd/user/largitdata-wifi-pool-ui.service`
- `loginctl enable-linger largitdata` 已啟用
- `Restart=always`, `RestartSec=3`

### Port 轉發
本機 Mac 上有 port-forward binary (`~/project/fast-api/`)，轉發 5678 到遠端。

### 其他仍在運行的舊服務（待淘汰）
- uiautodev: port 20242 (systemd user service)
- multi-device-viewer: port 20243 (systemd user service)
- sim-switch-api: port 3456 (systemd user service)

## 已知限制

1. **截圖非即時** — 使用截圖輪詢 (~200ms/台)，不是 scrcpy 串流。15 台時每輪約 3 秒。
2. **scrcpy 需要 HTTPS** — WebCodecs API 需要 Secure Context，目前 HTTP IP 存取無法使用。
3. **中文輸入** — `adb shell input text` 不支援中文，需要特殊 IME (如 ADBKeyboard)。
4. **SIM 切換速度** — Python 腳本操控 UI 需 10-30 秒/台。
5. **前端嵌入** — 改前端也要重新 `cargo build`，因為 rust-embed 在 compile-time 嵌入。

## 可能的改進方向

- 加 HTTPS (self-signed cert) 啟用 scrcpy 串流
- 前端改成 dev mode 時從磁碟讀取（不用每次 rebuild）
- 截圖並發優化（目前 round-robin 200ms interval）
- 裝置離線偵測和自動重連
- SIM 切換進度回報（目前是 blocking 等 Python 腳本完成）
- 批次操作：對多台裝置同時送相同 tap/swipe
