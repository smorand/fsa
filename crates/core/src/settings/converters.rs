//! The converter operations of [`Registry`] (SPEC-0106 DEC-002): checks in the
//! order of the spec, a change and its record in one transaction, as for the
//! model registry.

use super::converter::{
    Converter, ConverterInput, DEFAULT_MAX_INPUT_BYTES, DEFAULT_TIMEOUT_SECONDS, FORM_HTTP,
};
use super::converter_rules as rules;
use super::converter_store::ConverterRow;
use super::model::{Page, decode_marker, encode_marker};
use super::registry::{PAGE, Registry, Target, now};
use super::store::{SettingsStore, name_key};
use crate::errors::{Result, ToolError};
use serde_json::{Value, json};

fn converter_not_found(converter_id: &str) -> ToolError {
    ToolError::not_found(format!("converter not found: {converter_id}"))
}

/// The name a change record gives a converter that may be gone by now.
async fn name_of(
    tx: &mut Box<dyn crate::storage::rel::RelationalTx>,
    converter_id: &str,
) -> Result<String> {
    Ok(SettingsStore::converter_by_id(tx, converter_id)
        .await?
        .map(|r| r.converter.name)
        .unwrap_or_default())
}

/// `{converters, marker}` with the marker `null` on the last page (FR-NEW-006).
pub fn page_json(page: &Page<Converter>) -> Value {
    json!({"converters": page.items, "marker": page.marker})
}

