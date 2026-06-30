use crate::config::TimeoutsConfig;
use crate::transform_response;
use anyhow::{Context, Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use reqwest::StatusCode;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, info, warn};

const NVIDIA_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";

#[derive(Clone)]
pub struct ProxyClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    timeouts: TimeoutsConfig,
}

impl ProxyClient {
    pub fn new(api_key: String, timeouts: TimeoutsConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(
                timeouts.streaming_seconds.max(timeouts.request_seconds),
            ))
            .build()
            .context("Failed to build HTTP client")?;
        Ok(Self {
            client,
            api_key,
            base_url: NVIDIA_BASE_URL.to_string(),
            timeouts,
        })
    }

    pub async fn chat_completion(
        &self,
        body: Value,
        stream: bool,
        status_poll_path: Option<&str>,
    ) -> Result<Response> {
        let url = format!("{}/chat/completions", self.base_url);
        debug!("Upstream POST {}", url);

        let model_name = body
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let body_size = serde_json::to_string(&body).map(|s| s.len()).unwrap_or(0);

        let req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header(
                "Accept",
                if stream {
                    "text/event-stream"
                } else {
                    "application/json"
                },
            )
            .json(&body);

        if stream {
            let upstream_start = std::time::Instant::now();
            let res = req
                .timeout(Duration::from_secs(self.timeouts.streaming_seconds))
                .send()
                .await
                .map_err(|e| anyhow!("Failed to contact NVIDIA NIM (streaming): {e}"))?;

            let first_byte_ms = upstream_start.elapsed().as_millis();
            info!(
                upstream_model = %model_name,
                upstream_status = %res.status(),
                upstream_content_type = %res.headers().get("content-type").map(|v| v.to_str().unwrap_or("?")).unwrap_or("none"),
                upstream_body_bytes = body_size,
                first_byte_ms = first_byte_ms,
                "UPSTREAM_STREAM_START"
            );

            if res.status().is_success() {
                let status = res.status();
                let source = res.bytes_stream();
                let body = transform_response::transform_stream(source);
                return Ok(Response::builder()
                    .status(status)
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .body(body)
                    .unwrap());
            }

            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            warn!("Upstream streaming error {}: {}", status, text);
            return Ok(openai_error_response(status, &text));
        }

        let res = req
            .timeout(Duration::from_secs(self.timeouts.request_seconds))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to contact NVIDIA NIM (sync): {e}"))?;

        let status = res.status();
        if status.is_success() {
            let text = res.text().await.unwrap_or_default();
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(text))
                .unwrap());
        }

        if status == StatusCode::ACCEPTED {
            return self.handle_202(res, status_poll_path).await;
        }

        let text = res.text().await.unwrap_or_default();
        warn!("Upstream error {}: {}", status, text);
        Ok(openai_error_response(status, &text))
    }

    async fn handle_202(
        &self,
        res: reqwest::Response,
        status_poll_path: Option<&str>,
    ) -> Result<Response> {
        let path_template = match status_poll_path {
            Some(p) => p,
            None => {
                return Ok(openai_error_response(
                    StatusCode::BAD_GATEWAY,
                    "Upstream returned 202 but no status_poll_path configured for this model",
                ));
            }
        };

        let body: Value = res.json().await.unwrap_or_default();
        let request_id = body
            .get("requestId")
            .and_then(|v| v.as_str())
            .or_else(|| body.get("id").and_then(|v| v.as_str()))
            .ok_or_else(|| anyhow!("Upstream 202 response missing requestId"))?;

        info!("Polling status for requestId {}", request_id);

        let poll_url = format!(
            "{}{}",
            self.base_url,
            path_template.replace("{request_id}", request_id)
        );

        let mut interval_ms = self.timeouts.poll_interval_start_ms;
        let max_poll = Duration::from_secs(self.timeouts.max_poll_seconds);
        let start = std::time::Instant::now();

        loop {
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            interval_ms = (interval_ms * 2).min(self.timeouts.poll_interval_max_ms);

            if start.elapsed() > max_poll {
                return Ok(openai_error_response(
                    StatusCode::GATEWAY_TIMEOUT,
                    &format!("Polling for requestId {} timed out", request_id),
                ));
            }

            let poll_res = self
                .client
                .get(&poll_url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send()
                .await
                .context("Failed to poll NVIDIA status endpoint")?;

            let poll_status = poll_res.status();
            if poll_status.is_success() {
                let text = poll_res.text().await.unwrap_or_default();
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(text))
                    .unwrap());
            }

            if poll_status != StatusCode::ACCEPTED {
                let text = poll_res.text().await.unwrap_or_default();
                warn!("Status poll error {}: {}", poll_status, text);
                return Ok(openai_error_response(poll_status, &text));
            }

            debug!("requestId {} still pending", request_id);
        }
    }

    pub async fn test_call(&self, backend_id: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": backend_id,
            "messages": [{"role": "user", "content": "hello world"}],
            "max_tokens": 50,
        });

        let res = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .timeout(Duration::from_secs(self.timeouts.request_seconds))
            .send()
            .await
            .context("Failed to contact NVIDIA NIM")?;

        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if status.is_success() {
            Ok(text)
        } else {
            Err(anyhow!("Test call failed ({}): {}", status, text))
        }
    }
}

fn openai_error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": "upstream_error",
            "code": status.as_u16(),
        }
    });
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
