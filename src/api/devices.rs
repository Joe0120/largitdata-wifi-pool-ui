use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/devices", get(list_devices))
        .route("/api/devices/{serial}/screenshot", get(screenshot))
        .route("/api/devices/{serial}/tap", post(tap))
        .route("/api/devices/{serial}/swipe", post(swipe))
        .route("/api/devices/{serial}/key", post(key))
        .route("/api/devices/{serial}/text", post(text))
        .route("/api/devices/{serial}/shell", post(shell))
        .route("/api/devices/{serial}/rotate", post(rotate))
        .route("/api/devices/{serial}/window-size", get(window_size))
}

async fn list_devices(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let devices = state.adb.list_devices().await?;
    Ok(Json(devices))
}

async fn screenshot(
    State(state): State<AppState>,
    Path(serial): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // Try cache first (populated by background polling task)
    let data = if let Some(cached) = state.screenshots.get(&serial).await {
        cached
    } else {
        // Cache miss — take screenshot directly (first request or new device)
        state.adb.screenshot(&serial).await?
    };
    let mime = if data.len() > 2 && data[0] == 0xFF && data[1] == 0xD8 {
        "image/jpeg"
    } else {
        "image/png"
    };
    Ok(([(header::CONTENT_TYPE, mime)], data))
}

#[derive(Deserialize)]
struct TapRequest {
    x: f64,
    y: f64,
}

async fn tap(
    State(state): State<AppState>,
    Path(serial): Path<String>,
    Json(body): Json<TapRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.adb.tap(&serial, body.x, body.y).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
struct SwipeRequest {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    duration_ms: Option<u64>,
}

async fn swipe(
    State(state): State<AppState>,
    Path(serial): Path<String>,
    Json(body): Json<SwipeRequest>,
) -> Result<impl IntoResponse, AppError> {
    state
        .adb
        .swipe(&serial, body.x1, body.y1, body.x2, body.y2, body.duration_ms.unwrap_or(300))
        .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
struct KeyRequest {
    keycode: u32,
}

async fn key(
    State(state): State<AppState>,
    Path(serial): Path<String>,
    Json(body): Json<KeyRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.adb.key_event(&serial, body.keycode).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
struct TextRequest {
    text: String,
}

async fn text(
    State(state): State<AppState>,
    Path(serial): Path<String>,
    Json(body): Json<TextRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.adb.input_text(&serial, &body.text).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
struct ShellRequest {
    command: String,
}

async fn shell(
    State(state): State<AppState>,
    Path(serial): Path<String>,
    Json(body): Json<ShellRequest>,
) -> Result<impl IntoResponse, AppError> {
    let output = state.adb.shell(&serial, &body.command).await?;
    Ok(Json(serde_json::json!({"output": output})))
}

async fn rotate(
    State(state): State<AppState>,
    Path(serial): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.adb.force_portrait(&serial).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn window_size(
    State(state): State<AppState>,
    Path(serial): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let size = state.adb.window_size(&serial).await?;
    Ok(Json(size))
}
