use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use rust_embed::Embed;

use crate::AppState;

#[derive(Embed)]
#[folder = "frontend/"]
struct FrontendAssets;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .fallback(get(static_file))
}

async fn index() -> impl IntoResponse {
    serve_file("index.html")
}

async fn static_file(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_owned();
    serve_file(&path)
}

fn serve_file(path: &str) -> axum::response::Response {
    match FrontendAssets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
