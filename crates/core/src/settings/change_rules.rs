//! The rules only a change obeys (SPEC-0116): a key judged alone, the port range,
//! the redirect of a link prefix, the form of what a caller gives. The rules the
//! start obeys too live in `config.rs`, so a YAML file or a stored section the start
//! reads is judged by the same words but not by these (FR-NEW-020).
//!
//! `pre` runs on the patch and the section in force, before the merge; `post` runs on
//! the section as it will be, before it is read into its struct, so a value that does
//! not fit its type never reaches the reader whose message would echo it (FR-NEW-043).

use crate::config::{host_refusal, public_base_url_refusal};
use crate::errors::{Result, ToolError};
use serde_json::Value;

fn wrong_type(section: &str, key: &str) -> ToolError {
    ToolError::invalid_argument(format!(
        "invalid setting '{section}': the value of '{key}' has the wrong type"
    ))
}

/// Whether `v` has the kind a key of the section holds.
#[derive(Clone, Copy)]
enum Kind {
    Text,
    TextOrNull,
    Whole,
    /// A whole number a port can hold (a stored port is read as it is).
    Port,
    Texts,
    Object,
}

fn fits(v: &Value, kind: Kind) -> bool {
    match kind {
        Kind::Text => v.is_string(),
        Kind::TextOrNull => v.is_string() || v.is_null(),
        Kind::Whole => v.is_i64() || v.is_u64(),
        Kind::Port => v.as_u64().is_some_and(|n| n <= 65535),
        Kind::Texts => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
        Kind::Object => v.is_object(),
    }
}

/// The first key of `patch` (alphabetical order of the dotted names) whose value is
/// of the wrong kind; a dotted name reads into the nested object.
fn first_wrong<'a>(patch: &Value, keys: &[(&'a str, Kind)]) -> Option<&'a str> {
    keys.iter()
        .find(|(k, kind)| {
            let at = format!("/{}", k.replace('.', "/"));
            patch.pointer(&at).is_some_and(|v| !fits(v, *kind))
        })
        .map(|(k, _)| *k)
}

/// A text that holds no character, or only white space (the same reading as
/// `AuthConfig::holds_key` of the start, so a path the change accepts as a key is one
/// the start reads as a key).
fn blank(s: &str) -> bool {
    s.trim().is_empty()
}

pub(super) const SERVER_REDIRECT: &str = "server.public_base_url cannot be changed through the settings: it prefixes download links that carry a credential, ask the operator to change it in the database";

/// One of the five keys that name a variable holding a secret.
struct SecretKey {
    section: &'static str,
    /// Where the change gives it.
    pointer: &'static str,
    /// The dotted name the messages use.
    dotted: &'static str,
    /// The name of the key in the secret namespace `FSA_SECRET_<SINK>_...`.
    sink: &'static str,
    /// Whether null reads as "no variable".
    null_ok: bool,
    /// Whether the empty text reads as "no variable".
    empty_ok: bool,
}

const SECRET_KEYS: [SecretKey; 5] = [
    SecretKey {
        section: "git",
        pointer: "/github_client_secret_env",
        dotted: "git.github_client_secret_env",
        sink: "GITHUB",
        null_ok: true,
        empty_ok: false,
    },
    SecretKey {
        section: "git",
        pointer: "/gitlab_client_secret_env",
        dotted: "git.gitlab_client_secret_env",
        sink: "GITLAB",
        null_ok: false,
        empty_ok: false,
    },
    SecretKey {
        section: "extract",
        pointer: "/ocr/api_key_env",
        dotted: "extract.ocr.api_key_env",
        sink: "OCR",
        null_ok: true,
        empty_ok: false,
    },
    SecretKey {
        section: "search",
        pointer: "/embedding/api_key_env",
        dotted: "search.embedding.api_key_env",
        sink: "EMBEDDING",
        null_ok: false,
        empty_ok: true,
    },
    SecretKey {
        section: "search",
        pointer: "/reranking/api_key_env",
        dotted: "search.reranking.api_key_env",
        sink: "RERANKING",
        null_ok: false,
        empty_ok: true,
    },
];

/// Whether `name` is `FSA_SECRET_<sink>_` followed by capital letters, digits or
/// underscores (ASCII), at least one.
pub(crate) fn in_secret_namespace(sink: &str, name: &str) -> bool {
    name.strip_prefix(&format!("FSA_SECRET_{sink}_")).is_some_and(|rest| {
        !rest.is_empty()
            && rest.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    })
}

