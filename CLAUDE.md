# CLAUDE.md — Largitdata WiFi Pool UI

## 專案概述

單一 Rust binary 的 Web 應用，用於監看/操控 38+ 台 Android 手機 + SIM 卡切換 + 簡訊收集。取代原本三個獨立服務（uiautodev, multi-device-viewer, sim-switch-api）。前端是 vanilla JS，內嵌在 binary 中（rust-embed）。

## 技術棧

| 層 | 技術 |
|-----|------|
| **Backend** | Rust, axum 0.8, tokio (async runtime) |
| **Database** | SQLite via rusqlite (bundled，嵌入 binary) |
| **Frontend** | Vanilla HTML/CSS/JS, 無框架無 build step |
| **嵌入前端** | rust-embed 8 (compile-time 嵌入 `frontend/` 目錄) |
| **ADB 通訊** | tokio::process::Command 呼叫 `adb` CLI |
| **截圖** | atx-agent jsonrpc 取 JPEG → server 端 cache → client 讀 cache |
| **SIM 切換** | subprocess 呼叫 Python 腳本 (uiautomator2 操控 STK app) |
| **scrcpy** | 內建但目前前端未啟用（需要 jmuxer 或 WebCodecs） |
| **HTTP client** | reqwest (呼叫 atx-agent) |
| **序列化** | serde / serde_json |
| **日誌** | tracing + tracing-subscriber |
| **CORS** | tower-http CorsLayer::permissive() |

## 專案結構

```
src/
├── main.rs                    # 進入點：Config → DB → AppState → Router → serve
│                                啟動時：import JSON → start cache polling → setup adb reverse
├── config.rs                  # 環境變數設定 (PORT, ADB_PATH, DB_PATH 等)
├── error.rs                   # AppError enum → axum IntoResponse (JSON error)
├── screenshot_cache.rs        # ScreenshotCache: 背景 task 並行截圖 + in-memory cache
│                                - active flag: 沒人看時不截圖
│                                - 每輪對所有裝置並行截圖，各自完成即更新 cache
│                                - 多 client 讀同一份 cache，不加重手機負擔
├── adb/
│   ├── client.rs              # AdbClient — 所有 ADB 操作的封裝
│   │                            - Semaphore(50) 並發控制
│   │                            - 10s timeout per command
│   │                            - atx-agent JPEG 截圖 (優先) + screencap fallback
│   │                            - adb forward 管理 (atx_ports cache)
│   │                            - run_raw(): 不走 semaphore 的快速操作
│   └── types.rs               # DeviceInfo, WindowSize
├── db/
│   ├── mod.rs                 # Database struct (Arc<Mutex<Connection>>)
│   ├── migrations.rs          # CREATE TABLE IF NOT EXISTS (auto-run on startup)
│   ├── devices.rs             # devices + sim_cards CRUD, import_from_json, get_phone_status
│   └── sms.rs                 # sms_messages CRUD
├── api/
│   ├── mod.rs                 # 組裝所有 Router
│   ├── devices.rs             # /api/devices/* handlers (截圖從 cache 讀)
│   ├── sim.rs                 # /api/sim/* + /api/sms/* handlers
│   │                            - switch 成功後自動更新 DB
│   │                            - sync 解析 Python 輸出寫入 DB
│   ├── stream.rs              # WS /api/devices/{serial}/stream (scrcpy)
│   └── frontend.rs            # rust-embed 靜態檔案服務
├── scrcpy/
│   ├── protocol.rs            # scrcpy v2 二進制觸控/按鍵封包
│   ├── server.rs              # ScrcpySession: push JAR, launch, 2-socket connect
│   └── session_manager.rs     # SessionManager: create, Semaphore(3)
└── sim/
    ├── manager.rs             # SimManager: 呼叫 Python 腳本
    └── types.rs               # SimDevice, SimCard (對應 device_phones.json)

frontend/
├── index.html                 # 單頁 dashboard (sidebar + toolbar + grid)
├── style.css                  # 深色主題
└── app.js                     # 所有前端邏輯

scripts/
├── switch_all_devices.py      # 批次切換 + 驗證 + --current 查詢
└── switch_phone_number.py     # 單台切換

assets/
└── scrcpy-server-v2.7.jar     # include_bytes! 內嵌進 binary

device_phones.json             # 裝置與號碼對照表（啟動時匯入 DB）
data.db                        # SQLite 資料庫（runtime 產生，gitignore）
build.sh                       # rsync + remote cargo build
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
# 3. 本機測試（port 5679 避免跟 forward 衝突）
PORT=5679 cargo run
# 4. 部署到遠端
./build.sh
ssh largitdata-wifi-pool 'systemctl --user restart largitdata-wifi-pool-ui'
```

### 注意事項
- 前端改完也要 `cargo build`（rust-embed compile-time 嵌入）
- 本機沒手機，截圖和操控需在遠端測試
- `data.db` 在 `.gitignore` 裡，啟動時自動建立並從 JSON 匯入
- Port 5678 本機被 port-forward 佔用，本機測試用其他 port

## Coding Patterns

### AppState
```rust
#[derive(Clone)]
pub struct AppState {
    pub adb: AdbClient,
    pub scrcpy: SessionManager,
    pub sim: SimManager,
    pub screenshots: ScreenshotCache,
    pub db: Database,
}
```

### 錯誤處理
```rust
enum AppError { Adb(String), NotFound(String), Sim(String), Io(io::Error) }
// impl IntoResponse → JSON {"error": "..."} + HTTP status
```

### ADB 呼叫
- `AdbClient::run(args)` — 走 semaphore + timeout
- `AdbClient::run_raw(args)` — 不走 semaphore（for quick ops like `adb reverse`）
- `AdbClient::run_text(args)` — run + UTF-8 decode

