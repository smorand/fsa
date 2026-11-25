//! The SQL of the `converter` rows (SPEC-0106 DEC-003), on [`SettingsStore`]'s
//! handle so SQLite and PostgreSQL share every query; the rules live in
//! `converters.rs`.

use super::converter::Converter;
use super::store::{SettingsStore, name_key};
use crate::errors::{Result, ToolError};
use crate::storage::rel::{Query, RelationalTx, RowValues, SqlValue};

const CONVERTER_COLS: &str = "name_key, converter_id, name, form, command, url, auth_header, \
     secret_enc, file_field, response_field, extensions, max_input_bytes, timeout_seconds, \
     created_at, updated_at";

/// A converter as stored: the answer plus its encrypted secret.
#[derive(Clone)]
pub(crate) struct ConverterRow {
    pub converter: Converter,
    pub secret_enc: Option<Vec<u8>>,
}

fn list_of(raw: &str) -> Result<Vec<String>> {
    serde_json::from_str(raw)
        .map_err(|_| ToolError::internal("a converter holds an unreadable list"))
}

fn json_list(list: &[String]) -> String {
    serde_json::to_string(list).expect("a list of texts serializes")
}

impl SettingsStore {
    fn read_converter(r: &RowValues) -> Result<ConverterRow> {
        let secret_enc = match r.value(7)? {
            SqlValue::Null => None,
            _ => Some(r.blob(7)?),
        };
        Ok(ConverterRow {
            converter: Converter {
                converter_id: r.text(1)?,
                name: r.text(2)?,
                form: r.text(3)?,
                command: r.opt_text(4)?.as_deref().map(list_of).transpose()?,
                url: r.opt_text(5)?,
                auth_header: r.opt_text(6)?,
                has_secret: secret_enc.is_some(),
                file_field: r.opt_text(8)?,
                response_field: r.opt_text(9)?,
                extensions: list_of(&r.text(10)?)?,
                max_input_bytes: r.i64(11)?,
                timeout_seconds: r.i64(12)?,
                created_at: r.text(13)?,
                updated_at: r.text(14)?,
            },
            secret_enc,
        })
    }

    /// The id of the converter holding `key`, if any.
    pub(crate) async fn converter_id_of_key(
        tx: &mut Box<dyn RelationalTx>,
        key: &str,
    ) -> Result<Option<String>> {
        let q = Query::new("SELECT converter_id FROM converter WHERE name_key=?1").bind(key);
        tx.query_opt(&q).await?.map(|r| r.text(0)).transpose()
    }

    pub(crate) async fn converter_by_id(
        tx: &mut Box<dyn RelationalTx>,
        converter_id: &str,
    ) -> Result<Option<ConverterRow>> {
        let q = Query::new(format!("SELECT {CONVERTER_COLS} FROM converter WHERE converter_id=?1"))
            .bind(converter_id);
        tx.query_opt(&q).await?.as_ref().map(Self::read_converter).transpose()
    }

