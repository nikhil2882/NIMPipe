use crate::config::{self, AppConfig};
use crate::models::{self, ModelRegistryFile, SharedRegistry};
use crate::proxy::ProxyClient;
use crate::transform::transform_request;
use anyhow::Context;
use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post, put},
};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, warn};

static UI_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/ui");

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub registry: SharedRegistry,
    pub proxy: Arc<ProxyClient>,
    pub recent_events: Arc<RwLock<VecDeque<RecentEvent>>>,
}

#[derive(Clone, Serialize)]
pub struct RecentEvent {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Serialize)]
struct ModelsResponse {
    object: String,
    data: Vec<OpenAiModel>,
}

#[derive(Serialize)]
struct OpenAiModel {
    id: String,
    object: String,
    created: i64,
    owned_by: String,
}

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/assets/{*path}", get(serve_asset))
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/admin/models", get(admin_list_models))
        .route("/admin/models", put(admin_update_models))
        .route("/admin/test", post(admin_test_model))
        .route("/admin/reload", post(admin_reload))
        .route("/admin/config", get(admin_get_config))
        .route("/admin/events", get(admin_events))
        .with_state(state)
}

async fn serve_index() -> impl IntoResponse {
    match UI_DIR.get_file("index.html") {
        Some(file) => Html(file.contents_utf8().unwrap_or_default()).into_response(),
        None => (StatusCode::NOT_FOUND, "UI not found").into_response(),
    }
}

async fn serve_asset(axum::extract::Path(path): axum::extract::Path<String>) -> impl IntoResponse {
    let file = UI_DIR.get_file(&path);
    match file {
        Some(f) => {
            let content_type = match std::path::Path::new(&path).extension() {
                Some(ext) if ext == "js" => "application/javascript",
                Some(ext) if ext == "css" => "text/css",
                _ => "application/octet-stream",
            };
            Response::builder()
                .header("Content-Type", content_type)
                .body(axum::body::Body::from(f.contents().to_vec()))
                .unwrap()
        }
        None => (StatusCode::NOT_FOUND, "Asset not found").into_response(),
    }
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().unwrap();
    Json(serde_json::json!({
        "status": "ok",
        "port": cfg.server.port,
        "debug_mode": cfg.logging.debug_mode,
    }))
}

async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    let registry = state.registry.read().unwrap();
    let now = chrono::Utc::now().timestamp();
    let models: Vec<_> = registry
        .list()
        .into_iter()
        .map(|m| OpenAiModel {
            id: m.openai_id.clone(),
            object: "model".to_string(),
            created: now,
            owned_by: "nimpipe".to_string(),
        })
        .collect();

    Json(ModelsResponse {
        object: "list".to_string(),
        data: models,
    })
}

#[axum::debug_handler]
async fn chat_completions(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let request_id = uuid::Uuid::new_v4().to_string();
    let start = std::time::Instant::now();

    let model_id = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let stream = payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let payload_size = serde_json::to_string(&payload).map(|s| s.len()).unwrap_or(0);

    info!(
        request_id = %request_id,
        model = %model_id,
        stream = %stream,
        payload_bytes = payload_size,
        "REQUEST_START"
    );

    // Log full body ONLY to file (skip stdout by using debug level for big payloads)
    debug!(
        request_id = %request_id,
        body = %serde_json::to_string(&payload).unwrap_or_default(),
        "INCOMING_BODY"
    );

    let model = {
        let registry = state.registry.read().unwrap();
        match registry.get(&model_id) {
            Some(m) => m.clone(),
            None => {
                warn!(request_id = %request_id, model = %model_id, "Unknown model requested");
                return openai_error(
                    StatusCode::NOT_FOUND,
                    &format!("Model '{}' not found", model_id),
                );
            }
        }
    };

    if stream && !model.supports_streaming {
        return openai_error(StatusCode::BAD_REQUEST, "Model does not support streaming");
    }

    let transform_start = std::time::Instant::now();
    let upstream_body = match transform_request(payload, &model) {
        Ok(b) => b,
        Err(e) => {
            error!(request_id = %request_id, error = %e, "TRANSFORM_ERROR");
            return openai_error(StatusCode::BAD_REQUEST, &e.to_string());
        }
    };
    let transform_ms = transform_start.elapsed().as_millis();

    info!(
        request_id = %request_id,
        backend_model = %model.backend_id,
        transform_ms = transform_ms,
        upstream_bytes = serde_json::to_string(&upstream_body).map(|s| s.len()).unwrap_or(0),
        "TRANSFORMED"
    );

    debug!(
        request_id = %request_id,
        body = %serde_json::to_string(&upstream_body).unwrap_or_default(),
        "UPSTREAM_BODY"
    );

    log_event(
        &state,
        "INFO",
        &format!("[{request_id}] chat_completion model={model_id} stream={stream}"),
    );

    let upstream_start = std::time::Instant::now();
    match state
        .proxy
        .chat_completion(upstream_body, stream, model.status_poll_path.as_deref())
        .await
    {
        Ok(res) => {
            let upstream_ms = upstream_start.elapsed().as_millis();
            let total_ms = start.elapsed().as_millis();
            info!(
                request_id = %request_id,
                upstream_ms = upstream_ms,
                total_ms = total_ms,
                "REQUEST_COMPLETE"
            );
            res
        }
        Err(e) => {
            let upstream_ms = upstream_start.elapsed().as_millis();
            let total_ms = start.elapsed().as_millis();
            error!(
                request_id = %request_id,
                error = %e,
                upstream_ms = upstream_ms,
                total_ms = total_ms,
                "PROXY_ERROR"
            );
            openai_error(StatusCode::BAD_GATEWAY, &e.to_string())
        }
    }
}

