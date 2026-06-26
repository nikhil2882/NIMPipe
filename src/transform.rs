use crate::models::ModelEntry;
use anyhow::{Context, Result};
use serde_json::{Map, Value};

/// Transform an incoming OpenAI-compatible chat completion request into the
/// upstream body for NVIDIA NIM, using the rules defined in the model entry.
pub fn transform_request(body: Value, model: &ModelEntry) -> Result<Value> {
    let mut body = body;

    // Ensure body is an object.
    let obj = body
        .as_object_mut()
        .context("Request body must be a JSON object")?;

    // 1. Set the upstream model ID.
    obj.insert("model".to_string(), Value::String(model.backend_id.clone()));

    // 2. Strip unwanted parameters.
    for key in &model.strip_params {
        obj.remove(key);
    }

    // 3. Apply default parameters if missing.
    for (key, value) in &model.default_params {
        if !obj.contains_key(key) {
            obj.insert(key.clone(), value.clone());
        }
    }

    // 4. Clamp max_tokens if cap is defined.
    if let Some(cap) = model.max_tokens_cap {
        if let Some(Value::Number(n)) = obj.get("max_tokens") {
            if let Some(v) = n.as_u64() {
                let clamped = v.min(cap as u64);
                obj.insert("max_tokens".to_string(), Value::Number(clamped.into()));
            }
        }
    }

    // 5. Merge injected parameters, including nested dot-notation keys.
    for (key, value) in &model.injected_params {
        set_nested(obj, key, value.clone());
    }

    Ok(Value::Object(obj.clone()))
}

/// Set a possibly dotted key like `chat_template_kwargs.thinking` into a JSON object.
fn set_nested(obj: &mut Map<String, Value>, key: &str, value: Value) {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.is_empty() {
        return;
    }

    let mut current = obj;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            current.insert(part.to_string(), value.clone());
        } else {
            let next = current
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            current = next
                .as_object_mut()
                .expect("injected param path conflicts with non-object value");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_model() -> ModelEntry {
        ModelEntry {
            openai_id: "kimi-k2.6-thinking".to_string(),
            backend_id: "moonshotai/kimi-k2.6".to_string(),
            description: String::new(),
            max_tokens_cap: Some(100),
            default_params: [("temperature".to_string(), json!(0.7))]
                .into_iter()
                .collect(),
            injected_params: [("chat_template_kwargs.thinking".to_string(), json!(true))]
                .into_iter()
                .collect(),
            strip_params: vec!["seed".to_string()],
            supports_streaming: true,
            supports_tools: true,
            status_poll_path: None,
        }
    }

    #[test]
    fn maps_model_alias_and_injects_nested_param() {
        let model = sample_model();
        let body = json!({
            "model": "kimi-k2.6-thinking",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 200,
            "seed": 42,
        });

        let out = transform_request(body, &model).unwrap();
        assert_eq!(out["model"], "moonshotai/kimi-k2.6");
        assert_eq!(out["max_tokens"], 100);
        assert!(out.get("seed").is_none());
        assert_eq!(out["temperature"], 0.7);
        assert_eq!(out["chat_template_kwargs"]["thinking"], true);
    }
}
