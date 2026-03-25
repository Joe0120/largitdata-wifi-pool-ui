use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use axum::http::{header, StatusCode};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/openapi.json", get(openapi_json))
        .route("/swagger", get(swagger_ui))
        .route("/swagger/", get(swagger_ui))
}

async fn openapi_json() -> impl IntoResponse {
    let spec = serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Largitdata WiFi Pool UI",
            "description": "多台 Android 手機統一監看、操控、SIM 卡切換、簡訊收集平台",
            "version": "0.1.0"
        },
        "tags": [
            {"name": "Devices", "description": "裝置操控 (ADB)"},
            {"name": "SIM", "description": "SIM 卡切換管理"},
            {"name": "SMS", "description": "簡訊收集"}
        ],
        "paths": {
            "/api/devices": {
                "get": {
                    "tags": ["Devices"],
                    "summary": "列出所有連線裝置",
                    "responses": {
                        "200": {
                            "description": "裝置列表",
                            "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/DeviceInfo"}}}}
                        }
                    }
                }
            },
            "/api/devices/{serial}/screenshot": {
                "get": {
                    "tags": ["Devices"],
                    "summary": "取得裝置截圖 (JPEG from cache)",
                    "parameters": [{"name": "serial", "in": "path", "required": true, "schema": {"type": "string"}}],
                    "responses": {"200": {"description": "截圖", "content": {"image/jpeg": {"schema": {"type": "string", "format": "binary"}}}}}
                }
            },
            "/api/devices/{serial}/tap": {
                "post": {
                    "tags": ["Devices"],
                    "summary": "點擊裝置螢幕",
                    "parameters": [{"name": "serial", "in": "path", "required": true, "schema": {"type": "string"}}],
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/TapRequest"}}}},
                    "responses": {"200": {"description": "成功", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/OkResponse"}}}}}
                }
            },
            "/api/devices/{serial}/swipe": {
                "post": {
                    "tags": ["Devices"],
                    "summary": "滑動裝置螢幕",
                    "parameters": [{"name": "serial", "in": "path", "required": true, "schema": {"type": "string"}}],
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SwipeRequest"}}}},
                    "responses": {"200": {"description": "成功", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/OkResponse"}}}}}
                }
            },
            "/api/devices/{serial}/key": {
                "post": {
                    "tags": ["Devices"],
                    "summary": "送出按鍵事件 (3=Home, 4=Back, 187=Recent)",
                    "parameters": [{"name": "serial", "in": "path", "required": true, "schema": {"type": "string"}}],
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/KeyRequest"}}}},
                    "responses": {"200": {"description": "成功", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/OkResponse"}}}}}
                }
            },
            "/api/devices/{serial}/text": {
                "post": {
                    "tags": ["Devices"],
                    "summary": "輸入文字",
                    "parameters": [{"name": "serial", "in": "path", "required": true, "schema": {"type": "string"}}],
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/TextRequest"}}}},
                    "responses": {"200": {"description": "成功", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/OkResponse"}}}}}
                }
            },
            "/api/devices/{serial}/shell": {
                "post": {
                    "tags": ["Devices"],
                    "summary": "執行 ADB shell 指令",
                    "parameters": [{"name": "serial", "in": "path", "required": true, "schema": {"type": "string"}}],
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ShellRequest"}}}},
                    "responses": {"200": {"description": "指令輸出", "content": {"application/json": {"schema": {"type": "object", "properties": {"output": {"type": "string"}}}}}}}
                }
            },
            "/api/devices/{serial}/rotate": {
                "post": {
                    "tags": ["Devices"],
                    "summary": "強制直向 (關閉自動旋轉)",
                    "parameters": [{"name": "serial", "in": "path", "required": true, "schema": {"type": "string"}}],
                    "responses": {"200": {"description": "成功", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/OkResponse"}}}}}
                }
            },
            "/api/devices/{serial}/window-size": {
                "get": {
                    "tags": ["Devices"],
                    "summary": "取得螢幕尺寸",
                    "parameters": [{"name": "serial", "in": "path", "required": true, "schema": {"type": "string"}}],
                    "responses": {"200": {"description": "螢幕尺寸", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/WindowSize"}}}}}
                }
            },
            "/api/sim/devices": {
                "get": {
                    "tags": ["SIM"],
                    "summary": "號碼對照表 (讀 device_phones.json)",
                    "responses": {"200": {"description": "裝置與號碼列表"}}
                }
            },
            "/api/sim/current": {
                "get": {
                    "tags": ["SIM"],
                    "summary": "所有裝置目前號碼 (讀 DB)",
                    "responses": {"200": {"description": "裝置狀態列表"}}
                }
            },
            "/api/sim/current/{phone}": {
                "get": {
                    "tags": ["SIM"],
                    "summary": "指定號碼狀態 (讀 DB)",
                    "parameters": [{"name": "phone", "in": "path", "required": true, "schema": {"type": "string"}, "example": "886933246524"}],
                    "responses": {
                        "200": {"description": "號碼狀態", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/PhoneStatus"}}}},
                        "404": {"description": "號碼不存在"}
                    }
                }
            },
            "/api/sim/sync": {
                "get": {
                    "tags": ["SIM"],
                    "summary": "同步狀態 (跑 Python 腳本，結果寫入 DB)",
                    "description": "觸發 switch_all_devices.py --current，解析結果更新 DB 中的 current_phone",
                    "responses": {"200": {"description": "同步結果"}}
                }
            },
            "/api/sim/switch": {
                "post": {
                    "tags": ["SIM"],
                    "summary": "單台切換",
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SwitchRequest"}}}},
                    "responses": {"200": {"description": "切換結果"}}
                }
            },
            "/api/sim/switch-all": {
                "post": {
                    "tags": ["SIM"],
                    "summary": "全部裝置切換",
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SwitchAllRequest"}}}},
                    "responses": {"200": {"description": "逐台結果", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SwitchAllResponse"}}}}}
                }
            },
            "/api/sim/switch-by-phone/{phone}": {
                "get": {
                    "tags": ["SIM"],
                    "summary": "按號碼自動找裝置並切換",
                    "parameters": [{"name": "phone", "in": "path", "required": true, "schema": {"type": "string"}, "example": "886905349387"}],
                    "responses": {"200": {"description": "切換結果"}, "404": {"description": "號碼不存在"}}
                }
            },
            "/api/sms": {
                "post": {
                    "tags": ["SMS"],
                    "summary": "手機轉發簡訊",
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/NewSms"}}}},
                    "responses": {"200": {"description": "寫入成功", "content": {"application/json": {"schema": {"type": "object", "properties": {"ok": {"type": "boolean"}, "id": {"type": "integer"}}}}}}}
                }
            },
            "/api/sms/{phone}": {
                "get": {
                    "tags": ["SMS"],
                    "summary": "查詢指定號碼簡訊",
                    "parameters": [
                        {"name": "phone", "in": "path", "required": true, "schema": {"type": "string"}, "example": "886933246524"},
                        {"name": "limit", "in": "query", "required": false, "schema": {"type": "integer", "default": 5}, "description": "最新幾則"}
                    ],
                    "responses": {"200": {"description": "簡訊列表", "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/SmsMessage"}}}}}}
                }
            }
        },
        "components": {
            "schemas": {
                "DeviceInfo": {
                    "type": "object",
                    "properties": {
                        "serial": {"type": "string", "example": "R38M605CSEH"},
                        "model": {"type": "string", "example": "SM_G975U1"},
                        "product": {"type": "string", "example": "beyond2qlteue"},
                        "status": {"type": "string", "example": "device"}
                    }
                },
                "WindowSize": {
                    "type": "object",
                    "properties": {"width": {"type": "integer", "example": 1080}, "height": {"type": "integer", "example": 1920}}
                },
                "TapRequest": {
                    "type": "object", "required": ["x", "y"],
                    "properties": {"x": {"type": "number", "example": 540}, "y": {"type": "number", "example": 960}}
                },
                "SwipeRequest": {
                    "type": "object", "required": ["x1", "y1", "x2", "y2"],
                    "properties": {
                        "x1": {"type": "number", "example": 540}, "y1": {"type": "number", "example": 1500},
                        "x2": {"type": "number", "example": 540}, "y2": {"type": "number", "example": 500},
                        "duration_ms": {"type": "integer", "example": 300, "description": "預設 300ms"}
                    }
                },
                "KeyRequest": {
                    "type": "object", "required": ["keycode"],
                    "properties": {"keycode": {"type": "integer", "example": 3, "description": "3=Home, 4=Back, 187=Recent"}}
                },
                "TextRequest": {
                    "type": "object", "required": ["text"],
                    "properties": {"text": {"type": "string", "example": "hello"}}
                },
                "ShellRequest": {
                    "type": "object", "required": ["command"],
                    "properties": {"command": {"type": "string", "example": "pm list packages"}}
                },
                "OkResponse": {
                    "type": "object",
                    "properties": {"ok": {"type": "boolean"}}
                },
                "SwitchRequest": {
                    "type": "object", "required": ["device_id", "app_order"],
                    "properties": {"device_id": {"type": "string"}, "app_order": {"type": "integer", "example": 5}}
                },
                "SwitchAllRequest": {
                    "type": "object", "required": ["app_order"],
                    "properties": {"app_order": {"type": "integer", "example": 5}}
                },
                "SwitchAllResponse": {
                    "type": "object",
                    "properties": {
                        "ok": {"type": "boolean"},
                        "success": {"type": "integer"},
                        "failed": {"type": "integer"},
                        "results": {"type": "array", "items": {"type": "object", "properties": {"status": {"type": "string"}, "message": {"type": "string"}}}}
                    }
                },
                "PhoneStatus": {
                    "type": "object",
                    "properties": {
                        "phone_number": {"type": "string"}, "device_serial": {"type": "string"},
                        "app_order": {"type": "integer"}, "is_active": {"type": "boolean"},
                        "current_phone": {"type": "string", "nullable": true}
                    }
                },
                "NewSms": {
                    "type": "object", "required": ["device_serial"],
                    "properties": {
                        "device_serial": {"type": "string"}, "phone_number": {"type": "string"},
                        "sender": {"type": "string"}, "body": {"type": "string"},
                        "received_at": {"type": "string", "format": "date-time"}
                    }
                },
                "SmsMessage": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "integer"}, "device_serial": {"type": "string"},
                        "phone_number": {"type": "string"}, "sender": {"type": "string"},
                        "body": {"type": "string"}, "received_at": {"type": "string"},
                        "created_at": {"type": "string"}
                    }
                }
            }
        }
    });

    ([(header::CONTENT_TYPE, "application/json")], serde_json::to_string(&spec).unwrap())
}

async fn swagger_ui() -> impl IntoResponse {
    let html = r#"<!DOCTYPE html>
<html>
<head>
<title>Largitdata WiFi Pool - API Docs</title>
<meta charset="utf-8">
<link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
<style>body{margin:0} .swagger-ui .topbar{display:none}</style>
</head>
<body>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
<script>
SwaggerUIBundle({ url: '/api/openapi.json', dom_id: '#swagger-ui', deepLinking: true });
</script>
</body>
</html>"#;
    (StatusCode::OK, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], html)
}
