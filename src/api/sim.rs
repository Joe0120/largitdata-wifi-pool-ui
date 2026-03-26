use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::db::sms::NewSms;
use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/sim/devices", get(list_sim_devices))
        .route("/api/sim/current", get(current_from_db))
        .route("/api/sim/current/{phone}", get(current_phone_status))
        .route("/api/sim/sync", get(sync_all))
        .route("/api/sim/switch", post(switch_sim))
        .route("/api/sim/switch-all", post(switch_all))
        .route("/api/sim/switch-by-phone/{phone}", get(switch_by_phone))
        .route("/api/sms", post(receive_sms))
        .route("/api/sms/{phone}", get(get_sms))
        .route("/api/sms/device/{device_id}", get(get_sms_by_device))
}

// ---- SIM devices (from DB) ----

async fn list_sim_devices(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let devices = state.db.list_sim_devices().await?;
    Ok(Json(devices))
}

// ---- Current status (from DB) ----

async fn current_from_db(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let devices = state.db.list_devices().await?;
    Ok(Json(devices))
}

async fn current_phone_status(
    State(state): State<AppState>,
    Path(phone): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    match state.db.get_phone_status(&phone).await? {
        Some(status) => Ok(Json(serde_json::json!(status))),
        None => Err(AppError::NotFound(format!("Phone number {} not found", phone))),
    }
}

// ---- Sync (run Python script, update DB) ----

async fn sync_all(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let output = state.sim.get_current().await?;

    // Parse output lines: "device_id | 目前: 01933246524 (共 15 個號碼)"
    let mut updated = 0;
    for line in output.lines() {
        if !line.contains(" | ") {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, " | ").collect();
        if parts.len() != 2 {
            continue;
        }
        let device_id = parts[0].trim();
        let status = parts[1].trim();

        // Extract current app_lable from "目前: XXXXX (共 N 個號碼)"
        if let Some(rest) = status.strip_prefix("目前: ") {
            if let Some(idx) = rest.find(" (") {
                let current_lable = &rest[..idx];

                // Find phone_number from sim_cards by app_lable
                let conn = state.db.conn().lock().await;
                let result = conn.query_row(
                    "SELECT phone_number, app_order FROM sim_cards WHERE device_id = ?1 AND app_lable = ?2",
                    rusqlite::params![device_id, current_lable],
                    |row| Ok((row.get::<_, String>(0).ok(), row.get::<_, i32>(1).ok())),
                );
                drop(conn);

                if let Ok((phone, app_order)) = result {
                    let phone_str = phone.as_deref().unwrap_or(current_lable);
                    state.db.update_device_current(device_id, phone_str, app_order).await?;
                    updated += 1;
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "updated": updated,
        "output": output,
    })))
}

// ---- Switch ----

#[derive(Deserialize)]
struct SwitchRequest {
    device_id: Option<String>,
    app_order: u32,
}

async fn switch_sim(
    State(state): State<AppState>,
    Json(body): Json<SwitchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let device_id = body
        .device_id
        .as_ref()
        .ok_or_else(|| AppError::Sim("device_id is required".into()))?;
    let output = state.sim.switch_device(device_id, body.app_order).await?;

    // Update DB if switch succeeded
    if output.contains("[OK]") {
        // Find phone_number for this app_order
        let conn = state.db.conn().lock().await;
        let phone = conn.query_row(
            "SELECT phone_number FROM sim_cards WHERE device_id = ?1 AND app_order = ?2",
            rusqlite::params![device_id, body.app_order],
            |row| row.get::<_, String>(0),
        ).ok();
        drop(conn);

        if let Some(phone) = phone {
            state.db.update_device_current(device_id, &phone, Some(body.app_order as i32)).await?;
        }
    }

    Ok(Json(serde_json::json!({"ok": true, "output": output})))
}

#[derive(Deserialize)]
struct SwitchAllRequest {
    app_order: u32,
}

