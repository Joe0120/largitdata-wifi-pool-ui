use axum::extract::State;
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
