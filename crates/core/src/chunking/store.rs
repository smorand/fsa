//! `ChunkStore`: the `chunk_sets` and `chunks` rows of one volume's meta store
//! (DEC-004). Speaks only [`RelationalDb`], so SQLite and PostgreSQL share
//! every query; every statement is scoped by `volume_id`.

use super::model::{Chunk, Set};
use crate::errors::{Result, ToolError};
use crate::storage::rel::{Query, RelationalDb, RelationalTx, RowValues};
use std::sync::Arc;

const SET_COLS: &str = "set_id, path, request_key, strategy, settings, requested_strategy, \
     requested_settings, is_default, fallback_from, chunk_count, token_count, source_sha, built_at";

const CHUNK_COLS: &str =
    "number, unit, text, heading_path, page_start, page_end, token_count, figures, document_title";

/// Sets one file may hold, the default set included (FR-NEW-017).
pub const MAX_SETS: usize = 5;

#[derive(Clone)]
pub struct ChunkStore {
    db: Arc<dyn RelationalDb>,
    volume_id: String,
}

impl ChunkStore {
    /// No IO: the tables are created with the meta schema when the volume opens.
    pub fn new(db: Arc<dyn RelationalDb>, volume_id: impl Into<String>) -> Self {
        Self { db, volume_id: volume_id.into() }
    }

    fn read_set(r: &RowValues) -> Result<Set> {
        Ok(Set {
            set_id: r.text(0)?,
            path: r.text(1)?,
            request_key: r.text(2)?,
            strategy: r.text(3)?,
            settings: r.text(4)?,
            requested_strategy: r.text(5)?,
            requested_settings: r.text(6)?,
            is_default: r.i64(7)? != 0,
            fallback_from: r.opt_text(8)?,
            chunk_count: r.i64(9)?,
            token_count: r.i64(10)?,
            source_sha: r.text(11)?,
            built_at: r.text(12)?,
        })
    }

    fn read_chunk(r: &RowValues) -> Result<Chunk> {
        // Values were written by this store from `u32` fields, so a failed
        // narrowing means a hand edited row: read it as 0 rather than fail.
        let n = |i: usize| -> Result<u32> { Ok(u32::try_from(r.i64(i)?).unwrap_or(0)) };
        let on = |i: usize| -> Result<Option<u32>> {
            Ok(r.opt_i64(i)?.map(|v| u32::try_from(v).unwrap_or(0)))
        };
        let list = |i: usize| -> Result<Vec<String>> {
            Ok(serde_json::from_str(&r.text(i)?).unwrap_or_default())
        };
        Ok(Chunk {
            number: n(0)?,
            unit: n(1)?,
            text: r.text(2)?,
            heading_path: list(3)?,
            page_start: on(4)?,
            page_end: on(5)?,
            token_count: n(6)?,
            figures: list(7)?,
            document_title: r.text(8)?,
        })
    }

    /// The sets of one file in the order of FR-NEW-014: the default set first,
    /// then `built_at` oldest first, then `set_id`. At most 5 rows per file.
    pub async fn list_sets(&self, path: &str) -> Result<Vec<Set>> {
        let q = Query::new(format!(
            "SELECT {SET_COLS} FROM chunk_sets WHERE volume_id=?1 AND path=?2 \
             ORDER BY is_default DESC, built_at ASC, set_id ASC"
        ))
        .bind(&self.volume_id)
        .bind(path);
        self.db.query(&q).await?.iter().map(Self::read_set).collect()
    }

    /// Every path holding at least one set, one entry per set, sorted. Tests
    /// use it to prove no row outlives its file.
    #[cfg(test)]
    pub(crate) async fn paths_with_sets(&self) -> Result<Vec<String>> {
        let q = Query::new("SELECT path FROM chunk_sets WHERE volume_id=?1 ORDER BY path")
            .bind(&self.volume_id);
        self.db.query(&q).await?.iter().map(|r| r.text(0)).collect()
    }

    /// The default set of one file, if it has one (at most one row).
    pub async fn default_set(&self, path: &str) -> Result<Option<Set>> {
        let q = Query::new(format!(
            "SELECT {SET_COLS} FROM chunk_sets WHERE volume_id=?1 AND path=?2 AND is_default=1"
        ))
        .bind(&self.volume_id)
        .bind(path);
        self.db.query_opt(&q).await?.as_ref().map(Self::read_set).transpose()
    }

