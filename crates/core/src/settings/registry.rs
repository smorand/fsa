//! `Registry`: the one implementation of every registry operation (SPEC-0102
//! DEC-006). Tools, REST handlers and the screen add access control and shaping
//! only. Checks run in the order of the spec (DEC-011) on the provider as it is
//! after the update; a change and its record share one transaction (DEC-009).

use super::model::{
    AUTH_BEARER, AUTH_NONE, Change, DEFAULT_KINDS, Defaults, KINDS, Model, ModelInput, ModelUpdate,
    Page, Provider, ProviderInput, ProviderUpdate, decode_marker, encode_marker,
};
use super::secrets::{self, SecretText};
use super::store::{ProviderRow, SettingsStore, name_key};
use crate::errors::{Result, ToolError, code};
use crate::storage::traits::AdminBackend;
use crate::util::normalize_identity;

/// Rows per answer (FR-NEW-007, FR-NEW-019).
pub const PAGE: usize = 100;
const NAME_MAX: usize = 64;
const MODEL_NAME_MAX: usize = 128;
const ENDPOINT_MAX: usize = 2048;
const SECRET_MAX: usize = 4096;

pub(super) fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// The value of the out of date marker: milliseconds, so two changes of one
/// second hold two values and a recalculation that compares the marker it read
/// with the one stored cannot clear a change it did not see (SPEC-0108 DEC-006).
pub(super) fn marker_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn invalid(text: impl Into<String>) -> ToolError {
    ToolError::invalid_argument(text)
}

fn provider_not_found(provider_id: &str) -> ToolError {
    ToolError::not_found(format!("provider not found: {provider_id}"))
}

/// The endpoint rule of FR-NEW-001: http or https, a host, no credentials, at
/// most 2048 characters. The refusal never quotes the value, which may carry
/// a password (E2E-112).
fn check_endpoint(endpoint: &str) -> Result<()> {
    let refused = || invalid("endpoint must be an http or https URL");
    if endpoint.chars().count() > ENDPOINT_MAX {
        return Err(refused());
    }
    let url = url::Url::parse(endpoint).map_err(|_| refused())?;
    let has_host = url.host_str().is_some_and(|h| !h.is_empty());
    let credentials = !url.username().is_empty() || url.password().is_some();
    if !matches!(url.scheme(), "http" | "https") || !has_host || credentials {
        return Err(refused());
    }
    Ok(())
}

/// The checks of FR-NEW-002 that follow the duplicate name, on the state the
/// provider will have: `has_secret` after the operation, `new_secret` when a
/// secret is being written.
fn check_fields(
    endpoint: &str,
    auth_scheme: &str,
    has_secret: bool,
    new_secret: Option<&SecretText>,
    key: Option<[u8; 32]>,
) -> Result<()> {
    check_endpoint(endpoint)?;
    if auth_scheme != AUTH_NONE && auth_scheme != AUTH_BEARER {
        return Err(invalid("auth_scheme must be 'none' or 'bearer'"));
    }
    if auth_scheme == AUTH_NONE && has_secret {
        return Err(invalid("auth_scheme 'none' takes no secret"));
    }
    if auth_scheme == AUTH_BEARER && !has_secret {
        return Err(invalid("auth_scheme 'bearer' needs a secret"));
    }
    if let Some(secret) = new_secret {
        if secret.chars() > SECRET_MAX {
            return Err(invalid("secret must be at most 4096 characters"));
        }
        if key.is_none() {
            return Err(ToolError::not_supported("no master key is configured; set FSA_TOKEN_KEY"));
        }
    }
    Ok(())
}

fn check_name(name: &str) -> Result<()> {
    let n = name.chars().count();
    if n == 0 || n > NAME_MAX {
        return Err(invalid("name must be 1 to 64 characters"));
    }
    Ok(())
}

fn duplicate(name: &str) -> ToolError {
    invalid(format!("a provider named '{name}' already exists"))
}

/// What a change record names.
pub(super) struct Target<'a> {
    kind: &'static str,
    id: &'a str,
    name: &'a str,
}

impl<'a> Target<'a> {
    fn provider(p: &'a Provider) -> Self {
        Self { kind: "provider", id: &p.provider_id, name: &p.name }
    }

    fn model(m: &'a Model) -> Self {
        Self { kind: "model", id: &m.model_id, name: &m.name }
    }

