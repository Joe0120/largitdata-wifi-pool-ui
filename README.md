# Largitdata WiFi Pool UI

多台 Android 手機的統一監看、操控、SIM 卡切換、簡訊收集平台。單一 Rust binary，內嵌 Web 前端與 SQLite 資料庫。

## 功能概覽

### 多裝置監看
- Server 端背景截圖快取（34 台並行 ~700ms 一輪，透過 atx-agent JPEG）
- 多 client 同時看不加重手機負擔（都讀同一份 cache）
- 沒人看時自動停止截圖
- 可調整 grid 欄數（1-10），預設 8 欄
- 裝置按型號分組，可勾選單台或整組，勾選即自動顯示

### 裝置操控
- **點擊**：在截圖上左鍵點擊 = 手機 tap
- **滑動**：按住拖拉 = 手機 swipe（顯示紅色軌跡）
- **返回**：右鍵點擊 = Back 鍵
- **導航按鈕**：☰ Recent / ● Home / ◀ Back / ↻ 強制直向
- **文字輸入**：每台裝置底部輸入框，Enter 送出
- **全部轉直**：toolbar 的 All Portrait 按鈕

### SIM 卡切換
- **搜尋號碼**：輸入部分號碼即時搜尋，點擊結果直接切換
- **群組切換**：選 Group (1-16)，Switch All 全部裝置切到該組
- **同步狀態**：Sync 觸發 Python 腳本查詢所有裝置，結果寫入 DB
- **查看目前**：Current 直接讀 DB，不跑腳本
- **切換驗證**：切換後重新開 STK 確認是否真的生效
- **API 切換**：可透過 URL 直接觸發切換（適合自動化）
- **All Devices**：開新視窗查看完整號碼對照表 JSON

### 簡訊收集
- 手機透過 `adb reverse` 連到 server
- `POST /api/sms` 接收簡訊轉發
- `GET /api/sms/{phone}?limit=5` 查詢指定號碼最新簡訊

### 資料庫
- SQLite 嵌入 binary（rusqlite bundled）
- 啟動時自動從 `device_phones.json` 匯入
- SIM 切換成功自動更新裝置狀態
- Tables：`devices`、`sim_cards`、`sms_messages`

### 通知系統
- **Toast**：右上角彈出通知，3 秒自動消失
- **通知紀錄**：🔔 按鈕展開歷史紀錄，可標記已讀/全部已讀/清除
- 通知文字可選取複製
- SIM switch 結果逐台顯示成功/失敗

### UI 功能
- 左側 sidebar 可拖拉調整寬度，可收起/展開
- SIM Switch dropdown（toolbar 右側）
- 深色主題
- Device ID 可選取複製

---

## 存取方式

| 項目 | 值 |
|------|-----|
| URL | `http://192.168.88.191:5678` |
| Port | 5678 |
| Binary 位置 | `/home/largitdata/project/largitdata-wifi-pool-ui/target/release/largitdata-wifi-pool-ui` |
| DB 位置 | `data.db`（與 binary 同目錄） |

## 服務管理

```bash
systemctl --user status largitdata-wifi-pool-ui    # 查看狀態
systemctl --user restart largitdata-wifi-pool-ui   # 重啟
systemctl --user stop largitdata-wifi-pool-ui      # 停止
journalctl --user -u largitdata-wifi-pool-ui -f    # 看 log
```

---

## API 參考

### 裝置操控

| Method | Path | 說明 | Body |
|--------|------|------|------|
| GET | `/api/devices` | 列出所有連線裝置 | - |
| GET | `/api/devices/{serial}/screenshot` | 截圖（JPEG from cache） | - |
| POST | `/api/devices/{serial}/tap` | 點擊 | `{"x": 540, "y": 960}` |
| POST | `/api/devices/{serial}/swipe` | 滑動 | `{"x1":540,"y1":1500,"x2":540,"y2":500,"duration_ms":300}` |
| POST | `/api/devices/{serial}/key` | 按鍵 | `{"keycode": 3}` |
| POST | `/api/devices/{serial}/text` | 輸入文字 | `{"text": "hello"}` |
| POST | `/api/devices/{serial}/shell` | ADB shell 指令 | `{"command": "pm list packages"}` |
| POST | `/api/devices/{serial}/rotate` | 強制直向 | - |
| GET | `/api/devices/{serial}/window-size` | 螢幕尺寸 | - |
| WS | `/api/devices/{serial}/stream` | scrcpy H.264 串流（實驗性） | - |

常用 keycode：3=Home, 4=Back, 187=Recent, 24=Vol+, 25=Vol-

### SIM 卡管理

| Method | Path | 說明 | Body |
|--------|------|------|------|
| GET | `/api/sim/devices` | 號碼對照表（讀 device_phones.json） | - |
| GET | `/api/sim/current` | 所有裝置目前號碼（讀 DB） | - |
| GET | `/api/sim/current/{phone}` | 指定號碼狀態（讀 DB） | - |
| GET | `/api/sim/sync` | 跑腳本同步狀態到 DB | - |
| POST | `/api/sim/switch` | 單台切換 | `{"device_id":"xxx","app_order":5}` |
| POST | `/api/sim/switch-all` | 全部切換 | `{"app_order": 5}` |
| GET | `/api/sim/switch-by-phone/{phone}` | 按號碼切換 | - |

`/api/sim/switch-all` 回應包含逐台結果：
```json
{
  "ok": false,
  "success": 8,
  "failed": 2,
  "results": [
    {"status": "ok", "message": "[OK] R38M605CSEH | 切換成功 app_order=5 (...)"},
    {"status": "fail", "message": "[FAIL] RF8M31P4MQL | 點擊了 ... 但目前是 ..."}
  ]
}
```

### 簡訊

| Method | Path | 說明 | Body |
|--------|------|------|------|
| POST | `/api/sms` | 手機轉發簡訊 | `{"device_serial":"xxx","phone_number":"886...","sender":"...","body":"...","received_at":"..."}` |
| GET | `/api/sms/{phone}?limit=5` | 查詢指定號碼簡訊（預設最新 5 則） | - |

---

## 環境變數

| 變數 | 預設值 | 說明 |
|------|--------|------|
| `PORT` | `5678` | HTTP server port |
| `ADB_PATH` | `adb` | ADB binary 路徑 |
| `PYTHON_PATH` | `python3` | Python binary 路徑 |
| `SCRIPTS_DIR` | `.` | SIM 切換 Python 腳本目錄 |
| `DEVICE_JSON` | `device_phones.json` | 裝置號碼對照表路徑 |
| `DB_PATH` | `data.db` | SQLite 資料庫路徑 |
| `RUST_LOG` | `info` | Log 等級 |

---

## 部署更新

```bash
cd ~/project/largitdata-wifi-pool-ui
./build.sh
ssh largitdata-wifi-pool 'systemctl --user restart largitdata-wifi-pool-ui'
```

`build.sh` 會 rsync 原始碼到遠端並在遠端 `cargo build --release`。
