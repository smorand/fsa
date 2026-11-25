//! The input checks of a converter (SPEC-0106 FR-NEW-001 to FR-NEW-004), each
//! producing the exact text of the spec. Inputs arrive as JSON values (DEC-004),
//! so a wrong type is refused here in words rather than by a parser.

use super::converter::{FORM_HTTP, FORM_PROGRAM};
use super::secrets::{SecretText, encrypt};
use crate::errors::{Result, ToolError};
use reqwest::header::{HeaderName, HeaderValue};
use serde_json::Value;

const NAME_MAX: usize = 64;
const EXTENSIONS_MAX: usize = 64;
const EXTENSION_MAX: usize = 16;
const MAX_INPUT_BYTES_MAX: i64 = 4_294_967_296;
const TIMEOUT_MAX: i64 = 86_400;
const URL_MAX: usize = 2048;
const SECRET_MAX: usize = 4096;
const DEFAULT_AUTH_HEADER: &str = "Authorization";
const DEFAULT_FILE_FIELD: &str = "file";

/// The fields of the http form, in the order FR-NEW-002 checks them.
const HTTP_FIELDS: [&str; 5] = ["url", "auth_header", "secret", "file_field", "response_field"];

fn invalid(text: impl Into<String>) -> ToolError {
    ToolError::invalid_argument(text)
}

/// A field is given when present with a value other than `null` (FR-NEW-001).
pub(super) fn given(v: &Option<Value>) -> Option<&Value> {
    v.as_ref().filter(|v| !v.is_null())
}

/// A text field: `None` when not given, refused when given as another type.
pub(super) fn text(field: &str, v: &Option<Value>) -> Result<Option<String>> {
    match given(v) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{field} must be text"))),
    }
}

/// The trimmed name, 1 to 64 characters counted as characters.
pub(super) fn name(v: &Option<Value>) -> Result<String> {
    let name = text("name", v)?.unwrap_or_default().trim().to_string();
    let n = name.chars().count();
    if n == 0 || n > NAME_MAX {
        return Err(invalid("name must be 1 to 64 characters"));
    }
    Ok(name)
}

pub(super) fn duplicate(name: &str) -> ToolError {
    invalid(format!("a converter named '{name}' already exists"))
}

/// `program` or `http`; anything else, a missing form included, is one refusal.
pub(super) fn form(v: &Option<Value>) -> Result<&'static str> {
    match given(v).and_then(Value::as_str) {
        Some(FORM_PROGRAM) => Ok(FORM_PROGRAM),
        Some(FORM_HTTP) => Ok(FORM_HTTP),
        _ => Err(invalid("form must be 'program' or 'http'")),
    }
}

/// Occurrences of `placeholder` across every argument, so two in one argument count twice.
fn count(command: &[String], placeholder: &str) -> usize {
    command.iter().map(|a| a.matches(placeholder).count()).sum()
}

/// FR-NEW-002 on the command: a list of texts, not blank, one `{document}`,
/// at most one `{outdir}`. Arguments are kept as given; they never reach a shell.
pub(super) fn program_command(v: &Option<Value>) -> Result<Vec<String>> {
    let Some(raw) = given(v) else {
        return Err(invalid("command is required for a program converter"));
    };
    let not_list = || invalid("command must be a list of text arguments");
    let command: Vec<String> = raw
        .as_array()
        .ok_or_else(not_list)?
        .iter()
        .map(|a| a.as_str().map(str::to_string).ok_or_else(not_list))
        .collect::<Result<_>>()?;
    if command.iter().all(|a| a.trim().is_empty()) {
        return Err(invalid("command is required for a program converter"));
    }
    let documents = count(&command, "{document}");
    if documents != 1 {
        return Err(invalid(format!(
            "command must contain exactly one '{{document}}' placeholder across its arguments, found {documents}"
        )));
    }
    let outdirs = count(&command, "{outdir}");
    if outdirs > 1 {
        return Err(invalid(format!(
            "command may contain at most one '{{outdir}}' placeholder across its arguments, found {outdirs}"
        )));
    }
    Ok(command)
}