    pub(super) fn of(kind: &'static str, id: &'a str, name: &'a str) -> Self {
        Self { kind, id, name }
    }

    pub(super) fn converter(c: &'a super::converter::Converter) -> Self {
        Self { kind: "converter", id: &c.converter_id, name: &c.name }
    }
}

fn model_not_found(model_id: &str) -> ToolError {
    ToolError::not_found(format!("model not found: {model_id}"))
}

fn kind_text() -> String {
    format!("kind must be one of {}", KINDS.join(", "))
}

fn check_default_kind(kind: &str) -> Result<()> {
    if DEFAULT_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(invalid("kind must be 'text_embedding' or 'reranker'"))
    }
}

fn is_embedding(kind: &str) -> bool {
    kind.ends_with("_embedding")
}

/// The checks of FR-NEW-009 that follow the provider and the name: kind,
/// dimension (required, applies, range), then the token limit. `dimension` is
/// the value after the operation.
fn check_model_fields(
    kind: &str,
    dimension: Option<i64>,
    max_input_tokens: Option<i64>,
) -> Result<()> {
    if !KINDS.contains(&kind) {
        return Err(invalid(kind_text()));
    }
    match (is_embedding(kind), dimension) {
        (true, None) => return Err(invalid(format!("dimension is required for kind '{kind}'"))),
        (false, Some(_)) => return Err(invalid("dimension applies to embedding kinds only")),
        (true, Some(d)) if !(1..=8192).contains(&d) => {
            return Err(invalid("dimension must be between 1 and 8192"));
        }
        _ => {}
    }
    if max_input_tokens.is_some_and(|n| !(1..=1_000_000).contains(&n)) {
        return Err(invalid("max_input_tokens must be between 1 and 1000000"));
    }
    Ok(())
}

fn check_model_name(name: &str) -> Result<()> {
    let n = name.chars().count();
    if n == 0 || n > MODEL_NAME_MAX {
        return Err(invalid("name must be 1 to 128 characters"));
    }
    Ok(())
}

fn model_duplicate(name: &str, provider: &str) -> ToolError {
    invalid(format!("a model named '{name}' already exists for provider '{provider}'"))
}

pub struct Registry {
    pub(super) store: SettingsStore,
    pub(super) key: Option<[u8; 32]>,
    /// `config.search.embedding.dimensions`: the width of the stored vectors,
    /// which a default text embedding model must match (DEC-012).
    vector_dimension: u32,
}

impl Registry {
    pub fn new(store: SettingsStore, vector_dimension: u32) -> Self {
        let key = store.key();
        Self { store, key, vector_dimension }
    }

    /// The registry of an admin store; a store without settings tables (a test
    /// double) cannot hold a registry.
    pub fn of(admin: &dyn AdminBackend, vector_dimension: u32) -> Result<Self> {
        admin
            .settings()
            .map(|store| Self::new(store, vector_dimension))
            .ok_or_else(|| ToolError::not_supported("the platform settings store is not available"))
    }

    /// A primary key conflict between two concurrent writers (the change id or
    /// the name) surfaces as an internal error; one retry reads the new state,
    /// where the name check then answers in words (DEC-004).
    pub(super) fn retry(attempt: usize, e: &ToolError) -> bool {
        attempt == 0 && e.code == code::INTERNAL_ERROR
    }

