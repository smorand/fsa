//! The generic settings store (SPEC-0114): the eight scalar sections of the
//! YAML configuration kept as one JSON row each in the admin store.
//!
//! A section is read and written as the JSON of its serde struct, so the YAML
//! names are the setting names. A change merges the patch over the current
//! section, refuses an unknown key, deserializes into the struct, and runs the
//! real `ServerConfig::validate` on a candidate configuration, so the messages
//! are today's (DEC-001). Secrets leave the stored value and live in one
//! encrypted object under the master key (DEC-002). Nothing here applies a
//! change to the running server: every section is read at boot (DEC-004).

use super::registry::now;
use super::secrets::{self, SecretText};
use super::server_settings_store::ServerSettingRow;
use super::store::SettingsStore;
use crate::config::ServerConfig;
use crate::errors::{Result, ToolError, code};
use crate::storage::rel::RelationalTx;
use crate::util::normalize_identity;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The sections, in the order lists show them (SPEC-0114, SPEC-0115 added the next three, SPEC-0116 the next two, SPEC-0117 the last).
pub const SECTIONS: [&str; 14] = [
    "safety",
    "api",
    "extract",
    "doc",
    "sqlite",
    "db",
    "doc_service",
    "search",
    "git",
    "web",
    "context7",
    "server",
    "auth",
    "infra",
];

const SECRET_MAX: usize = 4096;

/// The keys of a section that hold a secret, as dotted paths.
pub(crate) fn secret_paths(section: &str) -> &'static [&'static str] {
    match section {
        "doc_service" => &["api.auth_token"],
        "infra" => &["blob.access_key", "blob.secret_key", "meta.dsn", "git.dsn", "oauth.dsn"],
        _ => &[],
    }
}

/// The keys of a section that hold a map with keys of their own (host names): a
/// change gives the whole map, which replaces the stored one, so a host can be
/// removed (SPEC-0115 DEC-001).
fn free_paths(section: &str) -> &'static [&'static str] {
    match section {
        "git" => &["hosts"],
        _ => &[],
    }
}

fn unknown_section(section: &str) -> ToolError {
    ToolError::invalid_argument(format!(
        "unknown setting section '{section}', expected one of {}",
        SECTIONS.join(", ")
    ))
}

fn check_section(section: &str) -> Result<()> {
    if SECTIONS.contains(&section) { Ok(()) } else { Err(unknown_section(section)) }
}

/// The JSON of one section of `config`, secrets included.
pub(crate) fn section_json(config: &ServerConfig, section: &str) -> Value {
    let v = match section {
        "safety" => serde_json::to_value(&config.safety),
        "api" => serde_json::to_value(&config.api),
        "extract" => serde_json::to_value(&config.extract),
        "doc" => serde_json::to_value(&config.doc),
        "sqlite" => serde_json::to_value(&config.sqlite),
        "db" => serde_json::to_value(&config.db),
        "doc_service" => serde_json::to_value(&config.doc_service),
        "search" => serde_json::to_value(&config.search),
        "git" => serde_json::to_value(&config.git),
        "web" => serde_json::to_value(&config.web),
        "context7" => serde_json::to_value(&config.context7),
        "server" => serde_json::to_value(&config.server),
        "auth" => serde_json::to_value(&config.auth),
        "infra" => serde_json::to_value(config.infra.stores()),
        other => unreachable!("section '{other}' was checked"),
    };
    v.expect("a configuration section serializes")
}

