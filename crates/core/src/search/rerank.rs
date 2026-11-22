//! HTTP rerank call to a Cohere/Jina-compatible rerank endpoint.
//!
//! Request: POST `{"model": "...", "query": "...", "documents": ["text1", ...], "top_n": n}`
//! Response: `{"results": [{"index": 0, "relevance_score": 0.9}, ...]}`
//!
//! Where to call is resolved at call time through [`RerankTargets`] (SPEC-0102
//! DEC-007, DEC-008): the registry's default reranker, with no YAML fallback
//! because nothing calls a configured reranker. The bearer travels as
//! [`SecretText`], never in a struct that could be logged.

use crate::errors::{Result, ToolError};
use crate::search::SearchResult;
use crate::settings::SecretText;
use reqwest::Client;
use serde_json::Value;

/// Where one rerank call goes: the full URL, the model name and the bearer
/// token, if any.
#[derive(Debug, Clone)]
pub struct RerankTarget {
    pub url: String,
    pub model: String,
    pub bearer: Option<SecretText>,
}

/// The source of the rerank target, asked once per query call so a registry
/// change applies at once (no cache). `None`: no default reranker, no call.
#[async_trait::async_trait]
pub trait RerankTargets: Send + Sync {
    async fn target(&self) -> Result<Option<RerankTarget>>;
}

/// Rerank `results` by relevance to `query` through `target`.
///
/// Returns at most `top_n` results in score order with freshly assigned rank
/// indices; the cut to `top_n` is made here, whatever the endpoint returns.
/// An HTTP error status keeps the given order (non fatal).
pub async fn rerank(
    client: &Client,
    target: &RerankTarget,
    query: &str,
    results: Vec<SearchResult>,
    top_n: usize,
) -> Result<Vec<SearchResult>> {
    if results.is_empty() {
        return Ok(results);
    }

    let n = top_n;
    let documents: Vec<&str> = results.iter().map(|r| r.chunk.as_str()).collect();

    let mut req = client.post(&target.url).header("Content-Type", "application/json");

    if let Some(key) = &target.bearer {
        req = req.header("Authorization", format!("Bearer {}", key.expose()));
    }

    let body = serde_json::json!({
        "model": target.model,
        "query": query,
        "documents": documents,
        "top_n": n,
    });

    let resp =
        req.json(&body).send().await.map_err(|e| {
            ToolError::internal(format!("rerank request failed: {}", e.without_url()))
        })?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        // Reranking failures are non-fatal: return the original order.
        tracing::warn!(status, "rerank endpoint returned HTTP {status}, using original order");
        return Ok(results);
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| ToolError::internal(format!("parse rerank response: {}", e.without_url())))?;

    let reranked = json
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| ToolError::internal("rerank response missing results array"))?;

    let mut out: Vec<SearchResult> = reranked
        .iter()
        .filter_map(|entry| {
            let idx = entry.get("index")?.as_u64()? as usize;
            let score = entry.get("relevance_score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
            results.get(idx).map(|r| SearchResult {
                path: r.path.clone(),
                score,
                chunk: r.chunk.clone(),
                rank: 0, // assigned below
                chunk_idx: r.chunk_idx,
            })
        })
        .take(n)
        .collect();

    for (i, r) in out.iter_mut().enumerate() {
        r.rank = i + 1;
    }

    Ok(out)
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
        let target = RerankTarget {
            url: format!("http://127.0.0.1:{port}/v1?token=abc123"),
            model: "m".into(),
            bearer: None,
        };
        let one = SearchResult {
            path: "/a".into(),
            score: 0.0,
            chunk: "x".into(),
            rank: 1,
            chunk_idx: 0,
        };
        let err = rerank(&Client::new(), &target, "q", vec![one], 1).await.unwrap_err();
        assert!(!err.to_string().contains("abc123"), "{err}");
    }
}