### 截圖架構
```
ScreenshotCache (背景 task, 持續循環)
  ├─ check active flag → false 則 sleep 1s
  ├─ adb.list_devices()
  ├─ 對每台 spawn tokio task → adb.screenshot()
  │   ├─ screenshot_atx(): HTTP POST atx-agent jsonrpc → base64 JPEG
  │   └─ fallback: adb exec-out screencap -p → PNG
  ├─ 每台截完立即寫入 cache (RwLock<HashMap>)
  └─ 不 sleep，立刻下一輪

API handler:
  GET /screenshot → cache.get() → 回傳 JPEG/PNG（< 1ms）
  cache.get() 同時設 active = true
```

### atx-agent 通訊
- 每台裝置跑 atx-agent HTTP server (port 9008)
- `adb forward tcp:{17100+N} tcp:9008` 映射到本機 port
- jsonrpc: `POST /jsonrpc/0` → `{"method":"takeScreenshot","params":[1,80]}`
- 回傳 base64 JPEG（含換行符，需 `retain(!is_ascii_whitespace)` 清理）
- port 快取在 `AdbClient::atx_ports: RwLock<HashMap>`

### Database (SQLite)
- 單連線 + `Arc<Mutex<Connection>>`
- WAL mode + foreign keys
- 啟動時 auto-migrate (CREATE TABLE IF NOT EXISTS)
- 首次啟動從 `device_phones.json` 匯入 (upsert)
- SIM switch 成功後自動更新 `devices.current_phone`

### 前端座標計算
`<img>` 用 `object-fit: contain`，黑邊不算：
```javascript
getImageContentRect(img)  // 圖片實際顯示區域
mouseToRatio(e, img)      // 滑鼠 → 0~1 比例座標
```

### 前端截圖更新
- 每 200ms 發一批 5 台（避免 Chrome 6 連線限制）
- 操作後立即 + 800ms 各截一次（actionRefresh）
- `screenshotInFlight` Set 避免同台重複請求

## 外部依賴

### ADB (Android Debug Bridge)
- 手機透過 USB 接在遠端主機
- `adb devices -l` — 列出裝置
- `adb -s {serial} exec-out screencap -p` — 截圖 (PNG, fallback)
- `adb -s {serial} shell input tap/swipe/keyevent/text` — 操控
- `adb -s {serial} forward tcp:PORT tcp:9008` — 轉發到 atx-agent
- `adb -s {serial} reverse tcp:5678 tcp:5678` — 讓手機連 server（自動設定，每 30s 檢查新裝置）

### atx-agent (uiautomator2)
- 跑在每台手機上的 HTTP server (port 9008)
- jsonrpc: `takeScreenshot(scale, quality)` → base64 JPEG
- 比 `adb screencap` 快 3 倍 (0.16s vs 0.4s)，小 3 倍

### SIM 切換 Python 腳本
- 位置: `scripts/`（專案內）
- `switch_phone_number.py {device_id} --index {app_order}` — 單台切換
- `switch_all_devices.py {app_order}` — 全部切換（並行，有驗證）
- `switch_all_devices.py --current` — 查詢目前號碼
- 透過 uiautomator2 操控 STK app UI
- 切換後重新開 STK 驗證是否生效
- 輸出格式: `[OK]`/`[FAIL]`/`[ERROR]`/`[SKIP] device_id | message`
- `device_phones.json` — 裝置與號碼對照表

### scrcpy server v2.7（實驗性，前端未啟用）
- JAR 內嵌 binary (`assets/scrcpy-server-v2.7.jar`)
- 連線: `adb forward` → TCP connect 兩次（video + control）
- Handshake 77 bytes → H.264 Annex B stream
- 前端需要 jmuxer (MSE) 解碼，不需要 HTTPS
- 目前前端用截圖模式，scrcpy 串流待未來整合

## 部署環境

### 遠端主機
- **Host**: `largitdata-wifi-pool` / `192.168.88.191`
- **OS**: Ubuntu 24.04 x86_64, 4 cores, 7.5 GB RAM
- **User**: `largitdata`

### systemd service
- `~/.config/systemd/user/largitdata-wifi-pool-ui.service`
- `loginctl enable-linger largitdata` 已啟用
- 環境變數: PORT=5678, DEVICE_JSON, SCRIPTS_DIR, DB_PATH

### Port 轉發
本機 Mac 上有 port-forward binary (`~/project/fast-api/`)，轉發 5678/20242/20243/3456 到遠端。

### 舊服務（待淘汰）
- uiautodev: port 20242
- multi-device-viewer: port 20243
- sim-switch-api: port 3456

## 已知限制

1. **截圖非即時** — 使用截圖快取輪詢（34 台 ~700ms 一輪），不是串流
2. **scrcpy 前端未整合** — 後端有，前端需要 jmuxer 才能在 HTTP 下播放
3. **中文輸入** — `adb shell input text` 不支援中文，需特殊 IME
4. **SIM 切換速度** — Python 腳本操控 UI 需 10-30 秒/台（含驗證）
5. **前端嵌入** — 改前端要重新 `cargo build`
6. **SQLite 單寫** — 單連線 Mutex，高併發寫入會排隊（目前場景足夠）

## 可能的改進方向

- jmuxer 整合 scrcpy 串流（替代截圖輪詢，延遲 50-100ms vs 700ms）
- SMS 前端 UI（目前只有 API）
- device_phones.json 改為純 DB 管理（前端 CRUD）
- 裝置離線偵測
- 批次操作：對多台裝置同時送相同 tap/swipe