async fn switch_all(
    State(state): State<AppState>,
    Json(body): Json<SwitchAllRequest>,
) -> Result<impl IntoResponse, AppError> {
    let output = state.sim.switch_all(body.app_order).await?;

    let mut results = Vec::new();
    for line in output.lines() {
        if line.starts_with("[OK]") {
            results.push(serde_json::json!({"status": "ok", "message": line}));

            // Update DB for successful switches
            // Line format: "[OK] SERIAL | 切換成功 app_order=N (LABLE) 目前: LABLE"
            if let Some(serial) = line.strip_prefix("[OK] ") {
                if let Some(idx) = serial.find(" |") {
                    let device_id = &serial[..idx];
                    let conn = state.db.conn().lock().await;
                    let phone = conn.query_row(
                        "SELECT phone_number FROM sim_cards WHERE device_id = ?1 AND app_order = ?2",
                        rusqlite::params![device_id, body.app_order],
                        |row| row.get::<_, String>(0),
                    ).ok();
                    drop(conn);
                    if let Some(phone) = phone {
                        let _ = state.db.update_device_current(device_id, &phone, Some(body.app_order as i32)).await;
                    }
                }
            }
        } else if line.starts_with("[FAIL]") {
            results.push(serde_json::json!({"status": "fail", "message": line}));
        } else if line.starts_with("[ERROR]") {
            results.push(serde_json::json!({"status": "error", "message": line}));
        } else if line.starts_with("[SKIP]") {
            results.push(serde_json::json!({"status": "skip", "message": line}));
        }
    }

    let ok_count = results.iter().filter(|r| r["status"] == "ok").count();
    let fail_count = results.iter().filter(|r| r["status"] != "ok" && r["status"] != "skip").count();

    Ok(Json(serde_json::json!({
        "ok": fail_count == 0 && ok_count > 0,
        "success": ok_count,
        "failed": fail_count,
        "results": results,
    })))
}

async fn switch_by_phone(
    State(state): State<AppState>,
    Path(phone): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let devices = state.sim.load_devices().await?;

    for dev in &devices {
        for card in &dev.card {
            if card.phone_number == phone {
                let app_order = card.app_order.as_u64()
                    .ok_or_else(|| AppError::Sim("Invalid app_order".into()))? as u32;
                let output = state.sim.switch_device(&dev.device_id, app_order).await?;

                // Update DB if succeeded
                if output.contains("[OK]") {
                    let _ = state.db.update_device_current(&dev.device_id, &phone, Some(app_order as i32)).await;
                }

                return Ok(Json(serde_json::json!({
                    "ok": true,
                    "device_id": dev.device_id,
                    "app_order": app_order,
                    "phone_number": phone,
                    "output": output
                })));
            }
        }
    }

    Err(AppError::NotFound(format!("Phone number {} not found", phone)))
}

// ---- SMS ----

