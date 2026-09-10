use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode, Uri},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use rust_embed::Embed;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::config::{get_vars_path, read_vars_nix, save_vars_nix, ChomiamConfig};
use crate::generations::{list_generations, GenerationsSummary};
use crate::system::{SystemCollector, SystemMetrics};
use crate::updates::{check_updates, spawn_command_stream, UpdateCheck};

#[derive(Embed)]
#[folder = "frontend/"]
pub struct FrontendAssets;

#[derive(Clone)]
pub struct AppState {
    pub collector: Arc<Mutex<SystemCollector>>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/system/metrics", get(get_system_metrics))
        .route("/api/system/updates", get(get_update_status))
        .route("/api/generations", get(get_nix_generations))
        .route("/api/config", get(get_chomiam_config))
        .route("/api/config/save", post(save_chomiam_config))
        .route("/api/action/stream", get(stream_action))
        .fallback(static_handler)
        .with_state(state)
}

async fn get_system_metrics(State(state): State<AppState>) -> Json<SystemMetrics> {
    let mut collector = state.collector.lock().await;
    let metrics = collector.collect();
    Json(metrics)
}

async fn get_update_status() -> Json<UpdateCheck> {
    Json(check_updates().await)
}

async fn get_nix_generations() -> Result<Json<GenerationsSummary>, (StatusCode, String)> {
    list_generations()
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn get_chomiam_config() -> Result<Json<ChomiamConfig>, (StatusCode, String)> {
    let path = get_vars_path();
    read_vars_nix(&path)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn save_chomiam_config(
    Json(payload): Json<ChomiamConfig>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = get_vars_path();
    save_vars_nix(&path, &payload)
        .map(|_| Json(serde_json::json!({ "success": true, "message": "Configuration /etc/nixos/vars.nix sauvegardée" })))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(serde::Deserialize)]
struct ActionQuery {
    action: String,
}

async fn stream_action(
    Query(query): Query<ActionQuery>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let stream = spawn_command_stream(&query.action)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let event_stream = stream.map(|res| match res {
        Ok(line) => Ok(Event::default().data(line)),
        Err(e) => Ok(Event::default().data(format!("Error: {}", e))),
    });

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()))
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match FrontendAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())], content.data).into_response()
        }
        None => match FrontendAssets::get("index.html") {
            Some(content) => ([(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))], content.data).into_response(),
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        },
    }
}
