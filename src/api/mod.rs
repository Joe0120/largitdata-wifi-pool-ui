pub mod devices;
pub mod events;
pub mod frontend;
pub mod openapi;
pub mod sim;
pub mod stream;

use axum::Router;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(devices::router())
        .merge(stream::router())
        .merge(sim::router())
        .merge(events::router())
        .merge(openapi::router())
        .merge(frontend::router())
}