/// The names the change gives to the keys that hold a secret variable (FR-NEW-018): a
/// master name is reserved, any other name must be a default of its key or sit in the
/// secret namespace of its key. A value that is not a text is left to the type refusal
/// of its section; a key the change does not give is not judged.
pub(super) fn names(section: &str, patch: &Value) -> Result<()> {
    for key in SECRET_KEYS.iter().filter(|k| k.section == section) {
        let name = match patch.pointer(key.pointer) {
            Some(Value::String(s)) => s.as_str(),
            Some(Value::Null) if key.null_ok => continue,
            // A null where no variable is not a thing the key reads as: refused as an
            // empty name that is not accepted.
            Some(Value::Null) => {
                return Err(ToolError::invalid_argument(format!(
                    "{} '' is not an accepted variable name: use FSA_SECRET_{}_ followed by capital letters, digits and underscores",
                    key.dotted, key.sink
                )));
            }
            _ => continue,
        };
        let refusal = if crate::config::is_master_name(name) {
            Some(format!(
                "{} '{name}' names a variable reserved by the server, it cannot hold a secret",
                key.dotted
            ))
        } else if name.is_empty() && key.empty_ok
            || crate::config::secret_name_defaults(key.dotted).contains(&name)
            || in_secret_namespace(key.sink, name)
        {
            None
        } else {
            Some(format!(
                "{} '{name}' is not an accepted variable name: use FSA_SECRET_{}_ followed by capital letters, digits and underscores",
                key.dotted, key.sink
            ))
        };
        if let Some(m) = refusal {
            return Err(ToolError::invalid_argument(m));
        }
    }
    Ok(())
}

/// A value of the wrong kind in a change to `server` or `auth` (FR-NEW-043), before
/// any other look at the patch, so no later message can echo it.
pub(super) fn types(section: &str, patch: &Value) -> Result<()> {
    let keys: &[(&str, Kind)] = match section {
        "server" => &[
            ("host", Kind::Text),
            ("mcp_path", Kind::Text),
            ("port", Kind::Whole),
            ("public_base_url", Kind::Text),
        ],
        "auth" => &AUTH_KEYS,
        _ => return Ok(()),
    };
    match first_wrong(patch, keys) {
        Some(k) => Err(wrong_type(section, k)),
        None => Ok(()),
    }
}

/// What is wrong with the shape of a stored `server` or `auth` value, in the words of
/// the start (SPEC-0116 FR-NEW-012): not a set of named values, or the first key of a
/// wrong kind in the alphabetical order of the dotted names.
pub(crate) fn stored_wrong_type(section: &str, full: &Value) -> Option<String> {
    let keys: &[(&str, Kind)] = match section {
        "server" => &[
            ("host", Kind::Text),
            ("mcp_path", Kind::Text),
            ("port", Kind::Port),
            ("public_base_url", Kind::Text),
        ],
        "auth" => &AUTH_KEYS,
        _ => return None,
    };
    if !full.is_object() {
        return Some("the value has the wrong type".into());
    }
    first_wrong(full, keys).map(|k| format!("the value of '{k}' has the wrong type"))
}

const AUTH_KEYS: [(&str, Kind); 9] = [
    ("admins", Kind::Texts),
    ("jwt", Kind::Object),
    ("jwt.algorithms", Kind::Texts),
    ("jwt.audience", Kind::TextOrNull),
    ("jwt.header", Kind::Text),
    ("jwt.issuer", Kind::TextOrNull),
    ("jwt.public_key_path", Kind::Text),
    ("jwt.public_keys", Kind::Texts),
    ("jwt.username_claim", Kind::Text),
];

/// The rules on what the change gives, before the merge. `in_force` is the section
/// as stored, else as running.
pub(super) fn pre(section: &str, patch: &Value, in_force: &Value) -> Result<()> {
    // A link prefix that carries a credential is the operator's (FR-NEW-042).
    if section == "server"
        && let Some(Value::String(url)) = patch.get("public_base_url")
        && !url.is_empty()
        && in_force.get("public_base_url").and_then(Value::as_str) != Some(url.as_str())
    {
        return Err(ToolError::invalid_argument(SERVER_REDIRECT));
    }
    Ok(())
}

/// The rules on the section as it will be (`full`), then on the given keys alone.
pub(super) fn post(section: &str, patch: &Value, full: &Value, old: &Value) -> Result<()> {
    if section == "server" {
        let text = |k: &str| full.get(k).and_then(Value::as_str).unwrap_or_default();
        if let Some(m) = host_refusal(text("host")) {
            return Err(ToolError::invalid_argument(m));
        }
        let path = text("mcp_path");
        if !path.starts_with('/') {
            return Err(ToolError::invalid_argument(format!(
                "server.mcp_path must start with '/' (got '{path}')"
            )));
        }
        if let Some(m) = crate::mcp_mount::refusal(path) {
            return Err(ToolError::invalid_argument(m));
        }
        if let Some(m) = public_base_url_refusal(text("public_base_url")) {
            return Err(ToolError::invalid_argument(m));
        }
        // The port is judged where it is given: a stored 0 does not block another key.
        // A whole number past the signed range is out of the port range too.
        if let Some(n) = patch.get("port").and_then(Value::as_number)
            && n.as_i64().is_none_or(|p| !(1..=65535).contains(&p))
        {
            return Err(ToolError::invalid_argument(
                "invalid setting 'server': server.port must be a whole number from 1 to 65535",
            ));
        }
    }
    if section == "auth" {
        auth_post(patch, full, old)?;
    }
    Ok(())
}