/// `config` with `section` replaced by `value`; a value that does not fit the
/// section is refused naming it.
pub(crate) fn with_section(
    config: &ServerConfig,
    section: &str,
    value: Value,
) -> Result<ServerConfig> {
    let bad = |e: serde_json::Error| {
        ToolError::invalid_argument(format!("invalid setting '{section}': {e}"))
    };
    let mut next = config.clone();
    match section {
        "safety" => next.safety = serde_json::from_value(value).map_err(bad)?,
        "api" => next.api = serde_json::from_value(value).map_err(bad)?,
        "extract" => next.extract = serde_json::from_value(value).map_err(bad)?,
        "doc" => next.doc = serde_json::from_value(value).map_err(bad)?,
        "sqlite" => next.sqlite = serde_json::from_value(value).map_err(bad)?,
        "db" => next.db = serde_json::from_value(value).map_err(bad)?,
        "doc_service" => next.doc_service = serde_json::from_value(value).map_err(bad)?,
        "search" => next.search = serde_json::from_value(value).map_err(bad)?,
        "git" => next.git = serde_json::from_value(value).map_err(bad)?,
        "web" => {
            // The test only fields are not settings: keep the ones the process holds.
            let mut web: crate::config::WebConfig = serde_json::from_value(value).map_err(bad)?;
            web.keep_test_seams_of(&config.web);
            next.web = web;
        }
        "context7" => next.context7 = serde_json::from_value(value).map_err(bad)?,
        "server" => next.server = serde_json::from_value(value).map_err(bad)?,
        "auth" => next.auth = serde_json::from_value(value).map_err(bad)?,
        "infra" => {
            let stores = serde_json::from_value(value).map_err(bad)?;
            next.infra = config.infra.with_stores(stores);
        }
        other => return Err(unknown_section(other)),
    }
    Ok(next)
}

fn path_get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |cur, k| cur.get(k))
}

fn path_remove(v: &mut Value, path: &str) -> Option<Value> {
    let (head, last) = match path.rsplit_once('.') {
        Some((h, l)) => (Some(h), l),
        None => (None, path),
    };
    let parent = match head {
        Some(h) => h.split('.').try_fold(&mut *v, |cur, k| cur.get_mut(k))?,
        None => v,
    };
    parent.as_object_mut()?.remove(last)
}

fn path_set(v: &mut Value, path: &str, value: Value) {
    let mut cur = v;
    let keys: Vec<&str> = path.split('.').collect();
    for k in &keys[..keys.len() - 1] {
        cur = cur
            .as_object_mut()
            .expect("an object holds the secret path")
            .entry(k.to_string())
            .or_insert_with(|| json!({}));
    }
    cur.as_object_mut()
        .expect("an object holds the secret path")
        .insert(keys[keys.len() - 1].to_string(), value);
}

/// Removes the secrets from `full` and returns the non blank ones by path.
pub(crate) fn split_secrets(section: &str, full: &mut Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for path in secret_paths(section) {
        if let Some(Value::String(s)) = path_remove(full, path)
            && !s.trim().is_empty()
        {
            out.insert((*path).to_string(), s.trim().to_string());
        }
    }
    out
}

/// `full` with the secrets back in place.
pub(crate) fn join_secrets(full: &mut Value, secrets: &BTreeMap<String, String>) {
    for (path, value) in secrets {
        path_set(full, path, Value::String(value.clone()));
    }
}

