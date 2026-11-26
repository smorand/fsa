//! The SQL of the `server_setting` rows (SPEC-0114 DEC-001), on
//! [`SettingsStore`]'s handle so SQLite and PostgreSQL share every query; the
//! rules live in `server_settings.rs`.

use super::store::SettingsStore;
use crate::errors::Result;
use crate::storage::rel::{Query, RelationalTx, RowValues, SqlValue};

const COLS: &str = "section, value, secret_enc, updated_at, updated_by";

/// A section as stored: the JSON without secrets and the encrypted secrets.
#[derive(Clone)]
pub(crate) struct ServerSettingRow {
    pub section: String,
    pub value: String,
    pub secret_enc: Option<Vec<u8>>,
    pub updated_at: String,
    pub updated_by: String,
}

impl SettingsStore {
    fn read_server_setting(r: &RowValues) -> Result<ServerSettingRow> {
        let secret_enc = match r.value(2)? {
            SqlValue::Null => None,
            _ => Some(r.blob(2)?),
        };
        Ok(ServerSettingRow {
            section: r.text(0)?,
            value: r.text(1)?,
            secret_enc,
            updated_at: r.text(3)?,
            updated_by: r.text(4)?,
        })
    }

    pub(crate) async fn server_setting_in(
        tx: &mut Box<dyn RelationalTx>,
        section: &str,
    ) -> Result<Option<ServerSettingRow>> {
        let q =
            Query::new(format!("SELECT {COLS} FROM server_setting WHERE section=?1")).bind(section);
        tx.query_opt(&q).await?.as_ref().map(Self::read_server_setting).transpose()
    }

    pub(crate) async fn all_server_settings(&self) -> Result<Vec<ServerSettingRow>> {
        let q = Query::new(format!("SELECT {COLS} FROM server_setting ORDER BY section"));
        self.db.query(&q).await?.iter().map(Self::read_server_setting).collect()
    }

    /// Insert when `exists` is false, else rewrite; the caller read the row in
    /// the same transaction, so a concurrent insert surfaces as a key conflict
    /// the registry's one retry resolves.
    pub(crate) async fn put_server_setting(
        tx: &mut Box<dyn RelationalTx>,
        row: &ServerSettingRow,
        exists: bool,
    ) -> Result<()> {
        let q = if exists {
            Query::new(
                "UPDATE server_setting SET value=?2, secret_enc=?3, updated_at=?4, updated_by=?5 \
                 WHERE section=?1",
            )
        } else {
            Query::new(format!("INSERT INTO server_setting ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5)"))
        };
        tx.execute(
            &q.bind(&row.section)
                .bind(&row.value)
                .bind(row.secret_enc.clone())
                .bind(&row.updated_at)
                .bind(&row.updated_by),
        )
        .await?;
        Ok(())
    }
}
