//! `SettingsStore`: the `provider` and `setting_change` rows of the admin store
//! (SPEC-0102 DEC-002). Speaks only [`RelationalDb`], so SQLite and PostgreSQL
//! share every query; the rules live in [`super::Registry`].

use super::model::{Change, Model, Provider};
use crate::errors::{Result, ToolError};
use crate::storage::rel::{Query, RelationalDb, RelationalTx, RowValues, SqlValue};
use std::sync::Arc;

const PROVIDER_COLS: &str =
    "name_key, provider_id, name, endpoint, auth_scheme, secret_enc, created_at, updated_at";
const MODEL_COLS: &str = "model_id, provider_id, name, kind, dimension, max_input_tokens, \
     created_at, updated_at";
const CHANGE_COLS: &str =
    "change_id, at, person, action, target_kind, target_id, target_name, fields";

/// A provider as stored: the answer plus its encrypted secret.
#[derive(Clone)]
pub(crate) struct ProviderRow {
    pub provider: Provider,
    pub secret_enc: Option<Vec<u8>>,
}

/// The caseless key names are compared on: trimmed, lower case (DEC-002).
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

#[derive(Clone)]
pub struct SettingsStore {
    pub(super) db: Arc<dyn RelationalDb>,
    /// The master key handed over at boot (DEC-003); never stored.
    pub(super) key: Option<[u8; 32]>,
}

impl SettingsStore {
    /// No IO: the tables are created with the admin schema at `connect()`.
    pub fn new(db: Arc<dyn RelationalDb>, key: Option<[u8; 32]>) -> Self {
        Self { db, key }
    }

    pub(crate) fn key(&self) -> Option<[u8; 32]> {
        self.key
    }

    /// Whether a provider of that id exists, outside any transaction.
    pub(crate) async fn provider_exists(&self, provider_id: &str) -> Result<Option<()>> {
        let q =
            Query::new("SELECT provider_id FROM provider WHERE provider_id=?1").bind(provider_id);
        Ok(self.db.query_opt(&q).await?.map(|_| ()))
    }

    pub(crate) async fn begin(&self) -> Result<Box<dyn RelationalTx>> {
        self.db.begin().await
    }

    fn read_provider(r: &RowValues) -> Result<ProviderRow> {
        let secret_enc = match r.value(5)? {
            SqlValue::Null => None,
            _ => Some(r.blob(5)?),
        };
        Ok(ProviderRow {
            provider: Provider {
                provider_id: r.text(1)?,
                name: r.text(2)?,
                endpoint: r.text(3)?,
                auth_scheme: r.text(4)?,
                has_secret: secret_enc.is_some(),
                created_at: r.text(6)?,
                updated_at: r.text(7)?,
            },
            secret_enc,
        })
    }

    fn read_change(r: &RowValues) -> Result<Change> {
        let fields: Vec<String> = serde_json::from_str(&r.text(7)?)
            .map_err(|_| ToolError::internal("a setting change holds unreadable fields"))?;
        Ok(Change {
            change_id: r.text(0)?,
            at: r.text(1)?,
            person: r.text(2)?,
            action: r.text(3)?,
            target_kind: r.text(4)?,
            target_id: r.text(5)?,
            target_name: r.text(6)?,
            fields,
        })
    }

    fn read_model(r: &RowValues) -> Result<Model> {
        Ok(Model {
            model_id: r.text(0)?,
            provider_id: r.text(1)?,
            name: r.text(2)?,
            kind: r.text(3)?,
            dimension: r.opt_i64(4)?,
            max_input_tokens: r.opt_i64(5)?,
            is_default: false,
            created_at: r.text(6)?,
            updated_at: r.text(7)?,
        })
    }

    pub(crate) async fn model_by_id(
        tx: &mut Box<dyn RelationalTx>,
        model_id: &str,
    ) -> Result<Option<Model>> {
        let q =
            Query::new(format!("SELECT {MODEL_COLS} FROM model WHERE model_id=?1")).bind(model_id);
        let Some(mut model) = tx.query_opt(&q).await?.as_ref().map(Self::read_model).transpose()?
        else {
            return Ok(None);
        };
        let q = Query::new("SELECT kind FROM default_model WHERE model_id=?1").bind(model_id);
        model.is_default = tx.query_opt(&q).await?.is_some();
        Ok(Some(model))
    }

    /// The id of the default model of `kind`, if one is set.
    pub(crate) async fn default_id(
        tx: &mut Box<dyn RelationalTx>,
        kind: &str,
    ) -> Result<Option<String>> {
        let q = Query::new("SELECT model_id FROM default_model WHERE kind=?1").bind(kind);
        tx.query_opt(&q).await?.map(|r| r.text(0)).transpose()
    }