impl Registry {
    pub async fn create_converter(&self, person: &str, input: ConverterInput) -> Result<Converter> {
        let mut attempt = 0;
        loop {
            match self.try_create_converter(person, &input).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_create_converter(&self, person: &str, i: &ConverterInput) -> Result<Converter> {
        let name = rules::name(&i.name)?;
        let mut tx = self.store.begin().await?;
        if SettingsStore::converter_id_of_key(&mut tx, &name_key(&name)).await?.is_some() {
            return Err(rules::duplicate(&name));
        }
        let form = rules::form(&i.form)?;
        let http = if form == FORM_HTTP {
            let h = rules::http_fields(
                &i.url,
                &i.command,
                &i.auth_header,
                &i.secret,
                &i.file_field,
                &i.response_field,
                false,
            )?;
            Some(h)
        } else {
            None
        };
        let command = match http {
            Some(_) => None,
            None => {
                let command = rules::program_command(&i.command)?;
                rules::no_http_field([
                    &i.url,
                    &i.auth_header,
                    &i.secret,
                    &i.file_field,
                    &i.response_field,
                ])?;
                Some(command)
            }
        };
        let extensions = rules::extensions(&i.extensions)?;
        let max_input_bytes = rules::max_input_bytes(&i.max_input_bytes, DEFAULT_MAX_INPUT_BYTES)?;
        let timeout_seconds = rules::timeout_seconds(&i.timeout_seconds, DEFAULT_TIMEOUT_SECONDS)?;
        let secret_enc =
            rules::http_secret(http.as_ref().and_then(|h| h.secret.as_ref()), self.key)?;

        let at = now();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let row = ConverterRow {
            converter: Converter {
                converter_id: format!("conv-{}", &id[..12]),
                name,
                form: form.into(),
                command,
                url: http.as_ref().map(|h| h.url.clone()),
                auth_header: http.as_ref().map(|h| h.auth_header.clone()),
                has_secret: secret_enc.is_some(),
                file_field: http.as_ref().map(|h| h.file_field.clone()),
                response_field: http.map(|h| h.response_field),
                extensions,
                max_input_bytes,
                timeout_seconds,
                created_at: at.clone(),
                updated_at: at.clone(),
            },
            secret_enc,
        };
        SettingsStore::insert_converter(&mut tx, &row).await?;
        let target = Target::converter(&row.converter);
        self.record(&mut tx, person, "create_converter", target, vec![], at).await?;
        tx.commit().await?;
        Ok(row.converter)
    }

    /// Applies the fields given (FR-NEW-005) and answers the converter. A field
    /// not given keeps its stored value, so the creation checks run on the
    /// converter as it is after the update. One transaction, so a refusal
    /// changes nothing.
    pub async fn update_converter(
        &self,
        person: &str,
        converter_id: &str,
        input: ConverterInput,
    ) -> Result<Converter> {
        let mut attempt = 0;
        loop {
            match self.try_update_converter(person, converter_id, &input).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_update_converter(
        &self,
        person: &str,
        converter_id: &str,
        i: &ConverterInput,
    ) -> Result<Converter> {
        let mut tx = self.store.begin().await?;
        let Some(current) = SettingsStore::converter_by_id(&mut tx, converter_id).await? else {
            return Err(ToolError::not_found(format!("converter not found: {converter_id}")));
        };
        let cur = &current.converter;
        if rules::given(&i.form).is_some_and(|f| f.as_str() != Some(cur.form.as_str())) {
            return Err(ToolError::invalid_argument("form cannot be changed"));
        }
        if rules::given(&i.converter_id).is_some_and(|v| v.as_str() != Some(converter_id)) {
            return Err(ToolError::invalid_argument("converter_id in the body does not match"));
        }
        let is_http = cur.form == FORM_HTTP;
        let clear = rules::clear_secret(&i.clear_secret)?;
        if clear && !is_http {
            return Err(rules::clear_secret_of_program());
        }
        // A blank secret is no secret: it keeps the stored one and does not clash with clear_secret.
        let new_secret = if is_http { rules::new_secret(&i.secret)? } else { None };
        if new_secret.is_some() && clear {
            return Err(ToolError::invalid_argument(
                "secret and clear_secret cannot be given together",
            ));
        }

        let name = match rules::given(&i.name) {
            None => cur.name.clone(),
            Some(_) => rules::name(&i.name)?,
        };
        // The name key ignores this converter, so a change of letter case is a rename.
        if let Some(owner) = SettingsStore::converter_id_of_key(&mut tx, &name_key(&name)).await?
            && owner != cur.converter_id
        {
            return Err(rules::duplicate(&name));
        }
        let keeps_secret = cur.has_secret && !clear;
        let mut next = cur.clone();
        next.name = name;
        if is_http {
            let h = rules::http_fields(
                &rules::or_current(&i.url, &cur.url),
                &i.command,
                &rules::or_current(&i.auth_header, &cur.auth_header),
                &i.secret,
                &rules::or_current(&i.file_field, &cur.file_field),
                &rules::or_current(&i.response_field, &cur.response_field),
                keeps_secret,
            )?;
            next.url = Some(h.url);
            next.auth_header = Some(h.auth_header);
            next.file_field = Some(h.file_field);
            next.response_field = Some(h.response_field);
        } else {
            next.command =
                Some(rules::program_command(&rules::or_current(&i.command, &cur.command))?);
            rules::no_http_field([
                &i.url,
                &i.auth_header,
                &i.secret,
                &i.file_field,
                &i.response_field,
            ])?;
        }
        next.extensions = rules::extensions(&rules::or_current(&i.extensions, &cur.extensions))?;
        next.max_input_bytes = rules::max_input_bytes(&i.max_input_bytes, cur.max_input_bytes)?;
        next.timeout_seconds = rules::timeout_seconds(&i.timeout_seconds, cur.timeout_seconds)?;
        let new_enc = rules::http_secret(new_secret.as_ref(), self.key)?;

        // The record names the fields in the order of FR-NEW-016, never a value.
        let secret_changes = new_enc.is_some() || (clear && cur.has_secret);
        let fields: Vec<String> = [
            ("name", next.name != cur.name),
            ("command", next.command != cur.command),
            ("url", next.url != cur.url),
            ("auth_header", next.auth_header != cur.auth_header),
            ("secret", secret_changes),
            ("file_field", next.file_field != cur.file_field),
            ("response_field", next.response_field != cur.response_field),
            ("extensions", next.extensions != cur.extensions),
            ("max_input_bytes", next.max_input_bytes != cur.max_input_bytes),
            ("timeout_seconds", next.timeout_seconds != cur.timeout_seconds),
        ]
        .into_iter()
        .filter(|(_, changed)| *changed)
        .map(|(f, _)| f.to_string())
        .collect();
        if fields.is_empty() {
            return Ok(current.converter);
        }
        let secret_enc = match (new_enc, clear) {
            (Some(enc), _) => Some(enc),
            (None, true) => None,
            (None, false) => current.secret_enc.clone(),
        };
        let at = now();
        next.has_secret = secret_enc.is_some();
        next.updated_at = at.clone();
        let row = ConverterRow { converter: next, secret_enc };
        SettingsStore::update_converter(&mut tx, &row).await?;
        self.record(
            &mut tx,
            person,
            "update_converter",
            Target::converter(&row.converter),
            fields,
            at,
        )
        .await?;
        tx.commit().await?;
        Ok(row.converter)
    }

    /// Removes a converter and its secret (FR-NEW-007) unless it is the platform
    /// default or the converter of a project; the default is judged first.
    pub async fn delete_converter(&self, person: &str, converter_id: &str) -> Result<()> {
        let mut attempt = 0;
        loop {
            match self.try_delete_converter(person, converter_id).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_delete_converter(&self, person: &str, converter_id: &str) -> Result<()> {
        let mut tx = self.store.begin().await?;
        let Some(row) = SettingsStore::converter_by_id(&mut tx, converter_id).await? else {
            return Err(converter_not_found(converter_id));
        };
        let c = row.converter;
        if SettingsStore::default_converter_id(&mut tx).await?.as_deref() == Some(converter_id) {
            return Err(ToolError::invalid_argument(format!(
                "converter '{}' is the platform default",
                c.name
            )));
        }
        let used = SettingsStore::count_converter_projects(&mut tx, converter_id).await?;
        if used > 0 {
            return Err(ToolError::invalid_argument(format!(
                "converter '{}' is used by {used} project(s)",
                c.name
            )));
        }
        SettingsStore::delete_converter(&mut tx, converter_id).await?;
        self.record(&mut tx, person, "delete_converter", Target::converter(&c), vec![], now())
            .await?;
        tx.commit().await
    }

    /// Makes a converter the platform default (FR-NEW-008); setting the current
    /// one again writes no record.
    pub async fn set_default_converter(
        &self,
        person: &str,
        converter_id: &str,
    ) -> Result<Converter> {
        let mut attempt = 0;
        loop {
            match self.try_set_default_converter(person, converter_id).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_set_default_converter(
        &self,
        person: &str,
        converter_id: &str,
    ) -> Result<Converter> {
        let mut tx = self.store.begin().await?;
        let Some(row) = SettingsStore::converter_by_id(&mut tx, converter_id).await? else {
            return Err(converter_not_found(converter_id));
        };
        let c = row.converter;
        if SettingsStore::default_converter_id(&mut tx).await?.as_deref() == Some(converter_id) {
            return Ok(c);
        }
        let at = now();
        SettingsStore::set_default_converter(&mut tx, converter_id).await?;
        let target = Target::of("default_converter", &c.converter_id, &c.name);
        self.record(&mut tx, person, "set_default_converter", target, vec![], at).await?;
        tx.commit().await?;
        Ok(c)
    }

    /// Removes the platform default; none set is not an error and writes no
    /// record.
    pub async fn clear_default_converter(&self, person: &str) -> Result<()> {
        let mut attempt = 0;
        loop {
            match self.try_clear_default_converter(person).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_clear_default_converter(&self, person: &str) -> Result<()> {
        let mut tx = self.store.begin().await?;
        let Some(id) = SettingsStore::default_converter_id(&mut tx).await? else {
            return Ok(());
        };
        let name = name_of(&mut tx, &id).await?;
        SettingsStore::clear_default_converter(&mut tx).await?;
        let target = Target::of("default_converter", &id, &name);
        self.record(&mut tx, person, "clear_default_converter", target, vec![], now()).await?;
        tx.commit().await
    }

    pub async fn default_converter(&self) -> Result<Option<Converter>> {
        let mut tx = self.store.begin().await?;
        let out = match SettingsStore::default_converter_id(&mut tx).await? {
            Some(id) => SettingsStore::converter_by_id(&mut tx, &id).await?.map(|r| r.converter),
            None => None,
        };
        tx.commit().await?;
        Ok(out)
    }

    /// Makes a converter the converter of a project (FR-NEW-009). The project
    /// is checked before the converter (E2E-123); a project in its grace period
    /// is found, one removed for good is not.
    pub async fn set_project_converter(
        &self,
        person: &str,
        project_id: &str,
        converter_id: &str,
    ) -> Result<Converter> {
        let mut attempt = 0;
        loop {
            match self.try_set_project_converter(person, project_id, converter_id).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_set_project_converter(
        &self,
        person: &str,
        project_id: &str,
        converter_id: &str,
    ) -> Result<Converter> {
        let mut tx = self.store.begin().await?;
        let Some(current) = SettingsStore::project_converter_id(&mut tx, project_id).await? else {
            return Err(ToolError::project_not_found(project_id));
        };
        let Some(row) = SettingsStore::converter_by_id(&mut tx, converter_id).await? else {
            return Err(converter_not_found(converter_id));
        };
        let c = row.converter;
        if current.as_deref() == Some(converter_id) {
            return Ok(c);
        }
        let at = now();
        SettingsStore::set_project_converter(&mut tx, project_id, Some(converter_id)).await?;
        let target = Target::of("project_converter", project_id, &c.name);
        self.record(&mut tx, person, "set_project_converter", target, vec![], at).await?;
        tx.commit().await?;
        Ok(c)
    }

    /// Removes the converter of a project; none set is not an error and
    /// writes no record.
    pub async fn clear_project_converter(&self, person: &str, project_id: &str) -> Result<()> {
        let mut attempt = 0;
        loop {
            match self.try_clear_project_converter(person, project_id).await {
                Err(e) if Self::retry(attempt, &e) => attempt += 1,
                out => return out,
            }
        }
    }

    async fn try_clear_project_converter(&self, person: &str, project_id: &str) -> Result<()> {
        let mut tx = self.store.begin().await?;
        let Some(current) = SettingsStore::project_converter_id(&mut tx, project_id).await? else {
            return Err(ToolError::project_not_found(project_id));
        };
        let Some(id) = current else {
            return Ok(());
        };
        let name = name_of(&mut tx, &id).await?;
        SettingsStore::set_project_converter(&mut tx, project_id, None).await?;
        let target = Target::of("project_converter", project_id, &name);
        self.record(&mut tx, person, "clear_project_converter", target, vec![], now()).await?;
        tx.commit().await
    }

    pub async fn project_converter(&self, project_id: &str) -> Result<Option<Converter>> {
        let mut tx = self.store.begin().await?;
        let Some(current) = SettingsStore::project_converter_id(&mut tx, project_id).await? else {
            return Err(ToolError::project_not_found(project_id));
        };
        let out = match current {
            Some(id) => SettingsStore::converter_by_id(&mut tx, &id).await?.map(|r| r.converter),
            None => None,
        };
        tx.commit().await?;
        Ok(out)
    }

    /// One keyset page of 100 by caseless name. A marker given as anything but
    /// a text is not valid; an empty one reads the first page.
    pub async fn list_converters(&self, marker: &Option<Value>) -> Result<Page<Converter>> {
        let after = match rules::given(marker) {
            None => None,
            Some(Value::String(m)) if m.is_empty() => None,
            Some(Value::String(m)) => Some(decode_marker(m)?.0),
            Some(_) => return Err(ToolError::invalid_argument("marker is not valid")),
        };
        let mut rows = self.store.list_converters(after.as_deref(), PAGE + 1).await?;
        let more = rows.len() > PAGE;
        rows.truncate(PAGE);
        let marker = more.then(|| {
            let last = &rows.last().expect("a full page").converter;
            encode_marker(&name_key(&last.name), &last.converter_id)
        });
        Ok(Page { items: rows.into_iter().map(|r| r.converter).collect(), marker })
    }
}

/// The converter a conversion in `project_id` uses (SPEC-0106 DEC-006): the
/// project converter, else the platform default; `None` when neither is set or
/// the admin store holds no settings tables (a test double). The secret is
/// decrypted here, `None` when it cannot be read.
pub(crate) async fn resolved_converter(
    admin: &dyn crate::storage::traits::AdminBackend,
    project_id: &str,
) -> Result<Option<(Converter, Option<crate::settings::SecretText>)>> {
    let Some(store) = admin.settings() else { return Ok(None) };
    let mut tx = store.begin().await?;
    let id = match SettingsStore::project_converter_id(&mut tx, project_id).await? {
        Some(Some(id)) => Some(id),
        _ => SettingsStore::default_converter_id(&mut tx).await?,
    };
    let row = match id {
        Some(id) => SettingsStore::converter_by_id(&mut tx, &id).await?,
        None => None,
    };
    tx.commit().await?;
    Ok(row.map(|r| {
        let secret = r.secret_enc.as_deref().and_then(|b| super::secrets::decrypt(store.key(), b));
        (r.converter, secret)
    }))
}