async fn receive_sms(
    State(state): State<AppState>,
    Json(mut body): Json<NewSms>,
) -> Result<impl IntoResponse, AppError> {
    // If body contains raw message, parse it
    // Format:
    //   line 1: sender
    //   line 2~: message content (until "Receiver:" line)
    //   Receiver: {app_lable}
    //   next line: ignored
    //   next line: datetime
    //   last line: ignored
    if let Some(raw) = &body.body {
        let lines: Vec<&str> = raw.lines().collect();
        if lines.len() >= 5 {
            // Check if any line starts with "Receiver:"
            if let Some(recv_idx) = lines.iter().position(|l| l.starts_with("Receiver:")) {
                // sender = first line
                if body.sender.is_none() {
                    body.sender = Some(lines[0].trim().to_string());
                }

                // message content = lines between sender and Receiver
                let content = lines[1..recv_idx].join("\n");

                // app_lable from "Receiver: XXXXX"
                let app_lable = lines[recv_idx]
                    .strip_prefix("Receiver:")
                    .unwrap_or("")
                    .trim();

                // datetime = 2 lines after Receiver
                if recv_idx + 2 < lines.len() && body.received_at.is_none() {
                    body.received_at = Some(lines[recv_idx + 2].trim().to_string());
                }

                // Lookup app_lable in DB → get device_id and phone_number
                if !app_lable.is_empty() {
                    let conn = state.db.conn().lock().await;
                    let result = conn.query_row(
                        "SELECT device_id, phone_number FROM sim_cards WHERE app_lable = ?1",
                        rusqlite::params![app_lable],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    );
                    drop(conn);

                    if let Ok((device_id, phone_number)) = result {
                        body.device_id = Some(device_id);
                        body.phone_number = Some(phone_number);
                    }
                }

                // Fallback: if Receiver was empty, try mobile_tag (line after Receiver)
                if body.device_id.is_none() && recv_idx + 1 < lines.len() {
                    let mobile_tag = lines[recv_idx + 1].trim();
                    if mobile_tag.starts_with("mobile") {
                        if let Ok(Some(device_id)) = state.db.get_device_by_mobile_tag(mobile_tag).await {
                            body.device_id = Some(device_id);
                        }
                    }
                }

                // Also try: if no Receiver line at all but has mobile tag
                // (handled below after this block)

                // Store parsed content as body, keep raw in raw_body
                body.raw_body = body.body.clone();
                body.body = Some(content);
            } else {
                // No "Receiver:" line — try format: sender\ncontent\nmobileXX\ndatetime\ngroup
                if let Some(tag_idx) = lines.iter().position(|l| l.trim().starts_with("mobile")) {
                    if body.sender.is_none() {
                        body.sender = Some(lines[0].trim().to_string());
                    }
                    let content = lines[1..tag_idx].join("\n");
                    let mobile_tag = lines[tag_idx].trim();

                    // datetime = next line after mobile tag
                    if tag_idx + 1 < lines.len() && body.received_at.is_none() {
                        body.received_at = Some(lines[tag_idx + 1].trim().to_string());
                    }

                    // Lookup device_id by mobile_tag
                    if let Ok(Some(device_id)) = state.db.get_device_by_mobile_tag(mobile_tag).await {
                        body.device_id = Some(device_id);
                    }

                    body.raw_body = body.body.clone();
                    body.body = Some(content);
                }
            }
        }
    }

    // If we have device_id but no phone_number, lookup current_phone from devices table
    if body.device_id.is_some() && body.phone_number.is_none() {
        if let Some(did) = &body.device_id {
            let conn = state.db.conn().lock().await;
            let phone = conn.query_row(
                "SELECT current_phone FROM devices WHERE device_id = ?1",
                rusqlite::params![did],
                |row| row.get::<_, String>(0),
            ).ok();
            drop(conn);
            body.phone_number = phone;
        }
    }

    let id = state.db.insert_sms(&body).await?;

    // Broadcast to all SSE clients
    let _ = state.events.send(crate::events::Event::Sms(crate::events::SmsPayload {
        id,
        device_id: body.device_id.clone(),
        phone_number: body.phone_number.clone(),
        sender: body.sender.clone(),
        body: body.body.clone(),
        received_at: body.received_at.clone(),
    }));

    Ok(Json(serde_json::json!({"ok": true, "id": id})))
}

#[derive(Deserialize)]
struct SmsQuery {
    limit: Option<u32>,
}

async fn get_sms(
    State(state): State<AppState>,
    Path(phone): Path<String>,
    Query(query): Query<SmsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(5);
    let messages = state.db.get_sms_by_phone(&phone, limit).await?;
    Ok(Json(messages))
}

async fn get_sms_by_device(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    Query(query): Query<SmsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(5);
    let messages = state.db.get_sms_by_device(&device_id, limit).await?;
    Ok(Json(messages))
}
