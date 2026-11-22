//! HTTP embedding call to an OpenAI-compatible `/v1/embeddings` endpoint.
//!
//! The endpoint contract:
//! - POST `{"model": "...", "input": ["text"]}`
//! - Response `{"data": [{"embedding": [float, ...], "index": 0}], ...}`
//!
//! Where to call is resolved at call time through [`EmbedTargets`] (SPEC-0102
//! DEC-007): the registry's default text embedding model when one is set, else
//! the YAML block. Secrets travel as [`SecretText`], never in a struct that
//! could be logged. Embedding vectors are never logged.
//!
//! Compiled in every build because `build_backend` and the settings registry
//! name the trait; only the vector engines of feature `rag` call [`embed`].

use crate::config::EmbeddingConfig;
use crate::errors::{Result, ToolError};
use crate::settings::SecretText;
use reqwest::Client;
use serde_json::Value;

/// Where one embedding call goes: the full URL, the model name and the bearer
/// token, if any.
#[derive(Debug, Clone)]
pub struct EmbedTarget {
    pub url: String,
    pub model: String,
    pub bearer: Option<SecretText>,
}

/// The source of the embedding target, asked once per index or query call so a
/// registry change applies at once (DEC-007: no cache).
#[async_trait::async_trait]
pub trait EmbedTargets: Send + Sync {
    async fn target(&self) -> Result<EmbedTarget>;
}

/// The YAML block as the target: the endpoint is the full URL, as before the
/// registry existed, and the key is read from `api_key_env` at call time.
pub struct StaticTargets(pub EmbeddingConfig);

impl StaticTargets {
    pub fn target_of(config: &EmbeddingConfig) -> EmbedTarget {
        let bearer = if config.api_key_env.is_empty() {
            None
        } else {
            std::env::var(&config.api_key_env).ok().map(SecretText::new)
        };
        EmbedTarget { url: config.endpoint.clone(), model: config.model.clone(), bearer }
    }
}

#[async_trait::async_trait]
impl EmbedTargets for StaticTargets {
    async fn target(&self) -> Result<EmbedTarget> {
        Ok(Self::target_of(&self.0))
    }
}

/// Call the embedding endpoint and return the embedding for `text`.
///
/// `client` is the shared `reqwest::Client`; build it once at boot.
pub async fn embed(client: &Client, target: &EmbedTarget, text: &str) -> Result<Vec<f32>> {
    let mut req = client.post(&target.url).header("Content-Type", "application/json");

    if let Some(key) = &target.bearer {
        req = req.header("Authorization", format!("Bearer {}", key.expose()));
    }

    let body = serde_json::json!({
        "model": target.model,
        "input": [text],
    });

    let resp = req.json(&body).send().await.map_err(|e| {
        ToolError::internal(format!("embedding request failed: {}", e.without_url()))
    })?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        return Err(ToolError::internal(format!("embedding endpoint returned HTTP {status}")));
    }

    let json: Value = resp.json().await.map_err(|e| {
        ToolError::internal(format!("parse embedding response: {}", e.without_url()))
    })?;

    let embedding = json
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|e| e.get("embedding"))
        .and_then(|e| e.as_array())
        .ok_or_else(|| ToolError::internal("embedding response missing data[0].embedding array"))?;

    let floats: Vec<f32> = embedding.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect();

    Ok(floats)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A token in the query string of an endpoint must not reach the error text.
    #[tokio::test]
    async fn a_failed_call_does_not_echo_the_endpoint_query() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let target = EmbedTarget {
            url: format!("http://127.0.0.1:{port}/v1?token=abc123"),
            model: "m".into(),
            bearer: None,
        };
        let err = embed(&Client::new(), &target, "x").await.unwrap_err();
        assert!(!err.to_string().contains("abc123"), "{err}");
    }
}
