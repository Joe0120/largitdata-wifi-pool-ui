use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::AppError;
use crate::scrcpy::protocol;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/devices/{serial}/stream", get(stream_handler))
}

async fn stream_handler(
    State(state): State<AppState>,
    Path(serial): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_stream(state, serial, socket))
}

async fn handle_stream(state: AppState, serial: String, socket: WebSocket) {
    let session = match state.scrcpy.get_or_create(&serial).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to start scrcpy for {serial}: {e}");
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let width = session.resolution_width;
    let height = session.resolution_height;

    // Task 1: video stream → WebSocket (binary)
    let video_session = session.clone();
    let serial_clone = serial.clone();
    let video_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 1024 * 1024]; // 1MB buffer
        tracing::debug!("Acquiring video lock for {serial_clone}...");
        let mut stream = video_session.video_stream.lock().await;
        tracing::debug!("Video lock acquired for {serial_clone}, starting read loop");
        loop {
            match stream.read(&mut buf).await {
                Ok(0) => {
                    tracing::warn!("Video EOF for {serial_clone}");
                    break;
                }
                Ok(n) => {
                    if ws_sender
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                        .is_err()
                    {
                        tracing::debug!("WebSocket send failed for {serial_clone}");
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Video read error for {serial_clone}: {e}");
                    break;
                }
            }
        }
    });

    // Task 2: WebSocket (JSON text) → control stream (binary)
    let control_session = session.clone();
    let control_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Err(e) =
                        handle_control_message(&control_session, &text, width, height).await
                    {
                        tracing::warn!("Control error: {e}");
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = video_task => {},
        _ = control_task => {},
    }

    tracing::info!("WebSocket closed for {serial}");
}

async fn handle_control_message(
    session: &crate::scrcpy::server::ScrcpySession,
    text: &str,
    width: u32,
    height: u32,
) -> Result<(), AppError> {
    let msg: serde_json::Value =
        serde_json::from_str(text).map_err(|e| AppError::Adb(format!("Invalid JSON: {e}")))?;

    let msg_type = msg["type"].as_str().unwrap_or("");

    let mut control = session.control_stream.lock().await;

    match msg_type {
        "touchDown" | "touchMove" | "touchUp" => {
            let xp = msg["xP"].as_f64().unwrap_or(0.0);
            let yp = msg["yP"].as_f64().unwrap_or(0.0);
            let x = (xp * width as f64) as u32;
            let y = (yp * height as f64) as u32;
            let action = match msg_type {
                "touchDown" => 0,
                "touchUp" => 1,
                "touchMove" => 2,
                _ => unreachable!(),
            };
            let packet = protocol::build_touch_event(action, x, y, width as u16, height as u16);
            control.write_all(&packet).await?;
        }
        "keyEvent" => {
            let keycode = msg["data"]["eventNumber"].as_u64().unwrap_or(0) as u32;
            // Send key down + key up
            let down = protocol::build_key_event(0, keycode);
            let up = protocol::build_key_event(1, keycode);
            control.write_all(&down).await?;
            control.write_all(&up).await?;
        }
        "ping" => {
            // Pong is handled at WebSocket level, nothing to do
        }
        _ => {
            tracing::debug!("Unknown control message type: {msg_type}");
        }
    }

    Ok(())
}

use futures_util::{SinkExt, StreamExt};