/// Refuses the first http field given to a program converter, in the order of
/// [`HTTP_FIELDS`]; an empty text counts as given.
pub(super) fn no_http_field(values: [&Option<Value>; 5]) -> Result<()> {
    match HTTP_FIELDS.iter().zip(values).find_map(|(f, v)| given(v).map(|_| f)) {
        Some(field) => Err(invalid(format!("{field} is not valid for a program converter"))),
        None => Ok(()),
    }
}

/// The http fields once checked; `secret` is trimmed and `None` when blank.
pub(super) struct HttpFields {
    pub url: String,
    pub auth_header: String,
    pub file_field: String,
    pub response_field: String,
    pub secret: Option<SecretText>,
}

/// FR-NEW-003 up to the header name; the secret value, length and master key
/// rules come after FR-NEW-004 in [`http_secret`]. No refusal quotes a value:
/// the url may hold a password and the header carries the secret.
pub(super) fn http_fields(
    url: &Option<Value>,
    command: &Option<Value>,
    auth_header: &Option<Value>,
    secret: &Option<Value>,
    file_field: &Option<Value>,
    response_field: &Option<Value>,
    keeps_secret: bool,
) -> Result<HttpFields> {
    let url = text("url", url)?.unwrap_or_default();
    if url.trim().is_empty() {
        return Err(invalid("url is required for an http converter"));
    }
    let scheme = || invalid("url must be an http or https URL");
    if url.chars().count() > URL_MAX {
        return Err(scheme());
    }
    let parsed = url::Url::parse(&url).map_err(|_| scheme())?;
    let has_host = parsed.host_str().is_some_and(|h| !h.is_empty());
    if !matches!(parsed.scheme(), "http" | "https") || !has_host {
        return Err(scheme());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(invalid("url must not carry a user name or password"));
    }
    if given(command).is_some() {
        return Err(invalid("command is not valid for an http converter"));
    }
    // Omitted is the default name; a given blank one is the default only without a secret.
    let auth_header = text("auth_header", auth_header)?
        .unwrap_or_else(|| DEFAULT_AUTH_HEADER.to_string())
        .trim()
        .to_string();
    let secret = text("secret", secret)?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(SecretText::new);
    // `keeps_secret`: an update that leaves a stored secret in place counts as a secret set.
    if auth_header.is_empty() && (secret.is_some() || keeps_secret) {
        return Err(invalid("auth_header is required when a secret is set"));
    }
    let file_field = match text("file_field", file_field)? {
        None => DEFAULT_FILE_FIELD.to_string(),
        Some(f) if f.trim().is_empty() => return Err(invalid("file_field must not be empty")),
        Some(f) => f,
    };
    let response_field = text("response_field", response_field)?.unwrap_or_default();
    let auth_header = if auth_header.is_empty() {
        DEFAULT_AUTH_HEADER.to_string()
    } else {
        HeaderName::from_bytes(auth_header.as_bytes())
            .map_err(|_| invalid("auth_header is not a valid header name"))?;
        auth_header
    };
    Ok(HttpFields { url, auth_header, file_field, response_field, secret })
}

/// The secret rules of FR-NEW-003, last of every check: a valid header value,
/// at most 4096 characters, then the master key; the encrypted blob is returned.
pub(super) fn http_secret(
    secret: Option<&SecretText>,
    key: Option<[u8; 32]>,
) -> Result<Option<Vec<u8>>> {
    let Some(secret) = secret else { return Ok(None) };
    HeaderValue::from_str(secret.expose())
        .map_err(|_| invalid("secret is not a valid header value"))?;
    if secret.chars() > SECRET_MAX {
        return Err(invalid("secret must be at most 4096 characters"));
    }
    let key = key.ok_or_else(|| {
        ToolError::not_supported("no master key is configured; set FSA_TOKEN_KEY")
    })?;
    encrypt(&key, secret).map(Some)
}