async fn admin_list_models(State(state): State<AppState>) -> impl IntoResponse {
    let registry = state.registry.read().unwrap();
    let file = registry.to_file();
    json_response(Ok(file))
}

#[derive(Deserialize)]
struct UpdateModelsPayload {
    content: String,
}

async fn admin_update_models(
    State(state): State<AppState>,
    Json(payload): Json<UpdateModelsPayload>,
) -> impl IntoResponse {
    let result: anyhow::Result<ModelRegistryFile> = (|| {
        let file: ModelRegistryFile = toml::from_str(&payload.content).context("Invalid TOML")?;
        let registry = models::ModelRegistry::from_file(&file)?;
        models::save_registry(&registry)?;
        Ok(file)
    })();

    match result {
        Ok(file) => {
            if let Ok(mut reg) = state.registry.write() {
                if let Ok(new_reg) = models::ModelRegistry::from_file(&file) {
                    *reg = new_reg;
                }
            }
            json_response(Ok(serde_json::json!({"saved": true })))
        }
        Err(e) => {
            warn!("Failed to update models: {}", e);
            json_response::<Value>(Err(e))
        }
    }
}

#[derive(Deserialize)]
struct TestModelPayload {
    openai_id: String,
}

#[axum::debug_handler]
async fn admin_test_model(
    State(state): State<AppState>,
    Json(payload): Json<TestModelPayload>,
) -> impl IntoResponse {
    let model = {
        let registry = state.registry.read().unwrap();
        match registry.get(&payload.openai_id) {
            Some(m) => m.clone(),
            None => return json_response::<Value>(Err(anyhow::anyhow!("Model not found"))),
        }
    };

    match state.proxy.test_call(&model.backend_id).await {
        Ok(text) => json_response(Ok(serde_json::json!({ "response": text }))),
        Err(e) => {
            warn!("Test call failed for {}: {}", model.openai_id, e);
            json_response::<Value>(Err(e))
        }
    }
}

async fn admin_reload(State(state): State<AppState>) -> impl IntoResponse {
    let result: anyhow::Result<Value> = async {
        let cfg = config::load_config()?;
        let registry = models::load_registry()?;
        if let (Ok(mut c), Ok(mut r)) = (state.config.write(), state.registry.write()) {
            *c = cfg.clone();
            *r = registry;
        }
        Ok(serde_json::json!({ "reloaded": true }))
    }
    .await;

    json_response(result)
}

async fn admin_get_config(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().unwrap();
    json_response(Ok(cfg.clone()))
}

async fn admin_events(State(state): State<AppState>) -> impl IntoResponse {
    let events = state.recent_events.read().unwrap();
    let data: Vec<_> = events.iter().cloned().collect();
    json_response(Ok(data))
}

fn json_response<T: Serialize>(result: anyhow::Result<T>) -> impl IntoResponse {
    match result {
        Ok(data) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(data),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<T> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        )
            .into_response(),
    }
}

fn openai_error(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": "nimpipe_error",
            "code": status.as_u16(),
        }
    });
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

fn log_event(state: &AppState, level: &str, message: &str) {
    if let Ok(mut events) = state.recent_events.write() {
        events.push_back(RecentEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: level.to_string(),
            message: message.to_string(),
        });
        while events.len() > 100 {
            events.pop_front();
        }
    }
}