fn texts(v: Option<&Value>) -> Vec<&str> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn text<'a>(v: &'a Value, pointer: &str) -> &'a str {
    v.pointer(pointer).and_then(Value::as_str).unwrap_or_default()
}

/// Whether a section holds a key: a file path, or a non empty list.
fn holds_key(section: &Value) -> bool {
    !blank(text(section, "/jwt/public_key_path"))
        || section
            .pointer("/jwt/public_keys")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
}

const MAX_KEYS: usize = 8;

fn invalid(m: &str) -> ToolError {
    ToolError::invalid_argument(m)
}

/// The rules of a change to `auth`, in the order of FR-NEW-023: what the change gives
/// (empty entries, issuer, audience, the keys by stage), then what it would take away.
fn auth_post(patch: &Value, full: &Value, old: &Value) -> Result<()> {
    let given_texts = |pointer: &str| patch.pointer(pointer).map(|v| texts(Some(v)));
    if given_texts("/admins").is_some_and(|l| l.iter().any(|a| blank(a))) {
        return Err(invalid("invalid setting 'auth': admins must not hold an empty name"));
    }
    if patch.pointer("/jwt/header").and_then(Value::as_str).is_some_and(blank) {
        return Err(invalid("invalid setting 'auth': jwt.header must not be empty"));
    }
    if given_texts("/jwt/algorithms").is_some_and(|l| l.is_empty() || l.iter().any(|a| blank(a))) {
        return Err(invalid("invalid setting 'auth': jwt.algorithms must not be empty"));
    }
    for (key, what) in [("issuer", "issuer"), ("audience", "audience")] {
        if patch.pointer(&format!("/jwt/{key}")).and_then(Value::as_str).is_some_and(blank) {
            return Err(invalid(&format!(
                "invalid setting 'auth': jwt.{key} must not be empty (give null for no {what})"
            )));
        }
    }
    if let Some(given) = given_texts("/jwt/public_keys") {
        check_keys(&given)?;
    }
    if texts(full.pointer("/jwt/public_keys")).len() > MAX_KEYS {
        return Err(invalid("auth.jwt.public_keys holds more than 8 keys"));
    }
    if holds_key(old) && !holds_key(full) {
        return Err(invalid(
            "auth.jwt needs at least one public key: give public_keys or public_key_path",
        ));
    }
    let admins =
        |s: &Value| s.get("admins").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
    if admins(old) && !admins(full) {
        return Err(invalid("auth.admins needs at least one platform administrator"));
    }
    if !blank(text(old, "/jwt/username_claim")) && blank(text(full, "/jwt/username_claim")) {
        return Err(invalid("auth.jwt.username_claim must not be empty"));
    }
    Ok(())
}

/// The keys the change gives, judged in stages (FR-NEW-047): a private key anywhere,
/// the form, a modulus under 2048 bits, a modulus over 8192 bits; the lowest position
/// of the first stage that fails is named, and no key text is ever echoed.
fn check_keys(given: &[&str]) -> Result<()> {
    check_key_entries(given, true)
}

/// [`check_keys`] with or without the stages on the size (a start judges the form
/// only).
pub(crate) fn check_key_entries(given: &[&str], sizes: bool) -> Result<()> {
    let at = |i: usize, what: &str| invalid(&format!("auth.jwt.public_keys[{i}] {what}"));
    if let Some(i) = given.iter().position(|e| holds_private(e)) {
        return Err(at(i, "holds a private key: only a public key is accepted"));
    }
    let bits: Vec<Option<usize>> = given.iter().map(|e| rsa_bits(e)).collect();
    if let Some(i) = bits.iter().position(Option::is_none) {
        return Err(at(i, "is not a public key in PEM form (an RSA public key is expected)"));
    }
    if !sizes {
        return Ok(());
    }
    if let Some(i) = bits.iter().position(|b| b.is_some_and(|n| n < 2048)) {
        return Err(at(i, "holds an RSA key shorter than 2048 bits"));
    }
    if let Some(i) = bits.iter().position(|b| b.is_some_and(|n| n > 8192)) {
        return Err(at(i, "holds an RSA key longer than 8192 bits"));
    }
    Ok(())
}

/// A line `-----BEGIN <label>-----` whose label ends with `PRIVATE KEY`, wherever it
/// stands in the text; a trailing space, tab or carriage return does not hide it, a
/// leading space does (FR-NEW-041).
fn holds_private(entry: &str) -> bool {
    entry.lines().any(|l| {
        let t = l.trim_end_matches([' ', '\t', '\r']);
        t.starts_with("-----BEGIN ") && t.ends_with("PRIVATE KEY-----")
    })
}

/// The modulus length of the RSA public key an entry holds, `None` when the entry is
/// not exactly one PEM block of an RSA public key between blank lines.
fn rsa_bits(entry: &str) -> Option<usize> {
    crate::identity::parse_rsa_public(entry).map(|(_, bits)| bits)
}
