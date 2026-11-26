//! The indexing profile of a project (SPEC-0107): the default chunk strategy
//! with its settings, the text embedding model and the image embedding model.
//! Same shape as the converter operations: checks in a fixed order, the change
//! and its record in one transaction, one retry on a lost race.

use super::profile_store::ProfileRow;
use super::registry::{Registry, Target, marker_now, now};
use super::store::SettingsStore;
use crate::chunking::ChunkDefaults;
use crate::chunking::service::Requested;
use crate::errors::{Result, ToolError};
use crate::storage::rel::RelationalTx;
use serde_json::{Map, Value, json};

/// The fields `admin.set_indexing_profile` may change. `None` keeps the stored
/// value, `Some("")` clears it (FR-NEW-006).
#[derive(Debug, Clone, Default)]
pub struct ProfileChange {
    pub chunk_strategy: Option<String>,
    pub chunk_settings: Option<Map<String, Value>>,
    pub text_model_id: Option<String>,
    pub image_model_id: Option<String>,
}

/// The text model the configuration file names, used when neither the project
/// nor the registry chose one (FR-NEW-008).
#[derive(Debug, Clone)]
pub struct ConfiguredModel {
    pub name: String,
    pub dimension: i64,
}

fn project_missing(project_id: &str) -> ToolError {
    ToolError::project_not_found(project_id)
}

fn article(kind: &str) -> &'static str {
    if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }
}

/// A model of the kind wanted, in words of FR-NEW-005.
async fn checked_model(tx: &mut Box<dyn RelationalTx>, model_id: &str, kind: &str) -> Result<()> {
    let Some(m) = SettingsStore::model_by_id(tx, model_id).await? else {
        return Err(ToolError::not_found(format!("model not found: {model_id}")));
    };
    if m.kind != kind {
        return Err(ToolError::invalid_argument(format!(
            "model '{}' is not {} {kind} model",
            m.name,
            article(kind)
        )));
    }
    Ok(())
}

