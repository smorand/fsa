//! The shapes of the registry (SPEC-0102 design 5.1). An answer struct never
//! has a secret field, only `has_secret` (DEC-005).

use super::secrets::SecretText;
use crate::errors::{Result, ToolError};
use base64::Engine;
use serde::Serialize;
use serde_json::{Value, json};

pub const AUTH_NONE: &str = "none";
pub const AUTH_BEARER: &str = "bearer";

/// A provider as every surface answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provider {
    pub provider_id: String,
    pub name: String,
    pub endpoint: String,
    pub auth_scheme: String,
    pub has_secret: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub const KINDS: [&str; 5] = ["text_embedding", "image_embedding", "reranker", "ocr", "chat"];

/// A model as every surface answers it; `is_default` is computed from the
/// `default_model` table, never stored on the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Model {
    pub model_id: String,
    pub provider_id: String,
    pub name: String,
    pub kind: String,
    pub dimension: Option<i64>,
    pub max_input_tokens: Option<i64>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// The two server defaults: `admin.get_default_models`' answer, `null` for a
/// kind with none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Defaults {
    pub text_embedding: Option<Model>,
    pub reranker: Option<Model>,
}

/// The kinds that can have a server default (FR-NEW-013).
pub const DEFAULT_KINDS: [&str; 2] = ["text_embedding", "reranker"];

/// `admin.create_model`'s input.
#[derive(Debug, Clone, Default)]
pub struct ModelInput {
    pub provider_id: String,
    pub name: String,
    pub kind: String,
    pub dimension: Option<i64>,
    pub max_input_tokens: Option<i64>,
}

/// `admin.update_model`'s input: `None` keeps the current value, a
/// `max_input_tokens` of 0 removes the limit.
#[derive(Debug, Clone, Default)]
pub struct ModelUpdate {
    pub name: Option<String>,
    pub dimension: Option<i64>,
    pub max_input_tokens: Option<i64>,
}

/// One row of the record of changes (FR-NEW-018).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub change_id: String,
    pub at: String,
    pub person: String,
    pub action: String,
    pub target_kind: String,
    pub target_id: String,
    pub target_name: String,
    pub fields: Vec<String>,
}

/// `admin.create_provider`'s input.
#[derive(Debug, Clone, Default)]
pub struct ProviderInput {
    pub name: String,
    pub endpoint: String,
    pub auth_scheme: String,
    pub secret: SecretText,
}

/// `admin.update_provider`'s input: `None` keeps the current value.
#[derive(Debug, Clone, Default)]
pub struct ProviderUpdate {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub auth_scheme: Option<String>,
    pub secret: Option<SecretText>,
    pub clear_secret: bool,
}

/// One page of a listing, with the marker of the next one when more follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub marker: Option<String>,
}

impl<T: Serialize> Page<T> {
    /// `{<key>: [...], marker?}`: the marker is absent when none remain.
    pub fn to_json(&self, key: &str) -> Value {
        let mut v = json!({ key: self.items });
        if let Some(m) = &self.marker {
            v["marker"] = json!(m);
        }
        v
    }
}

/// Base64 url safe without padding of the JSON `[<sort key>, <id>]` (design 5.1).
pub fn encode_marker(sort_key: &str, id: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!([sort_key, id]).to_string())
}

/// Any decoding or shape failure is the one refusal of the contract.
pub fn decode_marker(raw: &str) -> Result<(String, String)> {
    let parsed = || -> Option<(String, String)> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw).ok()?;
        let v: Value = serde_json::from_slice(&bytes).ok()?;
        let arr = v.as_array().filter(|a| a.len() == 2)?;
        Some((arr[0].as_str()?.to_string(), arr[1].as_str()?.to_string()))
    };
    parsed().ok_or_else(|| ToolError::invalid_argument("marker is not valid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_round_trips_and_garbage_is_refused_with_one_text() {
        let m = encode_marker("local gpu", "prov-0123456789ab");
        assert_eq!(decode_marker(&m).unwrap(), ("local gpu".into(), "prov-0123456789ab".into()));
        for bad in ["%%", "bm90IGpzb24", &encode_marker("a", "b")[..3]] {
            let e = decode_marker(bad).unwrap_err();
            assert_eq!(e.to_string(), "ERR_INVALID_ARGUMENT: marker is not valid");
        }
    }
}
