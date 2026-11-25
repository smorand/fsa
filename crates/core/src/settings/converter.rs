//! The shapes of the converter registry (SPEC-0106 design 4 and 5). An answer
//! never has a secret field, only `has_secret` (DEC-011).

use serde::Serialize;
use serde_json::Value;

pub const FORM_PROGRAM: &str = "program";
pub const FORM_HTTP: &str = "http";

/// FR-NEW-004 defaults: the answer cap and the timeout of a converter.
pub const DEFAULT_MAX_INPUT_BYTES: i64 = 536_870_912;
pub const DEFAULT_TIMEOUT_SECONDS: i64 = 900;

/// A converter as every surface answers it; a field of the other form is `null`.
/// The field order is the answer order of FR-NEW-001.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Converter {
    pub converter_id: String,
    pub name: String,
    pub form: String,
    pub command: Option<Vec<String>>,
    pub url: Option<String>,
    pub auth_header: Option<String>,
    pub has_secret: bool,
    pub file_field: Option<String>,
    pub response_field: Option<String>,
    pub extensions: Vec<String>,
    pub max_input_bytes: i64,
    pub timeout_seconds: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// `admin.create_converter`'s and `admin.update_converter`'s input, untyped so the rule module words a wrong
/// type itself (DEC-004). `None` and `Some(Value::Null)` both mean not given.
/// No `Debug`: `secret` would print (DEC-011).
#[derive(Clone, Default)]
pub struct ConverterInput {
    pub name: Option<Value>,
    pub form: Option<Value>,
    pub command: Option<Value>,
    pub url: Option<Value>,
    pub auth_header: Option<Value>,
    pub secret: Option<Value>,
    pub file_field: Option<Value>,
    pub response_field: Option<Value>,
    pub extensions: Option<Value>,
    pub max_input_bytes: Option<Value>,
    pub timeout_seconds: Option<Value>,
    /// Update only: the `converter_id` of a body, which must equal the target.
    pub converter_id: Option<Value>,
    /// Update only: true removes the secret.
    pub clear_secret: Option<Value>,
}