/// Orders the settings by name so two equal requests store the same text.
fn canonical(settings: &[(&'static str, u32)]) -> Option<String> {
    if settings.is_empty() {
        return None;
    }
    let mut sorted = settings.to_vec();
    sorted.sort();
    let map: Map<String, Value> =
        sorted.into_iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
    Some(Value::Object(map).to_string())
}

fn stored_settings(raw: &Option<String>) -> Map<String, Value> {
    raw.as_deref()
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// The new row a change leads to, checked, plus the names of the fields that
/// differ from the stored row (FR-NEW-004 to 007).
async fn apply(
    tx: &mut Box<dyn RelationalTx>,
    cur: &ProfileRow,
    c: &ProfileChange,
    d: &ChunkDefaults,
) -> Result<(ProfileRow, Vec<String>)> {
    let mut next = cur.clone();
    match c.chunk_strategy.as_deref() {
        Some("") => {
            next.chunk_strategy = None;
            next.chunk_settings = None;
        }
        Some(s) => {
            if cur.chunk_strategy.as_deref() != Some(s) {
                // Settings of another strategy never survive a change of strategy.
                next.chunk_settings = None;
            }
            next.chunk_strategy = Some(s.to_string());
        }
        None => {}
    }
    if let Some(given) = &c.chunk_settings {
        if given.is_empty() {
            next.chunk_settings = None;
        } else {
            let Some(_) = &next.chunk_strategy else {
                return Err(ToolError::invalid_argument("chunk_settings need a chunk_strategy"));
            };
            next.chunk_settings = Some(Value::Object(given.clone()).to_string());
        }
    }
    if let Some(strategy) = next.chunk_strategy.clone() {
        let settings = stored_settings(&next.chunk_settings);
        let req = Requested::parse(&strategy, &settings, d)?;
        next.chunk_settings = canonical(&req.settings);
    }
    if let Some(id) = c.text_model_id.as_deref() {
        if id.is_empty() {
            next.text_model_id = None;
        } else {
            checked_model(tx, id, "text_embedding").await?;
            next.text_model_id = Some(id.to_string());
        }
    }
    if let Some(id) = c.image_model_id.as_deref() {
        if id.is_empty() {
            next.image_model_id = None;
        } else {
            checked_model(tx, id, "image_embedding").await?;
            next.image_model_id = Some(id.to_string());
        }
    }
    let mut fields = Vec::new();
    if next.chunk_strategy != cur.chunk_strategy {
        fields.push("chunk_strategy".to_string());
    }
    if next.chunk_settings != cur.chunk_settings {
        fields.push("chunk_settings".to_string());
    }
    if next.text_model_id != cur.text_model_id {
        fields.push("text_model_id".to_string());
    }
    if next.image_model_id != cur.image_model_id {
        fields.push("image_model_id".to_string());
    }
    Ok((next, fields))
}

impl Registry {
    /// The profile as stored. `ERR_PROJECT_NOT_FOUND` when the project row is
    /// gone; a project in its grace period is found.
    pub async fn project_profile(&self, project_id: &str) -> Result<ProfileRow> {
        let mut tx = self.store.begin().await?;
        let row = SettingsStore::profile_row(&mut tx, project_id).await?;
        tx.commit().await?;
        row.ok_or_else(|| project_missing(project_id))
    }

    /// Changes the fields given and answers the stored profile. A change that
    /// leaves every field as it is writes and records nothing (FR-NEW-009).
    pub async fn set_project_profile(
        &self,
        person: &str,
        project_id: &str,
        change: ProfileChange,
        defaults: &ChunkDefaults,
    ) -> Result<ProfileRow> {
        let mut attempt = 0;
        loop {
            match self.try_set_project_profile(person, project_id, &change, defaults).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_set_project_profile(
        &self,
        person: &str,
        project_id: &str,
        change: &ProfileChange,
        defaults: &ChunkDefaults,
    ) -> Result<ProfileRow> {
        let mut tx = self.store.begin().await?;
        let Some(cur) = SettingsStore::profile_row(&mut tx, project_id).await? else {
            return Err(project_missing(project_id));
        };
        let (mut next, fields) = apply(&mut tx, &cur, change, defaults).await?;
        if fields.is_empty() {
            tx.commit().await?;
            return Ok(cur);
        }
        let at = now();
        if fields.iter().any(|f| f == "text_model_id" || f == "image_model_id") {
            next.vectors_stale_at = Some(marker_now());
        }
        SettingsStore::write_profile_row(&mut tx, project_id, &next, &fields).await?;
        let target = Target::of("project_profile", project_id, project_id);
        self.record(&mut tx, person, "set_indexing_profile", target, fields, at).await?;
        tx.commit().await?;
        Ok(next)
    }

    /// The profile as `admin.get_indexing_profile` answers it (FR-NEW-003):
    /// the stored choices, the models resolved with their provider, the
    /// fallback of the text model.
    pub async fn indexing_profile_json(
        &self,
        project_id: &str,
        defaults: &ChunkDefaults,
        configured: Option<&ConfiguredModel>,
    ) -> Result<Value> {
        let mut tx = self.store.begin().await?;
        let Some(row) = SettingsStore::profile_row(&mut tx, project_id).await? else {
            return Err(project_missing(project_id));
        };
        let out = self.render(&mut tx, project_id, &row, defaults, configured).await?;
        tx.commit().await?;
        Ok(out)
    }

    /// Every project with its profile in the same shape, by project id.
    pub async fn list_profiles_json(
        &self,
        defaults: &ChunkDefaults,
        configured: Option<&ConfiguredModel>,
    ) -> Result<Vec<Value>> {
        let rows = self.store.list_profile_rows().await?;
        let mut tx = self.store.begin().await?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, row) in rows {
            out.push(self.render(&mut tx, &id, &row, defaults, configured).await?);
        }
        tx.commit().await?;
        Ok(out)
    }

    async fn render(
        &self,
        tx: &mut Box<dyn RelationalTx>,
        project_id: &str,
        row: &ProfileRow,
        defaults: &ChunkDefaults,
        configured: Option<&ConfiguredModel>,
    ) -> Result<Value> {
        // A stored choice that the limits of today no longer accept still reads: the
        // filled view is dropped, so one project never breaks the page of every other.
        let chunking = match row.chunk_strategy.as_deref() {
            Some(strategy) => {
                let settings = stored_settings(&row.chunk_settings);
                let filled = Requested::parse(strategy, &settings, defaults)
                    .map(|req| Value::Object(req.filled(defaults)))
                    .unwrap_or(Value::Null);
                json!({"strategy": strategy, "settings": settings,
                       "filled": filled, "source": "project"})
            }
            None => json!({"strategy": null, "settings": {}, "filled": null,
                           "source": "server_default"}),
        };
        let text_model = match &row.text_model_id {
            Some(id) => model_view(tx, id, "project").await?,
            None => match SettingsStore::default_id(tx, "text_embedding").await? {
                Some(id) => model_view(tx, &id, "server_default").await?,
                None => match configured {
                    Some(c) => json!({"model_id": null, "name": c.name, "provider": null,
                        "kind": "text_embedding", "dimension": c.dimension,
                        "source": "configuration"}),
                    None => Value::Null,
                },
            },
        };
        let image_model = match &row.image_model_id {
            Some(id) => model_view(tx, id, "project").await?,
            None => Value::Null,
        };
        Ok(json!({
            "project_id": project_id,
            "chunking": chunking,
            "text_model": text_model,
            "image_model": image_model,
            "vectors_out_of_date": row.vectors_stale_at.is_some(),
            "vectors_out_of_date_since": row.vectors_stale_at,
        }))
    }
}

/// A model with its provider name; `Null` cannot happen for an id a profile
/// holds (a used model is never deleted), but a vanished one reads as none
/// rather than failing the whole read.
async fn model_view(tx: &mut Box<dyn RelationalTx>, model_id: &str, source: &str) -> Result<Value> {
    let Some(m) = SettingsStore::model_by_id(tx, model_id).await? else {
        return Ok(Value::Null);
    };
    let provider = SettingsStore::provider_name(tx, &m.provider_id).await?;
    Ok(json!({"model_id": m.model_id, "name": m.name, "provider": provider,
              "kind": m.kind, "dimension": m.dimension, "source": source}))
}
