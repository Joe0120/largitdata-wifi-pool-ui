# Largitdata WiFi Pool UI

多台 Android 手機的統一監看、操控、SIM 卡切換平台。單一 Rust binary，內嵌 Web 前端，取代原本三個獨立服務（uiautodev + multi-device-viewer + sim-switch-api）。

## 功能概覽

### 多裝置監看
- 即時截圖輪詢（透過 atx-agent JPEG，每張 ~0.16s）
- 可調整 grid 欄數（1-10），預設 8 欄
- 裝置按型號分組，可勾選單台或整組
- 勾選即自動開始顯示，增量更新不重連

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
- **查看目前**：View Current 查詢所有裝置當前使用的號碼
- **API 切換**：可透過 URL 直接觸發切換（適合自動化）

### 通知系統
- **Toast**：右上角彈出通知，3 秒自動消失
- **通知紀錄**：🔔 按鈕展開歷史紀錄，可標記已讀/全部已讀/清除
- 通知文字可選取複製
- 只存前端記憶體，刷新頁面即清空

### UI 功能
- 左側 sidebar 可拖拉調整寬度，可收起/展開
- 深色主題
- Device ID 可選取複製

---

## 存取方式

| 項目 | 值 |
|------|-----|
| URL | `http://192.168.88.191:5678` |
| Port | 5678 |
| Binary 位置 | `/home/largitdata/project/largitdata-wifi-pool-ui/target/release/largitdata-wifi-pool-ui` |

## 服務管理

```bash
# 查看狀態
systemctl --user status largitdata-wifi-pool-ui

# 重啟
systemctl --user restart largitdata-wifi-pool-ui

# 停止
systemctl --user stop largitdata-wifi-pool-ui

# 看 log
journalctl --user -u largitdata-wifi-pool-ui -f

# Debug log
# 修改 ~/.config/systemd/user/largitdata-wifi-pool-ui.service
# 把 RUST_LOG=info 改成 RUST_LOG=debug
systemctl --user daemon-reload
systemctl --user restart largitdata-wifi-pool-ui
```

---

## API 參考

### 裝置管理

#### `GET /api/devices`
列出所有已連接且已授權的 Android 裝置。

回應：
```json
[
  {
    "serial": "R38M605CSEH",
    "model": "SM_G975U1",
    "product": "beyond2qlteue",
    "status": "device"
  }
]
```

#### `GET /api/devices/{serial}/screenshot`
取得裝置截圖。優先使用 atx-agent（回傳 JPEG），失敗時 fallback 到 `adb screencap`（回傳 PNG）。

回應：`image/jpeg` 或 `image/png` binary

#### `POST /api/devices/{serial}/tap`
點擊裝置螢幕。

Body：
```json
{ "x": 540, "y": 960 }
```

#### `POST /api/devices/{serial}/swipe`
滑動裝置螢幕。

Body：
```json
{ "x1": 540, "y1": 1500, "x2": 540, "y2": 500, "duration_ms": 300 }
```

#### `POST /api/devices/{serial}/key`
送出按鍵事件。

Body：
```json
{ "keycode": 3 }
```

常用 keycode：
| keycode | 功能 |
|---------|------|
| 3 | Home |
| 4 | Back |
| 187 | Recent Apps |
| 24 | Volume Up |
| 25 | Volume Down |

#### `POST /api/devices/{serial}/text`
輸入文字（需先點擊輸入框讓它獲得焦點）。

Body：
```json
{ "text": "hello world" }
```

#### `POST /api/devices/{serial}/shell`
執行 ADB shell 指令。

Body：
```json
{ "command": "pm list packages" }
```

回應：
```json
{ "output": "package:com.android.settings\n..." }
```

#### `POST /api/devices/{serial}/rotate`
強制裝置轉為直向（關閉自動旋轉 + 設定 rotation=0）。

#### `GET /api/devices/{serial}/window-size`
取得裝置螢幕尺寸。

回應：
```json
{ "width": 1080, "height": 1920 }
```

### SIM 卡管理

#### `GET /api/sim/devices`
列出所有 SIM 裝置和號碼對照表（讀取 `device_phones.json`）。

回應：
```json
[
  {
    "device_id": "03157df34dcc1e3a",
    "card": [
      {
        "no": "193",
        "sim_no": "25",
        "phone_number": "886933246524",
        "app_lable": "01933246524",
        "sim_number": "3LK21CT007324",
        "sim_order": 1
      }
    ]
  }
]
```

#### `GET /api/sim/current`
查詢所有裝置目前使用的 SIM 號碼。

回應：
```json
{ "output": "03157df34dcc1e3a | 目前: 01933246524 (共 15 個號碼)\n..." }
```

#### `POST /api/sim/switch-all`
全部裝置切換到指定 SIM slot。

Body：
```json
{ "sim_order": 5 }
```

#### `POST /api/sim/switch`
指定裝置切換到指定 SIM slot。

Body：
```json
{ "device_id": "03157df34dcc1e3a", "sim_order": 5 }
```

#### `GET /api/sim/switch-by-phone/{phone_number}`
根據電話號碼自動找到對應裝置並切換。

範例：
```
GET /api/sim/switch-by-phone/886905349387
```

回應：
```json
{
  "ok": true,
  "device_id": "03157df3c91de513",
  "sim_order": 3,
  "phone_number": "886905349387",
  "output": "..."
}
```

### scrcpy 串流（實驗性）

#### `WS /api/devices/{serial}/stream`
WebSocket 連線，scrcpy H.264 即時串流。需要 WebCodecs API（Chrome Secure Context）。

- 下行：H.264 Annex B binary frames
- 上行：JSON text messages
  - `{"type": "touchDown", "xP": 0.5, "yP": 0.3}` — 觸控（比例座標 0~1）
  - `{"type": "touchMove", "xP": 0.6, "yP": 0.4}`
  - `{"type": "touchUp", "xP": 0.6, "yP": 0.4}`
  - `{"type": "keyEvent", "data": {"eventNumber": 4}}` — 按鍵

目前前端使用截圖模式，scrcpy 串流 API 保留供未來使用。

---

## 環境變數

| 變數 | 預設值 | 說明 |
|------|--------|------|
| `PORT` | `5678` | HTTP server port |
| `ADB_PATH` | `adb` | ADB binary 路徑 |
| `PYTHON_PATH` | `python3` | Python binary 路徑 |
| `SCRIPTS_DIR` | `.` | SIM 切換 Python 腳本目錄 |
| `DEVICE_JSON` | `device_phones.json` | 裝置號碼對照表路徑 |
| `RUST_LOG` | `info` | Log 等級（debug/info/warn/error） |

---

## 部署更新

從開發機（Mac）：

```bash
cd ~/project/largitdata-wifi-pool-ui
./build.sh
ssh largitdata-wifi-pool 'systemctl --user restart largitdata-wifi-pool-ui'
```

`build.sh` 會 rsync 原始碼到遠端並在遠端 `cargo build --release`。