/// The input field `v` when given, else the stored value `cur` as input, so an
/// update checks the converter as it will be (FR-NEW-005).
pub(super) fn or_current<T: serde::Serialize>(v: &Option<Value>, cur: &T) -> Option<Value> {
    given(v).cloned().or_else(|| serde_json::to_value(cur).ok())
}

/// The secret of an update: trimmed, `None` when not given or blank.
pub(super) fn new_secret(v: &Option<Value>) -> Result<Option<SecretText>> {
    Ok(text("secret", v)?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(SecretText::new))
}

/// `clear_secret` of an update: absent or `null` is false, anything but a boolean is refused.
pub(super) fn clear_secret(v: &Option<Value>) -> Result<bool> {
    match given(v) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(invalid("clear_secret must be true or false")),
    }
}

pub(super) fn clear_secret_of_program() -> ToolError {
    invalid("clear_secret is not valid for a program converter")
}

/// FR-NEW-004: up to 64 names of 1 to 16 letters or digits, lower cased, the
/// first of duplicates kept. Not given is the empty list.
pub(super) fn extensions(v: &Option<Value>) -> Result<Vec<String>> {
    let refused =
        || invalid("extensions must be a list of up to 64 names of 1 to 16 letters or digits");
    let Some(raw) = given(v) else { return Ok(Vec::new()) };
    let list = raw.as_array().filter(|l| l.len() <= EXTENSIONS_MAX).ok_or_else(refused)?;
    let mut out: Vec<String> = Vec::with_capacity(list.len());
    for item in list {
        let ext = item
            .as_str()
            .filter(|e| (1..=EXTENSION_MAX).contains(&e.chars().count()))
            .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()))
            .ok_or_else(refused)?
            .to_lowercase();
        // At most 64 items, so the linear scan stays cheap.
        if !out.contains(&ext) {
            out.push(ext);
        }
    }
    Ok(out)
}

/// A whole number in `1..=max`, else the range text of the field; a fraction,
/// a text or a boolean is not whole.
fn whole(field: &str, v: &Option<Value>, max: i64, default: i64) -> Result<i64> {
    let Some(raw) = given(v) else { return Ok(default) };
    raw.as_i64()
        .filter(|n| (1..=max).contains(n))
        .ok_or_else(|| invalid(format!("{field} must be between 1 and {max}")))
}

pub(super) fn max_input_bytes(v: &Option<Value>, default: i64) -> Result<i64> {
    whole("max_input_bytes", v, MAX_INPUT_BYTES_MAX, default)
}

pub(super) fn timeout_seconds(v: &Option<Value>, default: i64) -> Result<i64> {
    whole("timeout_seconds", v, TIMEOUT_MAX, default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_placeholder_is_counted_across_and_within_arguments() {
        let e = program_command(&Some(json!(["c", "{document}{document}"]))).unwrap_err();
        assert!(e.to_string().ends_with("found 2"), "{e}");
        assert!(program_command(&Some(json!(["c", "--in={document}"]))).is_ok());
    }

    #[test]
    fn a_whole_number_is_refused_as_a_fraction_a_text_or_a_boolean() {
        for bad in [json!(1.5), json!("10"), json!(true), json!(0), json!(86401)] {
            let e = timeout_seconds(&Some(bad), 900).unwrap_err();
            assert_eq!(
                e.to_string(),
                "ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400"
            );
        }
        assert_eq!(timeout_seconds(&Some(Value::Null), 900).unwrap(), 900);
    }

    #[test]
    fn extensions_are_lower_cased_without_duplicates_in_the_given_order() {
        let got = extensions(&Some(json!(["PDF", "docx", "pdf"]))).unwrap();
        assert_eq!(got, ["pdf", "docx"]);
        assert!(extensions(&Some(json!([".pdf"]))).is_err());
    }
}