    /// One set by id, on `idx_chunk_sets_id`.
    pub async fn get_set(&self, set_id: &str) -> Result<Option<Set>> {
        let q = Query::new(format!(
            "SELECT {SET_COLS} FROM chunk_sets WHERE volume_id=?1 AND set_id=?2"
        ))
        .bind(&self.volume_id)
        .bind(set_id);
        self.db.query_opt(&q).await?.as_ref().map(Self::read_set).transpose()
    }

    /// At most `limit` chunks numbered above `after`, by number, on the
    /// primary key.
    pub async fn list_chunks(&self, set_id: &str, after: u32, limit: usize) -> Result<Vec<Chunk>> {
        let q = Query::new(format!(
            "SELECT {CHUNK_COLS} FROM chunks WHERE volume_id=?1 AND set_id=?2 AND number>?3 \
             ORDER BY number ASC LIMIT {limit}"
        ))
        .bind(&self.volume_id)
        .bind(set_id)
        .bind(i64::from(after));
        self.db.query(&q).await?.iter().map(Self::read_chunk).collect()
    }

    /// The chunks numbered `from` to `to` inclusive, by number; numbers beyond
    /// the ends simply match no row.
    pub async fn get_chunk_window(&self, set_id: &str, from: u32, to: u32) -> Result<Vec<Chunk>> {
        let q = Query::new(format!(
            "SELECT {CHUNK_COLS} FROM chunks WHERE volume_id=?1 AND set_id=?2 \
             AND number>=?3 AND number<=?4 ORDER BY number ASC"
        ))
        .bind(&self.volume_id)
        .bind(set_id)
        .bind(i64::from(from))
        .bind(i64::from(to));
        self.db.query(&q).await?.iter().map(Self::read_chunk).collect()
    }

    async fn insert_chunks(
        &self,
        tx: &mut Box<dyn RelationalTx>,
        set_id: &str,
        chunks: &[Chunk],
    ) -> Result<()> {
        for c in chunks {
            tx.execute(
                &Query::new(format!(
                    "INSERT INTO chunks (volume_id, set_id, {CHUNK_COLS}) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
                ))
                .bind(&self.volume_id)
                .bind(set_id)
                .bind(i64::from(c.number))
                .bind(i64::from(c.unit))
                .bind(&c.text)
                .bind(serde_json::to_string(&c.heading_path).unwrap_or_else(|_| "[]".into()))
                .bind(c.page_start.map(i64::from))
                .bind(c.page_end.map(i64::from))
                .bind(i64::from(c.token_count))
                .bind(serde_json::to_string(&c.figures).unwrap_or_else(|_| "[]".into()))
                .bind(&c.document_title),
            )
            .await?;
        }
        Ok(())
    }

    /// The counts a set reports are those of its chunks (FR-NEW-014), so they
    /// are derived here rather than trusted from the caller.
    fn counts(chunks: &[Chunk]) -> (i64, i64) {
        let tokens: i64 = chunks.iter().map(|c| i64::from(c.token_count)).sum();
        (chunks.len() as i64, tokens)
    }

    /// The set of `path` stored for the request `request_key`, on the primary key.
    pub async fn find_set(&self, path: &str, request_key: &str) -> Result<Option<Set>> {
        let q = Query::new(format!(
            "SELECT {SET_COLS} FROM chunk_sets WHERE volume_id=?1 AND path=?2 AND request_key=?3"
        ))
        .bind(&self.volume_id)
        .bind(path)
        .bind(request_key);
        self.db.query_opt(&q).await?.as_ref().map(Self::read_set).transpose()
    }

    /// Inserts a new set and its chunks in one transaction and returns the set
    /// the file now holds for that request. Inside the transaction, in the
    /// order of FR-NEW-017: an equal request already stored is returned as is
    /// (DEC-005), then the file's sets are counted and a sixth is refused
    /// (DEC-006). Two equal requests racing past the lookup meet the primary
    /// key: the loser rolls back and returns the winner's row.
    pub async fn insert_set(&self, set: &Set, chunks: &[Chunk]) -> Result<Set> {
        match self.try_insert(set, chunks).await {
            Ok(stored) => Ok(stored),
            Err(e) => match self.find_set(&set.path, &set.request_key).await? {
                Some(winner) => Ok(winner),
                None => Err(e),
            },
        }
    }

