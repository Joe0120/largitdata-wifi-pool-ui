use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/sim/devices", get(list_sim_devices))
        .route("/api/sim/current", get(current_sims))
        .route("/api/sim/switch", post(switch_sim))
        .route("/api/sim/switch-all", post(switch_all))
        .route("/api/sim/switch-by-phone/{phone}", get(switch_by_phone))
}

async fn list_sim_devices(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let devices = state.sim.load_devices().await?;
    Ok(Json(devices))
}

async fn current_sims(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let output = state.sim.get_current().await?;
    Ok(Json(serde_json::json!({"output": output})))
}

#[derive(Deserialize)]
struct SwitchRequest {
    device_id: Option<String>,
    sim_order: u32,
}

async fn switch_sim(
    State(state): State<AppState>,
    Json(body): Json<SwitchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let device_id = body
        .device_id
        .as_ref()
        .ok_or_else(|| AppError::Sim("device_id is required".into()))?;
    let output = state.sim.switch_device(device_id, body.sim_order).await?;
    Ok(Json(serde_json::json!({"ok": true, "output": output})))
}

#[derive(Deserialize)]
struct SwitchAllRequest {
    sim_order: u32,
}

async fn switch_all(
    State(state): State<AppState>,
    Json(body): Json<SwitchAllRequest>,
) -> Result<impl IntoResponse, AppError> {
    let output = state.sim.switch_all(body.sim_order).await?;
    Ok(Json(serde_json::json!({"ok": true, "output": output})))
}

async fn switch_by_phone(
    State(state): State<AppState>,
    Path(phone): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let devices = state.sim.load_devices().await?;

    for dev in &devices {
        for card in &dev.card {
            if card.phone_number == phone {
                let sim_order = card.sim_order.as_u64()
                    .ok_or_else(|| AppError::Sim("Invalid sim_order".into()))? as u32;
                let output = state.sim.switch_device(&dev.device_id, sim_order).await?;
                return Ok(Json(serde_json::json!({
                    "ok": true,
                    "device_id": dev.device_id,
                    "sim_order": sim_order,
                    "phone_number": phone,
                    "output": output
                })));
            }
        }
    }

    Err(AppError::NotFound(format!("Phone number {} not found", phone)))
}