fn merge(into: &mut Value, patch: &Value) {
    match (into.as_object_mut(), patch.as_object()) {
        (Some(base), Some(p)) => {
            for (k, v) in p {
                match base.get_mut(k) {
                    Some(cur) if cur.is_object() && v.is_object() => merge(cur, v),
                    _ => {
                        base.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        _ => *into = patch.clone(),
    }
}

/// An unknown key at any depth, or an object where the section holds a scalar.
fn check_keys(section: &str, template: &Value, patch: &Value, prefix: &str) -> Result<()> {
    let Some(p) = patch.as_object() else { return Ok(()) };
    for (k, v) in p {
        let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
        if free_paths(section).contains(&path.as_str()) {
            continue;
        }
        let Some(t) = template.get(k) else {
            return Err(ToolError::invalid_argument(format!(
                "unknown key '{path}' in setting '{section}'"
            )));
        };
        if t.is_object() {
            if !v.is_object() {
                return Err(ToolError::invalid_argument(format!(
                    "invalid setting '{section}': {path} must be an object"
                )));
            }
            check_keys(section, t, v, &path)?;
        }
    }
    Ok(())
}

/// The dotted paths whose value differs between `old` and `new`, sorted.
fn changed_paths(old: &Value, new: &Value, prefix: &str, out: &mut Vec<String>) {
    match (old.as_object(), new.as_object()) {
        (Some(o), Some(n)) => {
            let mut keys: Vec<&String> = o.keys().chain(n.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                let (a, b) = (o.get(k).unwrap_or(&Value::Null), n.get(k).unwrap_or(&Value::Null));
                changed_paths(a, b, &path, out);
            }
        }
        _ => {
            if old != new {
                out.push(prefix.to_string());
            }
        }
    }
}

fn last4(secret: &str) -> String {
    let n = secret.chars().count();
    if n < 8 { String::new() } else { secret.chars().skip(n - 4).collect() }
}

/// What a read shows of a secret's end: nothing for a DSN, whose tail may be a password
/// (SPEC-0117 DEC-005).
fn shown_tail(path: &str, secret: &str) -> String {
    if path.ends_with(".dsn") { String::new() } else { last4(secret) }
}

pub(crate) fn decrypt_all(key: Option<[u8; 32]>, blob: &[u8]) -> Option<BTreeMap<String, String>> {
    let text = secrets::decrypt(key, blob)?;
    serde_json::from_str(text.expose()).ok()
}

pub(crate) fn encrypt_all(key: &[u8; 32], map: &BTreeMap<String, String>) -> Result<Vec<u8>> {
    let text = serde_json::to_string(map).expect("a map of texts serializes");
    secrets::encrypt(key, &SecretText::new(text))
}

pub struct ServerSettings {
    store: SettingsStore,
    running: Arc<ServerConfig>,
}

impl ServerSettings {
    pub fn of(
        admin: &dyn crate::storage::traits::AdminBackend,
        running: Arc<ServerConfig>,
    ) -> Result<Self> {
        admin
            .settings()
            // The flags of the command line are not settings: compare with and
            // persist from the configuration without them.
            .map(|store| Self { store, running: Arc::new(running.without_flags()) })
            .ok_or_else(|| ToolError::not_supported("the platform settings store is not available"))
    }

    /// `{settings: [...]}`: one short line per section.
    pub async fn list(&self) -> Result<Value> {
        let rows = self.store.all_server_settings().await?;
        let mut out = Vec::new();
        for section in SECTIONS {
            let row = rows.iter().find(|r| r.section == section);
            let a = self.answer(section, row)?;
            out.push(json!({
                "section": section,
                "source": a["source"],
                "restart_required": a["restart_required"],
                "updated_at": a["updated_at"],
                "updated_by": a["updated_by"],
            }));
        }
        Ok(json!({ "settings": out }))
    }

    pub async fn get(&self, section: &str) -> Result<Value> {
        check_section(section)?;
        let mut tx = self.store.begin().await?;
        let row = SettingsStore::server_setting_in(&mut tx, section).await?;
        self.answer(section, row.as_ref())
    }

    pub async fn set(&self, person: &str, section: &str, patch: Value) -> Result<Value> {
        // A key conflict between concurrent writers (the change id, or the first
        // insert of a section) surfaces as an internal error; a retry reads the
        // new state. Three attempts, since several writers may collide in a row.
        let mut attempt = 0;
        loop {
            match self.try_set(person, section, &patch).await {
                Err(e) if attempt < 2 && e.code == code::INTERNAL_ERROR => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_set(&self, person: &str, section: &str, patch: &Value) -> Result<Value> {
        check_section(section)?;
        if !patch.is_object() && matches!(section, "server" | "auth") {
            return Err(ToolError::invalid_argument(format!(
                "invalid setting '{section}': the value has the wrong type"
            )));
        }
        if !patch.is_object() {
            return Err(ToolError::invalid_argument(format!(
                "the value of setting '{section}' must be an object"
            )));
        }
        let key = self.store.key();
        super::change_rules::names(section, patch)?;
        super::change_rules::types(section, patch)?;
        let template = section_json(&ServerConfig::default(), section);
        check_keys(section, &template, patch, "")?;
        for path in free_paths(section) {
            if matches!(path_get(patch, path), Some(Value::Null)) {
                return Err(ToolError::invalid_argument(format!(
                    "invalid setting '{section}': {path} must be an object or a list of pairs"
                )));
            }
        }
        let mut given: BTreeMap<String, String> = BTreeMap::new();
        for path in secret_paths(section) {
            match path_get(patch, path) {
                None => {}
                Some(Value::String(s)) => {
                    if s.chars().count() > SECRET_MAX {
                        return Err(ToolError::invalid_argument(
                            "secret must be at most 4096 characters",
                        ));
                    }
                    if !s.trim().is_empty() && key.is_none() {
                        return Err(ToolError::not_supported(
                            "no master key is configured; set FSA_TOKEN_KEY",
                        ));
                    }
                    given.insert((*path).to_string(), s.trim().to_string());
                }
                Some(_) => {
                    return Err(ToolError::invalid_argument(format!(
                        "{path} of setting '{section}' must be a text"
                    )));
                }
            }
        }

        let mut tx = self.store.begin().await?;
        let row = SettingsStore::server_setting_in(&mut tx, section).await?;
        // The section as it stands, secrets in place.
        let (mut current, kept) = match &row {
            Some(r) => {
                let value: Value = serde_json::from_str(&r.value)
                    .map_err(|_| ToolError::internal("a stored setting is unreadable"))?;
                let kept = match &r.secret_enc {
                    None => BTreeMap::new(),
                    Some(blob) => match decrypt_all(key, blob) {
                        Some(m) => m,
                        // Only a section whose secrets are all given again can
                        // be rewritten without reading the old ones.
                        None if secret_paths(section).iter().all(|p| given.contains_key(*p)) => {
                            BTreeMap::new()
                        }
                        None => {
                            return Err(ToolError::invalid_argument(format!(
                                "the secrets of setting '{section}' cannot be read; \
                                 check FSA_TOKEN_KEY"
                            )));
                        }
                    },
                };
                (value, kept)
            }
            None => {
                let mut full = section_json(&self.running, section);
                let kept = split_secrets(section, &mut full);
                (full, kept)
            }
        };
        let mut old_full = current.clone();
        join_secrets(&mut old_full, &kept);
        // The rules on what the change gives (SPEC-0116), before anything is merged.
        super::change_rules::pre(section, patch, &old_full)?;

        // The patch without its secret keys, then the secrets decided.
        let mut plain = patch.clone();
        for path in secret_paths(section) {
            path_remove(&mut plain, path);
        }
        merge(&mut current, &plain);
        for path in free_paths(section) {
            if let Some(v) = path_get(&plain, path) {
                path_set(&mut current, path, v.clone());
            }
        }
        let mut secrets_now = kept;
        for (path, v) in given {
            if v.is_empty() {
                secrets_now.remove(&path);
            } else {
                secrets_now.insert(path, v);
            }
        }
        let mut full = current;
        join_secrets(&mut full, &secrets_now);

        // The rules on the section as it will be, before it is read into its struct.
        super::change_rules::post(section, patch, &full, &old_full)?;
        // Today's checks, on the configuration as it will be.
        let candidate = with_section(&self.running, section, full)?;
        candidate.check()?;
        if section == "infra" {
            super::infra_rules::blob_rules(&candidate)?;
            // The probes may wait on a network: the store has one connection, so the
            // transaction is given up for them and taken again, and a change made
            // meanwhile makes this attempt start over (the retry of `set`).
            drop(tx);
            let projects = self.store.project_ids().await?;
            super::infra_check::run(&self.running, &candidate, &projects).await?;
            tx = self.store.begin().await?;
            let again = SettingsStore::server_setting_in(&mut tx, section).await?;
            let key = |r: &Option<ServerSettingRow>| {
                r.as_ref().map(|r| (r.value.clone(), r.updated_at.clone()))
            };
            if key(&again) != key(&row) {
                return Err(ToolError::internal("the setting changed while it was checked"));
            }
        }
        let mut canonical = section_json(&candidate, section);
        let canonical_secrets = split_secrets(section, &mut canonical);
        let mut new_full = canonical.clone();
        join_secrets(&mut new_full, &canonical_secrets);
        let mut fields = Vec::new();
        changed_paths(&old_full, &new_full, "", &mut fields);

        let secret_enc = if canonical_secrets.is_empty() {
            None
        } else {
            let key = key.ok_or_else(|| {
                ToolError::not_supported("no master key is configured; set FSA_TOKEN_KEY")
            })?;
            Some(encrypt_all(&key, &canonical_secrets)?)
        };
        let at = now();
        let by = normalize_identity(person);
        let new_row = ServerSettingRow {
            section: section.into(),
            value: canonical.to_string(),
            secret_enc,
            updated_at: at.clone(),
            updated_by: by.clone(),
        };
        SettingsStore::put_server_setting(&mut tx, &new_row, row.is_some()).await?;
        record(&mut tx, &by, "set_setting", section, fields, at).await?;
        tx.commit().await?;
        self.answer(section, Some(&new_row))
    }

    /// The answer shape of `get`; `row` is the stored section, `None` when the
    /// section was never set.
    fn answer(&self, section: &str, row: Option<&ServerSettingRow>) -> Result<Value> {
        let mut running = section_json(&self.running, section);
        let running_secrets = split_secrets(section, &mut running);
        let paths = secret_paths(section);
        let mut secrets_json = Map::new();
        let (value, source, restart, at, by);
        match row {
            None => {
                for p in paths {
                    let s = running_secrets.get(*p);
                    secrets_json.insert(
                        (*p).to_string(),
                        json!({"has_value": s.is_some(), "last4": s.map(|s| shown_tail(p, s)).unwrap_or_default()}),
                    );
                }
                value = running;
                source = if self.running.from_yaml_file { "yaml" } else { "default" };
                restart = false;
                at = Value::Null;
                by = Value::Null;
            }
            Some(r) => {
                let stored: Value = serde_json::from_str(&r.value)
                    .map_err(|_| ToolError::internal("a stored setting is unreadable"))?;
                let secrets = r.secret_enc.as_ref().map(|blob| decrypt_all(self.store.key(), blob));
                for p in paths {
                    let (has, tail) = match &secrets {
                        None => (false, String::new()),
                        Some(Some(m)) => (
                            m.contains_key(*p),
                            m.get(*p).map(|s| shown_tail(p, s)).unwrap_or_default(),
                        ),
                        // Stored but unreadable: say it is set, show nothing.
                        Some(None) => (true, String::new()),
                    };
                    secrets_json.insert((*p).to_string(), json!({"has_value": has, "last4": tail}));
                }
                let secrets_differ = match &secrets {
                    None => !running_secrets.is_empty(),
                    Some(Some(m)) => *m != running_secrets,
                    Some(None) => false,
                };
                restart = stored != running || secrets_differ;
                value = stored;
                source = "database";
                at = json!(r.updated_at);
                by = json!(r.updated_by);
            }
        }
        Ok(json!({
            "section": section,
            "value": value,
            "secrets": Value::Object(secrets_json),
            "source": source,
            "restart_required": restart,
            "updated_at": at,
            "updated_by": by,
        }))
    }
}

pub(crate) async fn record(
    tx: &mut Box<dyn RelationalTx>,
    person: &str,
    action: &str,
    section: &str,
    fields: Vec<String>,
    at: String,
) -> Result<()> {
    SettingsStore::record(
        tx,
        super::model::Change {
            change_id: String::new(),
            at,
            person: person.into(),
            action: action.into(),
            target_kind: "setting".into(),
            target_id: section.into(),
            target_name: section.into(),
            fields,
        },
    )
    .await
}