    async fn try_insert(&self, set: &Set, chunks: &[Chunk]) -> Result<Set> {
        let (count, tokens) = Self::counts(chunks);
        let mut tx = self.db.begin().await?;
        let equal = tx
            .query_opt(
                &Query::new(format!(
                    "SELECT {SET_COLS} FROM chunk_sets WHERE volume_id=?1 AND path=?2 \
                     AND request_key=?3"
                ))
                .bind(&self.volume_id)
                .bind(&set.path)
                .bind(&set.request_key),
            )
            .await?;
        if let Some(row) = equal {
            return Self::read_set(&row);
        }
        let held = tx
            .query(
                &Query::new("SELECT set_id FROM chunk_sets WHERE volume_id=?1 AND path=?2")
                    .bind(&self.volume_id)
                    .bind(&set.path),
            )
            .await?
            .len();
        if held >= MAX_SETS {
            return Err(ToolError::invalid_argument("a file holds at most 5 chunk sets"));
        }
        tx.execute(
            &Query::new(format!(
                "INSERT INTO chunk_sets (volume_id, {SET_COLS}) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ))
            .bind(&self.volume_id)
            .bind(&set.set_id)
            .bind(&set.path)
            .bind(&set.request_key)
            .bind(&set.strategy)
            .bind(&set.settings)
            .bind(&set.requested_strategy)
            .bind(&set.requested_settings)
            .bind(i64::from(set.is_default))
            .bind(set.fallback_from.clone())
            .bind(count)
            .bind(tokens)
            .bind(&set.source_sha)
            .bind(&set.built_at),
        )
        .await?;
        self.insert_chunks(&mut tx, &set.set_id, chunks).await?;
        tx.commit().await?;
        Ok(Set { chunk_count: count, token_count: tokens, ..set.clone() })
    }

    /// Removes the set `set_id` and its chunks in one transaction. Returns
    /// false when no such set exists, in which case nothing is written.
    pub async fn delete_set(&self, set_id: &str) -> Result<bool> {
        let mut tx = self.db.begin().await?;
        let removed = tx
            .execute(
                &Query::new("DELETE FROM chunk_sets WHERE volume_id=?1 AND set_id=?2")
                    .bind(&self.volume_id)
                    .bind(set_id),
            )
            .await?;
        if removed == 0 {
            return Ok(false);
        }
        tx.execute(
            &Query::new("DELETE FROM chunks WHERE volume_id=?1 AND set_id=?2")
                .bind(&self.volume_id)
                .bind(set_id),
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Removes every set and chunk of the volume in one transaction, for a
    /// project that is soft deleted (FR-NEW-025). Cost: two statements.
    pub async fn delete_all_sets(&self) -> Result<()> {
        let mut tx = self.db.begin().await?;
        for table in ["chunks", "chunk_sets"] {
            tx.execute(
                &Query::new(format!("DELETE FROM {table} WHERE volume_id=?1"))
                    .bind(&self.volume_id),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// The hook a soft delete runs: forgets every set of the project's volume.
    /// A failure is logged, not raised, as the project is already deleted.
    pub fn project_deleted_hook(
        stores: Arc<crate::storage::StoreManager>,
    ) -> crate::storage::traits::ProjectGoneHook {
        Arc::new(move |project: String| {
            let stores = stores.clone();
            Box::pin(async move {
                let sets = match stores.client(&project).await {
                    Ok(c) => c.meta.chunk_store(),
                    Err(e) => {
                        tracing::warn!(project, error = %e, "cannot open the volume to drop its sets");
                        return;
                    }
                };
                if let Some(Err(e)) = match sets {
                    Some(s) => Some(s.delete_all_sets().await),
                    None => None,
                } {
                    tracing::warn!(project, error = %e, "cannot drop the sets of a project");
                }
            })
        })
    }

    /// Removes every set of `path` and their chunks in one transaction, for a
    /// file that is no longer text (FR-NEW-021).
    pub async fn delete_sets_of(&self, path: &str) -> Result<()> {
        let mut tx = self.db.begin().await?;
        tx.execute(
            &Query::new(
                "DELETE FROM chunks WHERE volume_id=?1 AND set_id IN \
                 (SELECT set_id FROM chunk_sets WHERE volume_id=?1 AND path=?2)",
            )
            .bind(&self.volume_id)
            .bind(path),
        )
        .await?;
        tx.execute(
            &Query::new("DELETE FROM chunk_sets WHERE volume_id=?1 AND path=?2")
                .bind(&self.volume_id)
                .bind(path),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Replaces the chunks and the build fields of the set `set.set_id` in one
    /// transaction, keeping its id and path (DEC-013): a reader sees all the
    /// old chunks or all the new ones. Returns false when no such set exists,
    /// in which case nothing is written.
    ///
    /// Another set of the same file holding the new `request_key` is removed in
    /// the same transaction: the default set taking the key of a set made by
    /// hand absorbs it (DEC-005), and the primary key cannot hold both.
    pub async fn replace_set(&self, set: &Set, chunks: &[Chunk]) -> Result<bool> {
        let (count, tokens) = Self::counts(chunks);
        let mut tx = self.db.begin().await?;
        let absorbed = tx
            .query(
                &Query::new(
                    "SELECT set_id FROM chunk_sets WHERE volume_id=?1 AND path=?2 \
                     AND request_key=?3 AND set_id<>?4",
                )
                .bind(&self.volume_id)
                .bind(&set.path)
                .bind(&set.request_key)
                .bind(&set.set_id),
            )
            .await?;
        for row in &absorbed {
            let id = row.text(0)?;
            for table in ["chunks", "chunk_sets"] {
                tx.execute(
                    &Query::new(format!("DELETE FROM {table} WHERE volume_id=?1 AND set_id=?2"))
                        .bind(&self.volume_id)
                        .bind(&id),
                )
                .await?;
            }
        }
        let updated = tx
            .execute(
                &Query::new(
                    "UPDATE chunk_sets SET request_key=?1, strategy=?2, settings=?3, \
                     requested_strategy=?4, requested_settings=?5, is_default=?6, \
                     fallback_from=?7, chunk_count=?8, token_count=?9, source_sha=?10, \
                     built_at=?11 WHERE volume_id=?12 AND set_id=?13",
                )
                .bind(&set.request_key)
                .bind(&set.strategy)
                .bind(&set.settings)
                .bind(&set.requested_strategy)
                .bind(&set.requested_settings)
                .bind(i64::from(set.is_default))
                .bind(set.fallback_from.clone())
                .bind(count)
                .bind(tokens)
                .bind(&set.source_sha)
                .bind(&set.built_at)
                .bind(&self.volume_id)
                .bind(&set.set_id),
            )
            .await?;
        if updated == 0 {
            // Dropping the handle rolls back: nothing was written.
            return Ok(false);
        }
        tx.execute(
            &Query::new("DELETE FROM chunks WHERE volume_id=?1 AND set_id=?2")
                .bind(&self.volume_id)
                .bind(&set.set_id),
        )
        .await?;
        self.insert_chunks(&mut tx, &set.set_id, chunks).await?;
        tx.commit().await?;
        Ok(true)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn chunk(number: u32, text: &str) -> Chunk {
        Chunk {
            number,
            unit: number,
            text: text.to_string(),
            heading_path: vec!["Big".into()],
            page_start: None,
            page_end: None,
            token_count: 3,
            figures: vec![],
            document_title: "big".into(),
        }
    }

    pub(crate) fn set(set_id: &str, path: &str, key: &str, built_at: &str) -> Set {
        Set {
            set_id: set_id.into(),
            path: path.into(),
            request_key: key.into(),
            strategy: "structural".into(),
            settings: r#"{"max_tokens":64}"#.into(),
            requested_strategy: "structural".into(),
            requested_settings: "{}".into(),
            is_default: false,
            fallback_from: None,
            chunk_count: 0,
            token_count: 0,
            source_sha: String::new(),
            built_at: built_at.into(),
        }
    }

    async fn store() -> ChunkStore {
        let db = Arc::new(crate::storage::rel::SqliteRelationalDb::open_in_memory().unwrap());
        crate::storage::meta::RelationalMetaStore::open(db.clone(), "vol").await.unwrap();
        ChunkStore::new(db, "vol")
    }

    #[tokio::test]
    async fn replacing_a_set_keeps_its_id_and_swaps_every_chunk() {
        let s = store().await;
        let old: Vec<Chunk> = (1..=4).map(|n| chunk(n, &format!("old {n}"))).collect();
        s.insert_set(&set("set-00000001", "/a.md", "k", "2026-11-20T10:00:00Z"), &old)
            .await
            .unwrap();
        let mut next = set("set-00000001", "/a.md", "k", "2026-11-20T10:05:00Z");
        next.source_sha = "abc".into();
        let new: Vec<Chunk> = (1..=2).map(|n| chunk(n, &format!("new {n}"))).collect();
        assert!(s.replace_set(&next, &new).await.unwrap());

        let got = s.get_set("set-00000001").await.unwrap().unwrap();
        assert_eq!((got.chunk_count, got.token_count), (2, 6));
        assert_eq!(got.source_sha, "abc");
        assert_eq!(got.built_at, "2026-11-20T10:05:00Z");
        let texts: Vec<String> = s
            .list_chunks("set-00000001", 0, 100)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.text)
            .collect();
        assert_eq!(texts, ["new 1", "new 2"]);
        assert!(!s.replace_set(&set("set-ghost", "/a.md", "k", "x"), &new).await.unwrap());
    }

    #[tokio::test]
    async fn a_replace_absorbs_the_set_holding_its_new_key_and_a_path_drop_clears_all() {
        let s = store().await;
        s.insert_set(&set("set-00000001", "/a.md", "old", "t"), &[chunk(1, "d")]).await.unwrap();
        s.insert_set(&set("set-00000002", "/a.md", "new", "t"), &[chunk(1, "h")]).await.unwrap();
        s.insert_set(&set("set-00000003", "/b.md", "new", "t"), &[chunk(1, "b")]).await.unwrap();
        assert!(s.replace_set(&set("set-00000001", "/a.md", "new", "t2"), &[]).await.unwrap());
        let ids: Vec<String> =
            s.list_sets("/a.md").await.unwrap().into_iter().map(|x| x.set_id).collect();
        assert_eq!(ids, ["set-00000001"], "the set with the key is absorbed");
        assert!(s.list_chunks("set-00000002", 0, 10).await.unwrap().is_empty());
        assert_eq!(s.list_sets("/b.md").await.unwrap().len(), 1, "another file is untouched");

        s.delete_sets_of("/b.md").await.unwrap();
        assert!(s.list_sets("/b.md").await.unwrap().is_empty());
        assert!(s.list_chunks("set-00000003", 0, 10).await.unwrap().is_empty());
        assert_eq!(s.list_sets("/a.md").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn an_equal_request_cannot_be_stored_twice_and_volumes_are_apart() {
        let s = store().await;
        s.insert_set(&set("set-00000001", "/a.md", "k", "t"), &[chunk(1, "x")]).await.unwrap();
        let again = s.insert_set(&set("set-00000002", "/a.md", "k", "t"), &[]).await.unwrap();
        assert_eq!(again.set_id, "set-00000001", "the stored set answers an equal request");
        assert!(s.get_set("set-00000002").await.unwrap().is_none(), "nothing left behind");
        let other = ChunkStore::new(s.db.clone(), "vol2");
        assert!(!other.delete_set("set-00000001").await.unwrap(), "volumes apart");
        assert!(other.get_set("set-00000001").await.unwrap().is_none());
        assert!(other.list_sets("/a.md").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_sixth_set_is_refused_and_delete_frees_a_place() {
        let s = store().await;
        for i in 1..=5 {
            let id = format!("set-0000000{i}");
            s.insert_set(&set(&id, "/a.md", &format!("k{i}"), "t"), &[chunk(1, "x")])
                .await
                .unwrap();
        }
        let e = s.insert_set(&set("set-00000006", "/a.md", "k6", "t"), &[]).await.unwrap_err();
        assert_eq!(e.to_string(), "ERR_INVALID_ARGUMENT: a file holds at most 5 chunk sets");
        // An equal request is answered before the count.
        let same = s.insert_set(&set("set-0000000x", "/a.md", "k5", "t"), &[]).await.unwrap();
        assert_eq!(same.set_id, "set-00000005");
        // Another file has its own places.
        s.insert_set(&set("set-0000000b", "/b.md", "k1", "t"), &[]).await.unwrap();

        assert!(s.delete_set("set-00000002").await.unwrap());
        assert!(s.list_chunks("set-00000002", 0, 10).await.unwrap().is_empty(), "chunks gone");
        assert!(!s.delete_set("set-00000002").await.unwrap());
        s.insert_set(&set("set-00000006", "/a.md", "k6", "t"), &[]).await.unwrap();
        assert_eq!(s.list_sets("/a.md").await.unwrap().len(), 5);
    }
}
