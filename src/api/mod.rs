pub mod devices;
pub mod frontend;
pub mod sim;
pub mod stream;

use axum::Router;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(devices::router())
        .merge(stream::router())
        .merge(sim::router())
        .merge(frontend::router())
}
