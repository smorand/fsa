//! The SQL of the per project indexing profile (SPEC-0107 DEC-001): five
//! nullable columns of the `project` row, on [`SettingsStore`]'s handle so
//! SQLite and PostgreSQL share every query. The rules live in `profile.rs`.

use super::store::SettingsStore;
use crate::errors::Result;
use crate::storage::rel::{Query, RelationalTx};

/// The profile of a project as stored.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfileRow {
    pub chunk_strategy: Option<String>,
    /// The requested settings as a JSON object, `None` when there are none.
    pub chunk_settings: Option<String>,
    pub text_model_id: Option<String>,
    pub image_model_id: Option<String>,
    /// RFC 3339 UTC seconds of the last change of a model; `None` when the
    /// vectors are not known to be out of date.
    pub vectors_stale_at: Option<String>,
}

impl SettingsStore {
    /// `None` when the project row does not exist; a project in its grace
    /// period keeps its row, so it is found.
    pub(crate) async fn profile_row(
        tx: &mut Box<dyn RelationalTx>,
        project_id: &str,
    ) -> Result<Option<ProfileRow>> {
        let q = Query::new(
            "SELECT chunk_strategy, chunk_settings, text_model_id, image_model_id, \
             vectors_stale_at FROM project WHERE id=?1",
        )
        .bind(project_id);
        let Some(r) = tx.query_opt(&q).await? else {
            return Ok(None);
        };
        Ok(Some(ProfileRow {
            chunk_strategy: r.opt_text(0)?,
            chunk_settings: r.opt_text(1)?,
            text_model_id: r.opt_text(2)?,
            image_model_id: r.opt_text(3)?,
            vectors_stale_at: r.opt_text(4)?,
        }))
    }

    /// Rewrites the columns named in `fields` only (the names of the record), so two
    /// concurrent sets of different fields never undo each other on an engine that
    /// reads without locking; a set is still one statement, all or nothing.
    pub(crate) async fn write_profile_row(
        tx: &mut Box<dyn RelationalTx>,
        project_id: &str,
        row: &ProfileRow,
        fields: &[String],
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut values: Vec<Option<String>> = Vec::new();
        for f in fields {
            let value = match f.as_str() {
                "chunk_strategy" => row.chunk_strategy.clone(),
                "chunk_settings" => row.chunk_settings.clone(),
                "text_model_id" => row.text_model_id.clone(),
                "image_model_id" => row.image_model_id.clone(),
                _ => continue,
            };
            values.push(value);
            sets.push(format!("{f}=?{}", values.len()));
        }
        // The marker moves with a model, nothing else rewrites it.
        if fields.iter().any(|f| f == "text_model_id" || f == "image_model_id") {
            values.push(row.vectors_stale_at.clone());
            sets.push(format!("vectors_stale_at=?{}", values.len()));
        }
        if sets.is_empty() {
            return Ok(());
        }
        let sql = format!("UPDATE project SET {} WHERE id=?{}", sets.join(", "), values.len() + 1);
        let mut q = Query::new(sql);
        for v in values {
            q = q.bind(v);
        }
        tx.execute(&q.bind(project_id)).await?;
        Ok(())
    }

    /// Projects whose profile names `model_id` as text or image model, a
    /// project in its grace period included (it keeps its row).
    pub(crate) async fn count_model_projects(
        tx: &mut Box<dyn RelationalTx>,
        model_id: &str,
    ) -> Result<i64> {
        let q =
            Query::new("SELECT COUNT(*) FROM project WHERE text_model_id=?1 OR image_model_id=?1")
                .bind(model_id);
        tx.query_opt(&q).await?.map(|r| r.i64(0)).transpose().map(|n| n.unwrap_or(0))
    }

    pub(crate) async fn provider_name(
        tx: &mut Box<dyn RelationalTx>,
        provider_id: &str,
    ) -> Result<Option<String>> {
        let q = Query::new("SELECT name FROM provider WHERE provider_id=?1").bind(provider_id);
        tx.query_opt(&q).await?.map(|r| r.text(0)).transpose()
    }

    /// Every project id with its profile, by id; the screen shows them.
    pub(crate) async fn list_profile_rows(&self) -> Result<Vec<(String, ProfileRow)>> {
        let q = Query::new(
            "SELECT id, chunk_strategy, chunk_settings, text_model_id, image_model_id, \
             vectors_stale_at FROM project ORDER BY id",
        );
        self.db
            .query(&q)
            .await?
            .iter()
            .map(|r| {
                Ok((
                    r.text(0)?,
                    ProfileRow {
                        chunk_strategy: r.opt_text(1)?,
                        chunk_settings: r.opt_text(2)?,
                        text_model_id: r.opt_text(3)?,
                        image_model_id: r.opt_text(4)?,
                        vectors_stale_at: r.opt_text(5)?,
                    },
                ))
            })
            .collect()
    }
}