    /// Rewrites every field an update may change; the id, the form and the
    /// creation time stay.
    pub(crate) async fn update_converter(
        tx: &mut Box<dyn RelationalTx>,
        row: &ConverterRow,
    ) -> Result<()> {
        let c = &row.converter;
        tx.execute(
            &Query::new(
                "UPDATE converter SET name_key=?1, name=?2, command=?3, url=?4, auth_header=?5, \
                 secret_enc=?6, file_field=?7, response_field=?8, extensions=?9, \
                 max_input_bytes=?10, timeout_seconds=?11, updated_at=?12 WHERE converter_id=?13",
            )
            .bind(name_key(&c.name))
            .bind(&c.name)
            .bind(c.command.as_deref().map(json_list))
            .bind(c.url.clone())
            .bind(c.auth_header.clone())
            .bind(row.secret_enc.clone())
            .bind(c.file_field.clone())
            .bind(c.response_field.clone())
            .bind(json_list(&c.extensions))
            .bind(c.max_input_bytes)
            .bind(c.timeout_seconds)
            .bind(&c.updated_at)
            .bind(&c.converter_id),
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn insert_converter(
        tx: &mut Box<dyn RelationalTx>,
        row: &ConverterRow,
    ) -> Result<()> {
        let c = &row.converter;
        tx.execute(
            &Query::new(format!(
                "INSERT INTO converter ({CONVERTER_COLS}) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"
            ))
            .bind(name_key(&c.name))
            .bind(&c.converter_id)
            .bind(&c.name)
            .bind(&c.form)
            .bind(c.command.as_deref().map(json_list))
            .bind(c.url.clone())
            .bind(c.auth_header.clone())
            .bind(row.secret_enc.clone())
            .bind(c.file_field.clone())
            .bind(c.response_field.clone())
            .bind(json_list(&c.extensions))
            .bind(c.max_input_bytes)
            .bind(c.timeout_seconds)
            .bind(&c.created_at)
            .bind(&c.updated_at),
        )
        .await?;
        Ok(())
    }

    /// Removes the row and, with it, the stored secret.
    pub(crate) async fn delete_converter(
        tx: &mut Box<dyn RelationalTx>,
        converter_id: &str,
    ) -> Result<()> {
        tx.execute(&Query::new("DELETE FROM converter WHERE converter_id=?1").bind(converter_id))
            .await?;
        Ok(())
    }

    /// Projects holding `converter_id`, a project in its grace period included
    /// (its row stays, DEC-014). One indexed or scanned count, no row read.
    pub(crate) async fn count_converter_projects(
        tx: &mut Box<dyn RelationalTx>,
        converter_id: &str,
    ) -> Result<i64> {
        let q = Query::new("SELECT COUNT(*) FROM project WHERE converter_id=?1").bind(converter_id);
        tx.query_opt(&q).await?.map(|r| r.i64(0)).transpose().map(|n| n.unwrap_or(0))
    }

    /// The id of the platform default converter, if one is set.
    pub(crate) async fn default_converter_id(
        tx: &mut Box<dyn RelationalTx>,
    ) -> Result<Option<String>> {
        let q = Query::new("SELECT converter_id FROM converter_default WHERE slot='default'");
        tx.query_opt(&q).await?.map(|r| r.text(0)).transpose()
    }

    /// Makes `converter_id` the platform default, replacing the previous one in
    /// the caller's transaction.
    pub(crate) async fn set_default_converter(
        tx: &mut Box<dyn RelationalTx>,
        converter_id: &str,
    ) -> Result<()> {
        Self::clear_default_converter(tx).await?;
        tx.execute(
            &Query::new(
                "INSERT INTO converter_default (slot, converter_id) VALUES ('default', ?1)",
            )
            .bind(converter_id),
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn clear_default_converter(tx: &mut Box<dyn RelationalTx>) -> Result<()> {
        tx.execute(&Query::new("DELETE FROM converter_default WHERE slot='default'")).await?;
        Ok(())
    }

    /// `None` when the project row does not exist (removed for good); otherwise
    /// its converter id, null when it has none. A soft removed project keeps
    /// its row, so it is found.
    pub(crate) async fn project_converter_id(
        tx: &mut Box<dyn RelationalTx>,
        project_id: &str,
    ) -> Result<Option<Option<String>>> {
        let q = Query::new("SELECT converter_id FROM project WHERE id=?1").bind(project_id);
        tx.query_opt(&q).await?.map(|r| r.opt_text(0)).transpose()
    }

    pub(crate) async fn set_project_converter(
        tx: &mut Box<dyn RelationalTx>,
        project_id: &str,
        converter_id: Option<&str>,
    ) -> Result<()> {
        tx.execute(
            &Query::new("UPDATE project SET converter_id=?1 WHERE id=?2")
                .bind(converter_id.map(str::to_string))
                .bind(project_id),
        )
        .await?;
        Ok(())
    }

    /// Converters ordered by `name_key`, strictly after `after` when given; the
    /// names are unique without regard to case, so the key alone orders them and
    /// the primary key serves the keyset.
    pub(crate) async fn list_converters(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ConverterRow>> {
        let q = match after {
            Some(k) => Query::new(format!(
                "SELECT {CONVERTER_COLS} FROM converter WHERE name_key>?1 \
                 ORDER BY name_key LIMIT {limit}"
            ))
            .bind(k),
            None => Query::new(format!(
                "SELECT {CONVERTER_COLS} FROM converter ORDER BY name_key LIMIT {limit}"
            )),
        };
        self.db.query(&q).await?.iter().map(Self::read_converter).collect()
    }
}