    /// Makes `model_id` the default of `kind`, replacing the previous one; the
    /// delete and the insert share the caller's transaction.
    pub(crate) async fn set_default(
        tx: &mut Box<dyn RelationalTx>,
        kind: &str,
        model_id: &str,
        at: &str,
    ) -> Result<()> {
        tx.execute(&Query::new("DELETE FROM default_model WHERE kind=?1").bind(kind)).await?;
        tx.execute(
            &Query::new("INSERT INTO default_model (kind, model_id, set_at) VALUES (?1, ?2, ?3)")
                .bind(kind)
                .bind(model_id)
                .bind(at),
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn clear_default(tx: &mut Box<dyn RelationalTx>, kind: &str) -> Result<()> {
        tx.execute(&Query::new("DELETE FROM default_model WHERE kind=?1").bind(kind)).await?;
        Ok(())
    }

    /// The default model of `kind` with `is_default` true, outside any
    /// transaction (the primary key on `kind` makes it one row).
    pub(crate) async fn default_model(&self, kind: &str) -> Result<Option<Model>> {
        let cols = MODEL_COLS
            .split(", ")
            .map(|c| format!("m.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        let q = Query::new(format!(
            "SELECT {cols} FROM default_model d JOIN model m ON m.model_id=d.model_id \
             WHERE d.kind=?1"
        ))
        .bind(kind);
        let model = self.db.query_opt(&q).await?.as_ref().map(Self::read_model).transpose()?;
        Ok(model.map(|m| Model { is_default: true, ..m }))
    }

    /// The default model of `kind` with its provider, in one query: search asks
    /// it on every index or query call (DEC-007), so it stays one round trip.
    pub(crate) async fn default_with_provider(
        &self,
        kind: &str,
    ) -> Result<Option<(Model, ProviderRow)>> {
        let m = MODEL_COLS.split(", ").map(|c| format!("m.{}", c.trim()));
        let p = PROVIDER_COLS.split(", ").map(|c| format!("p.{}", c.trim()));
        let cols = m.chain(p).collect::<Vec<_>>().join(", ");
        let q = Query::new(format!(
            "SELECT {cols} FROM default_model d JOIN model m ON m.model_id=d.model_id \
             JOIN provider p ON p.provider_id=m.provider_id WHERE d.kind=?1"
        ))
        .bind(kind);
        let Some(r) = self.db.query_opt(&q).await? else { return Ok(None) };
        let model = Model { is_default: true, ..Self::read_model(&r)? };
        let n = MODEL_COLS.split(", ").count();
        let tail: Vec<SqlValue> =
            (n..r.len()).map(|i| r.value(i).cloned()).collect::<Result<_>>()?;
        let provider = Self::read_provider(&RowValues::new(r.columns()[n..].into(), tail))?;
        Ok(Some((model, provider)))
    }

    /// The model `model_id` with its provider, in one query (a project's profile
    /// model, SPEC-0108 FR-NEW-001); `None` when it is gone.
    pub(crate) async fn model_with_provider(
        &self,
        model_id: &str,
    ) -> Result<Option<(Model, ProviderRow)>> {
        let m = MODEL_COLS.split(", ").map(|c| format!("m.{}", c.trim()));
        let p = PROVIDER_COLS.split(", ").map(|c| format!("p.{}", c.trim()));
        let cols = m.chain(p).collect::<Vec<_>>().join(", ");
        let q = Query::new(format!(
            "SELECT {cols} FROM model m JOIN provider p ON p.provider_id=m.provider_id \
             WHERE m.model_id=?1"
        ))
        .bind(model_id);
        let Some(r) = self.db.query_opt(&q).await? else { return Ok(None) };
        let model = Self::read_model(&r)?;
        let n = MODEL_COLS.split(", ").count();
        let tail: Vec<SqlValue> =
            (n..r.len()).map(|i| r.value(i).cloned()).collect::<Result<_>>()?;
        let provider = Self::read_provider(&RowValues::new(r.columns()[n..].into(), tail))?;
        Ok(Some((model, provider)))
    }

    /// The id of the model of `provider_id` holding `key`, if any.
    pub(crate) async fn model_id_of_key(
        tx: &mut Box<dyn RelationalTx>,
        provider_id: &str,
        key: &str,
    ) -> Result<Option<String>> {
        let q = Query::new("SELECT model_id FROM model WHERE provider_id=?1 AND name_key=?2")
            .bind(provider_id)
            .bind(key);
        tx.query_opt(&q).await?.map(|r| r.text(0)).transpose()
    }

    pub(crate) async fn count_models(
        tx: &mut Box<dyn RelationalTx>,
        provider_id: &str,
    ) -> Result<i64> {
        let q = Query::new("SELECT COUNT(*) FROM model WHERE provider_id=?1").bind(provider_id);
        match tx.query_opt(&q).await? {
            Some(r) => r.i64(0),
            None => Ok(0),
        }
    }

    pub(crate) async fn insert_model(tx: &mut Box<dyn RelationalTx>, m: &Model) -> Result<()> {
        tx.execute(
            &Query::new(
                "INSERT INTO model (provider_id, name_key, model_id, name, kind, dimension, \
                 max_input_tokens, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .bind(&m.provider_id)
            .bind(name_key(&m.name))
            .bind(&m.model_id)
            .bind(&m.name)
            .bind(&m.kind)
            .bind(m.dimension)
            .bind(m.max_input_tokens)
            .bind(&m.created_at)
            .bind(&m.updated_at),
        )
        .await?;
        Ok(())
    }

    /// Rewrites the fields a model may change; the key moves with the name.
    pub(crate) async fn update_model(tx: &mut Box<dyn RelationalTx>, m: &Model) -> Result<()> {
        tx.execute(
            &Query::new(
                "UPDATE model SET name_key=?1, name=?2, dimension=?3, max_input_tokens=?4, \
                 updated_at=?5 WHERE model_id=?6",
            )
            .bind(name_key(&m.name))
            .bind(&m.name)
            .bind(m.dimension)
            .bind(m.max_input_tokens)
            .bind(&m.updated_at)
            .bind(&m.model_id),
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn delete_model(tx: &mut Box<dyn RelationalTx>, model_id: &str) -> Result<()> {
        tx.execute(&Query::new("DELETE FROM model WHERE model_id=?1").bind(model_id)).await?;
        Ok(())
    }

    pub(crate) async fn provider_by_id(
        tx: &mut Box<dyn RelationalTx>,
        provider_id: &str,
    ) -> Result<Option<ProviderRow>> {
        let q = Query::new(format!("SELECT {PROVIDER_COLS} FROM provider WHERE provider_id=?1"))
            .bind(provider_id);
        tx.query_opt(&q).await?.as_ref().map(Self::read_provider).transpose()
    }

    /// The id of the provider holding `key`, if any.
    pub(crate) async fn provider_id_of_key(
        tx: &mut Box<dyn RelationalTx>,
        key: &str,
    ) -> Result<Option<String>> {
        let q = Query::new("SELECT provider_id FROM provider WHERE name_key=?1").bind(key);
        tx.query_opt(&q).await?.map(|r| r.text(0)).transpose()
    }

    pub(crate) async fn insert_provider(
        tx: &mut Box<dyn RelationalTx>,
        row: &ProviderRow,
    ) -> Result<()> {
        let p = &row.provider;
        tx.execute(
            &Query::new(format!(
                "INSERT INTO provider ({PROVIDER_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ))
            .bind(name_key(&p.name))
            .bind(&p.provider_id)
            .bind(&p.name)
            .bind(&p.endpoint)
            .bind(&p.auth_scheme)
            .bind(row.secret_enc.clone())
            .bind(&p.created_at)
            .bind(&p.updated_at),
        )
        .await?;
        Ok(())
    }

    /// Rewrites every column but the id and the creation time; the key moves
    /// with the name, which a primary key on `name_key` allows.
    pub(crate) async fn update_provider(
        tx: &mut Box<dyn RelationalTx>,
        row: &ProviderRow,
    ) -> Result<()> {
        let p = &row.provider;
        tx.execute(
            &Query::new(
                "UPDATE provider SET name_key=?1, name=?2, endpoint=?3, auth_scheme=?4, \
                 secret_enc=?5, updated_at=?6 WHERE provider_id=?7",
            )
            .bind(name_key(&p.name))
            .bind(&p.name)
            .bind(&p.endpoint)
            .bind(&p.auth_scheme)
            .bind(row.secret_enc.clone())
            .bind(&p.updated_at)
            .bind(&p.provider_id),
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn delete_provider(
        tx: &mut Box<dyn RelationalTx>,
        provider_id: &str,
    ) -> Result<()> {
        tx.execute(&Query::new("DELETE FROM provider WHERE provider_id=?1").bind(provider_id))
            .await?;
        Ok(())
    }

    /// Inserts `change` under the next id, read inside the same transaction
    /// so the ids follow the order of the commits (DEC-004, DEC-009).
    pub(crate) async fn record(tx: &mut Box<dyn RelationalTx>, mut change: Change) -> Result<()> {
        let last = tx.query_opt(&Query::new("SELECT MAX(change_id) FROM setting_change")).await?;
        let last = match last {
            Some(r) => r.opt_text(0)?,
            None => None,
        };
        let n = last
            .as_deref()
            .and_then(|id| id.strip_prefix("chg-"))
            .and_then(|d| d.parse::<u64>().ok())
            .unwrap_or(0);
        change.change_id = format!("chg-{:012}", n + 1);
        let fields = serde_json::to_string(&change.fields).expect("a list of names serializes");
        tx.execute(
            &Query::new(format!(
                "INSERT INTO setting_change ({CHANGE_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ))
            .bind(&change.change_id)
            .bind(&change.at)
            .bind(&change.person)
            .bind(&change.action)
            .bind(&change.target_kind)
            .bind(&change.target_id)
            .bind(&change.target_name)
            .bind(fields),
        )
        .await?;
        Ok(())
    }

    /// Providers ordered by `name_key`, strictly after `after` when given
    /// (DEC-010); names are unique without regard to case, so the key alone
    /// orders them.
    pub(crate) async fn list_providers(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ProviderRow>> {
        let q = match after {
            Some(k) => Query::new(format!(
                "SELECT {PROVIDER_COLS} FROM provider WHERE name_key>?1 ORDER BY name_key LIMIT {limit}"
            ))
            .bind(k),
            None => Query::new(format!(
                "SELECT {PROVIDER_COLS} FROM provider ORDER BY name_key LIMIT {limit}"
            )),
        };
        self.db.query(&q).await?.iter().map(Self::read_provider).collect()
    }

    /// Models ordered by provider name key then model name key, strictly after
    /// `after` (both keys) when given, optionally of one provider or one kind
    /// (DEC-010). The join reads the provider key, so a renamed provider moves
    /// its models with it. The keyset reads `idx_model_*` and the provider key.
    pub(crate) async fn list_models(
        &self,
        provider_id: &str,
        kind: &str,
        after: Option<(&str, &str)>,
        limit: usize,
    ) -> Result<Vec<(Model, String)>> {
        let mut conds: Vec<String> = Vec::new();
        let mut binds: Vec<SqlValue> = Vec::new();
        let next = |v: SqlValue, binds: &mut Vec<SqlValue>| {
            binds.push(v);
            binds.len()
        };
        if !provider_id.is_empty() {
            let n = next(provider_id.into(), &mut binds);
            conds.push(format!("m.provider_id=?{n}"));
        }
        if !kind.is_empty() {
            let n = next(kind.into(), &mut binds);
            conds.push(format!("m.kind=?{n}"));
        }
        if let Some((pk, mk)) = after {
            let a = next(pk.into(), &mut binds);
            let b = next(mk.into(), &mut binds);
            conds.push(format!("(p.name_key>?{a} OR (p.name_key=?{a} AND m.name_key>?{b}))"));
        }
        let wher =
            if conds.is_empty() { String::new() } else { format!("WHERE {}", conds.join(" AND ")) };
        let cols = MODEL_COLS
            .split(", ")
            .map(|c| format!("m.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut q = Query::new(format!(
            "SELECT {cols}, p.name_key, d.kind FROM model m \
             JOIN provider p ON p.provider_id=m.provider_id \
             LEFT JOIN default_model d ON d.model_id=m.model_id \
             {wher} ORDER BY p.name_key, m.name_key LIMIT {limit}"
        ));
        for v in binds {
            q = q.bind(v);
        }
        self.db
            .query(&q)
            .await?
            .iter()
            .map(|r| {
                let model = Self::read_model(r)?;
                let is_default = !matches!(r.value(9)?, SqlValue::Null);
                Ok((Model { is_default, ..model }, r.text(8)?))
            })
            .collect()
    }

    /// Changes newest first by `change_id`, strictly before `before` when given.
    pub(crate) async fn list_changes(
        &self,
        before: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Change>> {
        let q = match before {
            Some(id) => Query::new(format!(
                "SELECT {CHANGE_COLS} FROM setting_change WHERE change_id<?1 \
                 ORDER BY change_id DESC LIMIT {limit}"
            ))
            .bind(id),
            None => Query::new(format!(
                "SELECT {CHANGE_COLS} FROM setting_change ORDER BY change_id DESC LIMIT {limit}"
            )),
        };
        self.db.query(&q).await?.iter().map(Self::read_change).collect()
    }

    /// The newest `limit` changes whose `target_kind` is one of `kinds`, newest
    /// first by `change_id`. One filtered read, so a screen showing a single
    /// registry is not starved by the changes of another (DEC-013).
    pub(crate) async fn list_changes_of_kinds(
        &self,
        kinds: &[&str],
        limit: usize,
    ) -> Result<Vec<Change>> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        let marks = (1..=kinds.len()).map(|n| format!("?{n}")).collect::<Vec<_>>().join(", ");
        let mut q = Query::new(format!(
            "SELECT {CHANGE_COLS} FROM setting_change WHERE target_kind IN ({marks}) \
             ORDER BY change_id DESC LIMIT {limit}"
        ));
        for k in kinds {
            q = q.bind(*k);
        }
        self.db.query(&q).await?.iter().map(Self::read_change).collect()
    }
}