    pub async fn create_provider(&self, person: &str, input: ProviderInput) -> Result<Provider> {
        let mut attempt = 0;
        loop {
            match self.try_create_provider(person, &input).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_create_provider(&self, person: &str, input: &ProviderInput) -> Result<Provider> {
        let name = input.name.trim().to_string();
        let endpoint = input.endpoint.trim().to_string();
        // Only an absent scheme defaults to none, in the callers; an explicit empty one is refused.
        let auth_scheme = input.auth_scheme.clone();
        check_name(&name)?;
        let mut tx = self.store.begin().await?;
        if SettingsStore::provider_id_of_key(&mut tx, &name_key(&name)).await?.is_some() {
            return Err(duplicate(&name));
        }
        let new_secret = (!input.secret.is_empty()).then_some(&input.secret);
        check_fields(&endpoint, &auth_scheme, new_secret.is_some(), new_secret, self.key)?;
        let secret_enc = match (new_secret, self.key) {
            (Some(s), Some(k)) => Some(secrets::encrypt(&k, s)?),
            _ => None,
        };
        let at = now();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let row = ProviderRow {
            provider: Provider {
                provider_id: format!("prov-{}", &id[..12]),
                name,
                endpoint,
                auth_scheme,
                has_secret: secret_enc.is_some(),
                created_at: at.clone(),
                updated_at: at.clone(),
            },
            secret_enc,
        };
        SettingsStore::insert_provider(&mut tx, &row).await?;
        self.record(
            &mut tx,
            person,
            "create_provider",
            Target::provider(&row.provider),
            vec![],
            at,
        )
        .await?;
        tx.commit().await?;
        Ok(row.provider)
    }

    pub async fn update_provider(
        &self,
        person: &str,
        provider_id: &str,
        update: ProviderUpdate,
    ) -> Result<Provider> {
        let mut attempt = 0;
        loop {
            match self.try_update_provider(person, provider_id, &update).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_update_provider(
        &self,
        person: &str,
        provider_id: &str,
        u: &ProviderUpdate,
    ) -> Result<Provider> {
        let mut tx = self.store.begin().await?;
        let Some(current) = SettingsStore::provider_by_id(&mut tx, provider_id).await? else {
            return Err(provider_not_found(provider_id));
        };
        // An empty secret keeps the stored one, as an empty field on the screen does.
        let new_secret = u.secret.as_ref().filter(|s| !s.is_empty());
        if new_secret.is_some() && u.clear_secret {
            return Err(invalid("secret and clear_secret cannot be given together"));
        }
        let cur = &current.provider;
        let name = u.name.as_deref().map_or_else(|| cur.name.clone(), |n| n.trim().to_string());
        let endpoint =
            u.endpoint.as_deref().map_or_else(|| cur.endpoint.clone(), |e| e.trim().to_string());
        let auth_scheme = u.auth_scheme.clone().unwrap_or_else(|| cur.auth_scheme.clone());
        let clears = u.clear_secret && cur.has_secret;
        let has_secret = new_secret.is_some() || (cur.has_secret && !u.clear_secret);

        check_name(&name)?;
        if let Some(owner) = SettingsStore::provider_id_of_key(&mut tx, &name_key(&name)).await?
            && owner != cur.provider_id
        {
            return Err(duplicate(&name));
        }
        check_fields(&endpoint, &auth_scheme, has_secret, new_secret, self.key)?;

        let mut fields = Vec::new();
        if name != cur.name {
            fields.push("name".to_string());
        }
        if endpoint != cur.endpoint {
            fields.push("endpoint".to_string());
        }
        if auth_scheme != cur.auth_scheme {
            fields.push("auth_scheme".to_string());
        }
        if new_secret.is_some() || clears {
            fields.push("secret".to_string());
        }
        if fields.is_empty() {
            return Ok(current.provider);
        }
        let secret_enc = match (new_secret, self.key) {
            (Some(s), Some(k)) => Some(secrets::encrypt(&k, s)?),
            _ if u.clear_secret => None,
            _ => current.secret_enc.clone(),
        };
        let at = now();
        let row = ProviderRow {
            provider: Provider {
                provider_id: cur.provider_id.clone(),
                name,
                endpoint,
                auth_scheme,
                has_secret: secret_enc.is_some(),
                created_at: cur.created_at.clone(),
                updated_at: at.clone(),
            },
            secret_enc,
        };
        SettingsStore::update_provider(&mut tx, &row).await?;
        self.record(
            &mut tx,
            person,
            "update_provider",
            Target::provider(&row.provider),
            fields,
            at,
        )
        .await?;
        tx.commit().await?;
        Ok(row.provider)
    }

    /// Removes the provider and its secret with it (FR-NEW-006).
    pub async fn delete_provider(&self, person: &str, provider_id: &str) -> Result<()> {
        let mut attempt = 0;
        loop {
            match self.try_delete_provider(person, provider_id).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_delete_provider(&self, person: &str, provider_id: &str) -> Result<()> {
        let mut tx = self.store.begin().await?;
        let Some(current) = SettingsStore::provider_by_id(&mut tx, provider_id).await? else {
            return Err(provider_not_found(provider_id));
        };
        let models = SettingsStore::count_models(&mut tx, provider_id).await?;
        if models > 0 {
            return Err(invalid(format!(
                "provider '{}' still has {models} model(s)",
                current.provider.name
            )));
        }
        SettingsStore::delete_provider(&mut tx, provider_id).await?;
        let target = Target::provider(&current.provider);
        self.record(&mut tx, person, "delete_provider", target, vec![], now()).await?;
        tx.commit().await
    }

    pub async fn list_providers(&self, marker: &str) -> Result<Page<Provider>> {
        let after = if marker.is_empty() { None } else { Some(decode_marker(marker)?.0) };
        let mut rows = self.store.list_providers(after.as_deref(), PAGE + 1).await?;
        let more = rows.len() > PAGE;
        rows.truncate(PAGE);
        let marker = more.then(|| {
            let last = &rows.last().expect("a full page").provider;
            encode_marker(&name_key(&last.name), &last.provider_id)
        });
        Ok(Page { items: rows.into_iter().map(|r| r.provider).collect(), marker })
    }

    pub async fn create_model(&self, person: &str, input: ModelInput) -> Result<Model> {
        let mut attempt = 0;
        loop {
            match self.try_create_model(person, &input).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_create_model(&self, person: &str, input: &ModelInput) -> Result<Model> {
        let name = input.name.trim().to_string();
        let mut tx = self.store.begin().await?;
        let Some(provider) = SettingsStore::provider_by_id(&mut tx, &input.provider_id).await?
        else {
            return Err(provider_not_found(&input.provider_id));
        };
        check_model_name(&name)?;
        check_model_fields(&input.kind, input.dimension, input.max_input_tokens)?;
        let provider = provider.provider;
        if SettingsStore::model_id_of_key(&mut tx, &provider.provider_id, &name_key(&name))
            .await?
            .is_some()
        {
            return Err(model_duplicate(&name, &provider.name));
        }
        let at = now();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let model = Model {
            model_id: format!("mdl-{}", &id[..12]),
            provider_id: provider.provider_id,
            name,
            kind: input.kind.clone(),
            dimension: input.dimension,
            max_input_tokens: input.max_input_tokens,
            is_default: false,
            created_at: at.clone(),
            updated_at: at.clone(),
        };
        SettingsStore::insert_model(&mut tx, &model).await?;
        self.record(&mut tx, person, "create_model", Target::model(&model), vec![], at).await?;
        tx.commit().await?;
        Ok(model)
    }

    pub async fn update_model(
        &self,
        person: &str,
        model_id: &str,
        update: ModelUpdate,
    ) -> Result<Model> {
        let mut attempt = 0;
        loop {
            match self.try_update_model(person, model_id, &update).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_update_model(
        &self,
        person: &str,
        model_id: &str,
        u: &ModelUpdate,
    ) -> Result<Model> {
        let mut tx = self.store.begin().await?;
        let Some(cur) = SettingsStore::model_by_id(&mut tx, model_id).await? else {
            return Err(model_not_found(model_id));
        };
        let name = u.name.as_deref().map_or_else(|| cur.name.clone(), |n| n.trim().to_string());
        let dimension = u.dimension.or(cur.dimension);
        // A limit of 0 removes it, so it is never range checked as a value.
        let max_input_tokens = match u.max_input_tokens {
            Some(0) => None,
            Some(n) => Some(n),
            None => cur.max_input_tokens,
        };
        check_model_name(&name)?;
        check_model_fields(&cur.kind, dimension, max_input_tokens)?;
        // Vectors already stored have the dimension of the default, so it is frozen
        // while the model is one; the same value is not a change. After the field
        // checks, so a malformed value is told as such first.
        if cur.is_default && u.dimension.is_some_and(|d| Some(d) != cur.dimension) {
            return Err(invalid("the dimension of a server default model cannot change"));
        }
        if let Some(owner) =
            SettingsStore::model_id_of_key(&mut tx, &cur.provider_id, &name_key(&name)).await?
            && owner != cur.model_id
        {
            let provider = SettingsStore::provider_by_id(&mut tx, &cur.provider_id).await?;
            let provider = provider.map(|p| p.provider.name).unwrap_or_default();
            return Err(model_duplicate(&name, &provider));
        }
        let mut fields = Vec::new();
        if name != cur.name {
            fields.push("name".to_string());
        }
        if dimension != cur.dimension {
            fields.push("dimension".to_string());
        }
        if max_input_tokens != cur.max_input_tokens {
            fields.push("max_input_tokens".to_string());
        }
        if fields.is_empty() {
            return Ok(cur);
        }
        let at = now();
        let model = Model { name, dimension, max_input_tokens, updated_at: at.clone(), ..cur };
        SettingsStore::update_model(&mut tx, &model).await?;
        // A vector made with another width or another model name is no longer the
        // one the model gives (SPEC-0108 DEC-004): its projects read out of date.
        if fields.iter().any(|f| f == "dimension" || f == "name") {
            SettingsStore::mark_model_projects_stale(&mut tx, &model.model_id, &marker_now())
                .await?;
        }
        self.record(&mut tx, person, "update_model", Target::model(&model), fields, at).await?;
        tx.commit().await?;
        Ok(model)
    }

    pub async fn delete_model(&self, person: &str, model_id: &str) -> Result<()> {
        let mut attempt = 0;
        loop {
            match self.try_delete_model(person, model_id).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_delete_model(&self, person: &str, model_id: &str) -> Result<()> {
        let mut tx = self.store.begin().await?;
        let Some(current) = SettingsStore::model_by_id(&mut tx, model_id).await? else {
            return Err(model_not_found(model_id));
        };
        if current.is_default {
            return Err(invalid(format!(
                "model '{}' is a server default; clear the default first",
                current.name
            )));
        }
        let used = SettingsStore::count_model_projects(&mut tx, model_id).await?;
        if used > 0 {
            return Err(invalid(format!("model '{}' is used by {used} project(s)", current.name)));
        }
        SettingsStore::delete_model(&mut tx, model_id).await?;
        self.record(&mut tx, person, "delete_model", Target::model(&current), vec![], now())
            .await?;
        tx.commit().await
    }

    /// One keyset page; the marker holds the two sort keys as a JSON list so a
    /// name with any character cannot split it (DEC-010).
    pub async fn list_models(
        &self,
        provider_id: &str,
        kind: &str,
        marker: &str,
    ) -> Result<Page<Model>> {
        if !provider_id.is_empty() && self.store.provider_exists(provider_id).await?.is_none() {
            return Err(provider_not_found(provider_id));
        }
        if !kind.is_empty() && !KINDS.contains(&kind) {
            return Err(invalid(kind_text()));
        }
        let after = if marker.is_empty() {
            None
        } else {
            let keys: Vec<String> = serde_json::from_str(&decode_marker(marker)?.0)
                .ok()
                .filter(|k: &Vec<String>| k.len() == 2)
                .ok_or_else(|| invalid("marker is not valid"))?;
            Some((keys[0].clone(), keys[1].clone()))
        };
        let after_ref = after.as_ref().map(|(p, m)| (p.as_str(), m.as_str()));
        let mut rows = self.store.list_models(provider_id, kind, after_ref, PAGE + 1).await?;
        let more = rows.len() > PAGE;
        rows.truncate(PAGE);
        let marker = more.then(|| {
            let (last, provider_key) = rows.last().expect("a full page");
            let keys = serde_json::json!([provider_key, name_key(&last.name)]).to_string();
            encode_marker(&keys, &last.model_id)
        });
        Ok(Page { items: rows.into_iter().map(|(m, _)| m).collect(), marker })
    }

    /// Replaces the default of `kind` by `model_id` (FR-NEW-013). Setting the
    /// model that already is the default writes nothing.
    pub async fn set_default_model(
        &self,
        person: &str,
        kind: &str,
        model_id: &str,
    ) -> Result<Model> {
        let mut attempt = 0;
        loop {
            match self.try_set_default(person, kind, model_id).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_set_default(&self, person: &str, kind: &str, model_id: &str) -> Result<Model> {
        check_default_kind(kind)?;
        let mut tx = self.store.begin().await?;
        let Some(model) = SettingsStore::model_by_id(&mut tx, model_id).await? else {
            return Err(model_not_found(model_id));
        };
        if model.kind != kind {
            return Err(invalid(format!(
                "model '{}' is of kind '{}', not '{kind}'",
                model.name, model.kind
            )));
        }
        if kind == "text_embedding" && model.dimension != Some(i64::from(self.vector_dimension)) {
            return Err(invalid(format!(
                "the default text embedding model must have dimension {}",
                self.vector_dimension
            )));
        }
        if model.is_default {
            return Ok(model);
        }
        let at = now();
        SettingsStore::set_default(&mut tx, kind, model_id, &at).await?;
        let target = Target { kind: "default", id: &model.model_id, name: kind };
        self.record(&mut tx, person, "set_default", target, vec![], at).await?;
        tx.commit().await?;
        Ok(Model { is_default: true, ..model })
    }

    /// Removes the default of `kind`; a kind with none is not an error and
    /// writes no record (FR-NEW-014).
    pub async fn clear_default_model(&self, person: &str, kind: &str) -> Result<()> {
        let mut attempt = 0;
        loop {
            match self.try_clear_default(person, kind).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_clear_default(&self, person: &str, kind: &str) -> Result<()> {
        check_default_kind(kind)?;
        let mut tx = self.store.begin().await?;
        let Some(model_id) = SettingsStore::default_id(&mut tx, kind).await? else {
            return Ok(());
        };
        SettingsStore::clear_default(&mut tx, kind).await?;
        let target = Target { kind: "default", id: &model_id, name: kind };
        self.record(&mut tx, person, "clear_default", target, vec![], now()).await?;
        tx.commit().await
    }

    pub async fn default_models(&self) -> Result<Defaults> {
        Ok(Defaults {
            text_embedding: self.store.default_model("text_embedding").await?,
            reranker: self.store.default_model("reranker").await?,
        })
    }

    pub async fn list_changes(&self, marker: &str) -> Result<Page<Change>> {
        let before = if marker.is_empty() { None } else { Some(decode_marker(marker)?.1) };
        let mut rows = self.store.list_changes(before.as_deref(), PAGE + 1).await?;
        let more = rows.len() > PAGE;
        rows.truncate(PAGE);
        let marker = more.then(|| {
            let last = rows.last().expect("a full page");
            encode_marker(&last.at, &last.change_id)
        });
        Ok(Page { items: rows, marker })
    }

    /// The newest `limit` changes of the given target kinds, newest first.
    pub async fn list_changes_of_kinds(&self, kinds: &[&str], limit: usize) -> Result<Vec<Change>> {
        self.store.list_changes_of_kinds(kinds, limit).await
    }

    pub(super) async fn record(
        &self,
        tx: &mut Box<dyn crate::storage::rel::RelationalTx>,
        person: &str,
        action: &str,
        target: Target<'_>,
        fields: Vec<String>,
        at: String,
    ) -> Result<()> {
        let change = Change {
            change_id: String::new(),
            at,
            person: normalize_identity(person),
            action: action.into(),
            target_kind: target.kind.into(),
            target_id: target.id.into(),
            target_name: target.name.into(),
            fields,
        };
        SettingsStore::record(tx, change).await
    }
}

/// Where search embeds (SPEC-0102 DEC-007): the default text embedding model
/// joined with its provider when one is set, else the YAML block. Read on every
/// call, one admin query, so a registry change applies at once.
pub struct RegistryTargets {
    pub admin: std::sync::Arc<dyn AdminBackend>,
    pub fallback: crate::config::EmbeddingConfig,
}

impl RegistryTargets {
    /// The `url` (endpoint plus `suffix`) and decrypted bearer of a model's provider.
    fn target_of(
        store: &SettingsStore,
        model: &Model,
        row: &super::store::ProviderRow,
        suffix: &str,
    ) -> Result<(String, String, Option<SecretText>)> {
        let bearer = match (row.provider.auth_scheme.as_str(), &row.secret_enc) {
            (AUTH_BEARER, Some(blob)) => {
                Some(secrets::decrypt(store.key(), blob).ok_or_else(|| {
                    ToolError::internal(format!(
                        "the secret of provider '{}' cannot be read",
                        row.provider.name
                    ))
                })?)
            }
            _ => None,
        };
        let url = format!("{}{suffix}", row.provider.endpoint.trim_end_matches('/'));
        Ok((url, model.name.clone(), bearer))
    }

    /// The default model of `kind` with its provider's `url` (endpoint plus
    /// `suffix`) and decrypted bearer, or `None` when the kind has no default.
    async fn default_of(
        &self,
        kind: &str,
        suffix: &str,
    ) -> Result<Option<(String, String, Option<SecretText>)>> {
        let found = match self.admin.settings() {
            Some(store) => store.default_with_provider(kind).await?.map(|d| (store, d)),
            None => None,
        };
        let Some((store, (model, row))) = found else {
            return Ok(None);
        };
        Self::target_of(&store, &model, &row, suffix).map(Some)
    }

    /// The model a project's profile names for `kind`, as a target of its
    /// declared width; `None` when the profile names none (SPEC-0108 FR-NEW-001).
    async fn profile_model(
        &self,
        project: &str,
        kind: &str,
    ) -> Result<Option<crate::search::embedding::ProjectTarget>> {
        let Some(store) = self.admin.settings() else { return Ok(None) };
        let Some(profile) = store.profile_of(project).await? else { return Ok(None) };
        let id = match kind {
            "image_embedding" => profile.image_model_id,
            _ => profile.text_model_id,
        };
        let Some(id) = id else { return Ok(None) };
        let Some((model, row)) = store.model_with_provider(&id).await? else { return Ok(None) };
        let (url, name, bearer) = Self::target_of(&store, &model, &row, "/embeddings")?;
        Ok(Some(crate::search::embedding::ProjectTarget {
            target: crate::search::embedding::EmbedTarget { url, model: name, bearer },
            dimension: model.dimension.and_then(|d| u32::try_from(d).ok()),
        }))
    }
}

#[async_trait::async_trait]
impl crate::search::embedding::EmbedTargets for RegistryTargets {
    async fn target(&self) -> Result<crate::search::embedding::EmbedTarget> {
        Ok(match self.default_of("text_embedding", "/embeddings").await? {
            Some((url, model, bearer)) => {
                crate::search::embedding::EmbedTarget { url, model, bearer }
            }
            None => crate::search::embedding::StaticTargets::target_of(&self.fallback)?,
        })
    }

    async fn target_for(&self, project: &str) -> Result<crate::search::embedding::ProjectTarget> {
        match self.profile_model(project, "text_embedding").await? {
            Some(found) => Ok(found),
            None => Ok(crate::search::embedding::ProjectTarget {
                target: self.target().await?,
                dimension: None,
            }),
        }
    }

    async fn image_target_for(
        &self,
        project: &str,
    ) -> Result<Option<crate::search::embedding::ProjectTarget>> {
        self.profile_model(project, "image_embedding").await
    }
}

/// The default reranker joined with its provider; no default, no target: the
/// YAML `search.reranking` block is never used (SPEC-0102 DEC-008).
#[async_trait::async_trait]
impl crate::search::rerank::RerankTargets for RegistryTargets {
    async fn target(&self) -> Result<Option<crate::search::rerank::RerankTarget>> {
        Ok(self
            .default_of("reranker", "/rerank")
            .await?
            .map(|(url, model, bearer)| crate::search::rerank::RerankTarget { url, model, bearer }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_endpoint_is_http_or_https_with_a_host_and_no_credentials() {
        for ok in ["http://localhost:9010/v1", "https://api.example.com/v1"] {
            assert!(check_endpoint(ok).is_ok(), "{ok}");
        }
        let long = format!("http://h/{}", "a".repeat(2048));
        for bad in [
            "localhost:9010",
            "ftp://x",
            "http://user:pw@host:9010/v1",
            "http://user@host/",
            "http://",
            "",
            long.as_str(),
        ] {
            let e = check_endpoint(bad).unwrap_err();
            assert_eq!(
                e.to_string(),
                "ERR_INVALID_ARGUMENT: endpoint must be an http or https URL"
            );
        }
    }

    #[test]
    fn a_name_holds_1_to_64_characters_counted_as_characters() {
        assert!(check_name("é").is_ok());
        assert!(check_name(&"é".repeat(64)).is_ok());
        assert!(check_name(&"é".repeat(65)).is_err());
        assert!(check_name("").is_err());
    }

    #[test]
    fn a_secret_without_a_master_key_is_refused_after_the_length_rule() {
        let s = SecretText::new("sk-test-123");
        let e = check_fields("http://h/v1", "bearer", true, Some(&s), None).unwrap_err();
        assert_eq!(
            e.to_string(),
            "ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY"
        );
        let long = SecretText::new("x".repeat(4097));
        let e = check_fields("http://h/v1", "bearer", true, Some(&long), None).unwrap_err();
        assert_eq!(e.to_string(), "ERR_INVALID_ARGUMENT: secret must be at most 4096 characters");
        assert!(check_fields("http://h/v1", "none", false, None, None).is_ok());
    }
}
