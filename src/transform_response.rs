use axum::body::Body;
use bytes::Bytes;
use futures_core::Stream;
use serde_json::Value;
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing::{info, warn};

/// Transforms MiniMax M3 thinking streaming responses:
///   - Maps `reasoning_content` → `content` in SSE delta when content is missing/null
///   - Strips NVIDIA SSE comment lines (`: {...}`) that carry usage metadata
///   - Logs transform statistics on stream completion
///
/// The transform is model-agnostic — applied whenever these patterns appear.
pub fn transform_stream(
    source: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> Body {
    Body::from_stream(SseTransformStream {
        inner: Box::pin(source),
        buf: Vec::new(),
        done: false,
        chunks: 0,
        reasoning_mapped: 0,
        comments_stripped: 0,
        errors: 0,
    })
}

struct SseTransformStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    buf: Vec<u8>,
    done: bool,
    chunks: u64,
    reasoning_mapped: u64,
    comments_stripped: u64,
    errors: u64,
}

impl Stream for SseTransformStream {
    type Item = Result<Bytes, axum::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }

        loop {
            match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    self.buf.extend_from_slice(&chunk);

                    if let Some(newline_pos) = find_sse_event_end(&self.buf) {
                        let event_bytes = self.buf.drain(..newline_pos + 2).collect::<Vec<_>>();
                        let (transformed, reasonings, stripped) = transform_sse_event_counted(&event_bytes);
                        self.chunks += 1;
                        self.reasoning_mapped += reasonings;
                        self.comments_stripped += stripped;
                        return Poll::Ready(Some(Ok(Bytes::from(transformed))));
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    self.done = true;
                    self.errors += 1;
                    warn!(
                        chunks = self.chunks,
                        reasoning_mapped = self.reasoning_mapped,
                        comments_stripped = self.comments_stripped,
                        errors = self.errors,
                        "SSE_STREAM_ERROR"
                    );
                    if !self.buf.is_empty() {
                        let remaining = std::mem::take(&mut self.buf);
                        return Poll::Ready(Some(Ok(Bytes::from(remaining))));
                    }
                    return Poll::Ready(Some(Err(axum::Error::new(e))));
                }
                Poll::Ready(None) => {
                    self.done = true;
                    info!(
                        chunks = self.chunks,
                        reasoning_mapped = self.reasoning_mapped,
                        comments_stripped = self.comments_stripped,
                        "SSE_STREAM_COMPLETE"
                    );
                    if !self.buf.is_empty() {
                        let remaining = std::mem::take(&mut self.buf);
                        return Poll::Ready(Some(Ok(Bytes::from(remaining))));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// Find the position of `\n\n` (end of SSE event) in buffer
fn find_sse_event_end(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

/// Transform a single SSE event with counting for statistics.
/// Returns (transformed_bytes, reasoning_mapped_count, comments_stripped_count)
fn transform_sse_event_counted(event: &[u8]) -> (Vec<u8>, u64, u64) {
    let event_str = match std::str::from_utf8(event) {
        Ok(s) => s,
        Err(_) => return (event.to_vec(), 0, 0),
    };

    let mut out = String::with_capacity(event_str.len());
    let mut reasoning_mapped = 0u64;
    let mut comments_stripped = 0u64;

    for line in event_str.lines() {
        if line.starts_with("data:") || line.starts_with("data: ") {
            let json_str = if line.starts_with("data: ") {
                &line[6..]
            } else {
                &line[5..]
            };

            let (transformed_json, mapped) = transform_sse_data_counted(json_str);
            if mapped {
                reasoning_mapped += 1;
            }

            let prefix = if line.starts_with("data: ") { "data: " } else { "data:" };
            out.push_str(prefix);
            out.push_str(&transformed_json);
            out.push('\n');
        } else if line.starts_with(": ") || line == ":" {
            let content = if line.len() > 2 { &line[2..] } else { "" };
            if content.trim_start().starts_with('{') {
                comments_stripped += 1;
                continue;
            }
            out.push_str(line);
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    let result = if reasoning_mapped > 0 || comments_stripped > 0 {
        out.into_bytes()
    } else {
        event.to_vec()
    };

    (result, reasoning_mapped, comments_stripped)
}

/// Transform the JSON content of a "data:" SSE line.
/// Returns (transformed_json_string, was_reasoning_mapped_to_content)
fn transform_sse_data_counted(json_str: &str) -> (String, bool) {
    let mut value: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return (json_str.to_string(), false),
    };

    let choices = match value.get_mut("choices") {
        Some(Value::Array(arr)) => arr,
        _ => return (json_str.to_string(), false),
    };

    let mut changed = false;
    for choice in choices.iter_mut() {
        let delta = match choice.get_mut("delta") {
            Some(Value::Object(obj)) => obj,
            _ => continue,
        };

        let has_reasoning = delta.contains_key("reasoning_content")
            && !delta.get("reasoning_content").map_or(true, |v| v.is_null());

        let has_content = delta.contains_key("content")
            && !delta.get("content").map_or(true, |v| v.is_null());

        if has_reasoning && !has_content {
            let reasoning = delta["reasoning_content"].clone();
            delta.insert("content".to_string(), reasoning);
            changed = true;
        }
    }

    if changed {
        (serde_json::to_string(&value).unwrap_or_else(|_| json_str.to_string()), true)
    } else {
        (json_str.to_string(), false)
    }
}

#[allow(dead_code)]
fn transform_sse_data(json_str: &str) -> String {
    transform_sse_data_counted(json_str).0
}

#[allow(dead_code)]
fn transform_sse_event(event: &[u8]) -> Vec<u8> {
    transform_sse_event_counted(event).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_event_end() {
        assert_eq!(find_sse_event_end(b"hello\n\nworld"), Some(5));
        assert_eq!(find_sse_event_end(b"no double newline"), None);
    }

    #[test]
    fn test_transform_sse_data_maps_reasoning_to_content() {
        let input = r#"{"choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"hello"},"finish_reason":null}]}"#;
        let output = transform_sse_data(input);
        let v: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(v["choices"][0]["delta"]["content"], "hello");
        assert_eq!(v["choices"][0]["delta"]["reasoning_content"], "hello");
    }

    #[test]
    fn test_transform_sse_data_leaves_content_alone() {
        let input = r#"{"choices":[{"index":0,"delta":{"content":"hello","role":"assistant"},"finish_reason":null}]}"#;
        let output = transform_sse_data(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_transform_sse_data_null_content() {
        let input = r#"{"choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"hi","content":null},"finish_reason":null}]}"#;
        let output = transform_sse_data(input);
        let v: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(v["choices"][0]["delta"]["content"], "hi");
    }

    #[test]
    fn test_transform_sse_event_strips_comments() {
        let input = b"data: {\"choices\":[]}\n: {\"input_tokens\":5}\n\n";
        let output = transform_sse_event(input);
        let s = std::str::from_utf8(&output).unwrap();
        assert!(s.contains("data:"));
        assert!(!s.contains(": {"));
    }

    #[test]
    fn test_transform_sse_event_preserves_non_json_comments() {
        let input = b"data: {\"choices\":[]}\n: heartbeat\n\n";
        let output = transform_sse_event(input);
        let s = std::str::from_utf8(&output).unwrap();
        assert!(s.contains(": heartbeat"));
    }
}
