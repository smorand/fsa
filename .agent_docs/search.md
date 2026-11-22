# Search Tools

The `search.*` family (4 tools: `search.index`, `search.query`, `search.delete`,
`search.status`) adds BM25 full-text search and optional RAG vector search to every
volume. They are registered only when `search.enabled: true` in the server config.

## Config keys

```yaml
search:
  enabled: false        # master switch; false = no tools registered
  mode: bm25            # bm25 | rag | both
  tantivy_dir: state/search  # base dir for Tantivy on-disk indexes (SQLite only)

  embedding:
    endpoint: ""        # OpenAI-compatible /v1/embeddings URL (required for rag/both)
    model: text-embedding-3-small
    api_key_env: FSA_EMBEDDING_KEY  # env var holding the Bearer key
    dimensions: 1536    # must match the model; frozen after first index call

  reranking:
    enabled: false      # set true to call the rerank endpoint on search.query
    endpoint: ""        # Cohere/Jina rerank URL
    model: ""
    api_key_env: FSA_RERANK_KEY
    top_n: 10           # how many chunks to send to the reranker
```

## Per-project index mode

`search.mode` above is the SERVER mode: which engine is wired. Whether a given
project's content reaches that engine is a per-project property, `index_mode`, stored
on the `project` row and managed by two tools:

| tool | gate | returns |
|---|---|---|
| `admin.set_index_mode` | owner or platform admin | `project_id`, `index_mode`, `previous_mode`, `reindex_started` |
| `admin.get_index_mode` | member, or platform admin | `project_id`, `index_mode` |

Values: `none` (the default for every project, including every project that existed
before the column shipped), `bm25`, `rag`, `both`. `rag` and `both` are refused with
`ERR_INVALID_ARGUMENT` when `search.embedding.endpoint` is empty; any value is refused
with `ERR_NOT_SUPPORTED` when `search.enabled` is false; an unknown value is
`ERR_INVALID_ARGUMENT`.

### Transitions

| From \ To | none | bm25 | rag | both |
|---|---|---|---|---|
| **none** | no-op | wipe + full index | wipe + full index | wipe + full index |
| **bm25** | wipe | no-op | wipe + full index | wipe + full index |
| **rag** | wipe | wipe + full index | no-op | wipe + full index |
| **both** | wipe | wipe + full index | wipe + full index | no-op |

"Wipe" is `SearchBackend::delete_all(volume_id)`, scoped to the one volume, and it is
finished before the tool answers. "Full index" is a detached `tokio` task walking every
file of the volume, so a `reindex_started: true` answer means a pass is RUNNING, not
that the volume is searchable. Poll `search.status` and watch `bm25_docs` /
`vector_chunks` settle. A same-mode call is a real no-op: it neither wipes nor rebuilds,
which is why it reports `reindex_started: false`.

### Auto indexing

While a project's mode is active, every write reaches the index without a
`search.index` call: `fs.write`, `fs.append`, `fs.write_bytes`, `fs.write_docx`,
`fs.edit`, `fs.multi_edit`, `fs.search_replace`, `fs.insert_at_line` and
`fs.apply_patch`. `fs.delete` removes the deleted path, and
every file underneath it for a recursive delete. `fs.move` and `fs.copy` follow the
bytes (see below). A `dry_run` edit writes nothing and so
indexes nothing. Files that are not valid UTF-8 are skipped silently, which covers
binary uploads and the `.docx` that `fs.write_docx` produces.

`fs.create_empty` runs no hook: an empty file has no content to index.

The REST data plane is the same engine behind a second door, and it runs the same
hooks: every `/api/fs/{mount}/` route that mutates content calls the very helper
its `fs.*` twin calls, so the two doors index the same thing for write, append,
edit, multi-edit, search-replace, insert-at-line, apply-patch, write-bytes,
write-docx, extract-text, documentize, delete, move, copy and upload.

#### The Markdown companion

`fs.extract_text`, `fs.documentize` and a `trigger_documentation_service` write
store a Markdown COMPANION next to the source, and that companion is the text
RAG exists for: the source itself is a PDF or a zip the reread skips. It is
indexed by `indexer::after_companion`, the one helper that decodes both payload
shapes (a top level `md_path`, and the nested `documentation.md_path` a
documented write returns). A null companion or a failed conversion indexes
nothing. A documented upload indexes BOTH the source, when it is text, and the
companion.

The work never sits on the write path (`search/indexer.rs`): the write returns as soon
as the bytes are committed. A write, edit, patch or upload goes through the rebuild
scheduler (see "Rebuild scheduler" below) and is searchable once the file has been quiet
for `search.settle_delay_seconds` (default 30) and its rebuild ended. The companion, a
move and a copy still index at once on a detached task whose failures are a WARN log.
There is no read-your-write guarantee on the index. A caller that needs one must poll
`search.status` or `fs.list_derived_jobs`, or call `search.index` synchronously.

#### Move and copy

`fs.move` drops the index entries of the source and indexes the destination by reading
it back from the volume; `fs.copy` indexes the destination and leaves the source's
entries alone, because a copy leaves the source's bytes alone. Both follow the WHOLE
subtree, not just the named path, so a recursive move or copy of a tree moves or
duplicates every entry underneath it. The REST `POST /api/fs/{mount}/move` and
`/copy` call the same two helpers (`search/indexer.rs`), so the two doors cannot drift.

**Ordering matters on a move**: both the source tree AND the destination tree are
enumerated BEFORE the rename (`paths_displaced_by_move`), and the destination is walked
again after it. With `overwrite: true` the engine deletes the destination outright, so a
file that lived only there is gone from the volume and must be dropped from the index
too; after the rename neither tree is left to enumerate. Every clear is then AWAITED
before the re-index starts, because the two sets overlap on a file present under both
trees and both hooks are detached: a delete landing after the write would drop the fresh
chunks of a file that does exist. `fs.copy` needs none of this, because `copy_tree`
merges into the destination and deletes nothing there.

A move or copy that FAILS leaves the index untouched: the hooks run only on the success
path, after the engine has returned. Non UTF-8 destinations are skipped silently, and
directories carry no entries of their own.

When the mode is `none`, a write costs one extra `project` row read and nothing else.
Entries are the chunks of the file's default set (see "Chunking" below); there is no
per-project chunk configuration.

The mode decides WHETHER a project is indexed, not by which engine: the backend is
process wide and serves whatever `search.mode` configured. Setting `bm25` on a server
running `search.mode: rag` indexes into the vector store.

### Crash mid-reindex

The wipe commits before the rebuild starts, so a server that dies during the initial
full index leaves the project with an active mode and a partial or empty index. The
state is consistent, never corrupt, but nothing restarts the pass: call
`admin.set_index_mode` with a DIFFERENT mode and back, or `search.index` with
`recursive: true`, to rebuild. Same for the storage schema: `index_mode` is written
before the wipe, so the failure mode is a stale index under a correct mode rather than
a wiped index under a mode nothing will rebuild.

## Modes

| mode  | BM25 engine    | Vector engine  | Merge | Reranking |
|-------|----------------|----------------|-------|-----------|
| bm25  | Tantivy        | none           | none  | no        |
| rag   | none           | sqlite-vec     | none  | optional  |
| both  | Tantivy        | sqlite-vec     | RRF   | optional  |

The `mode` param on `search.query` overrides the server default per call.

Requesting a mode that requires a capability that is not provisioned (for example
`rag` with an empty `embedding.endpoint`) returns `ERR_NOT_SUPPORTED`.

## PostgreSQL (future)

When `infra.meta.backend = postgres`, BM25 would use `tsvector` + GIN and vectors
would use `pgvector`. The `CREATE EXTENSION IF NOT EXISTS vector` DDL runs in the
pool's `after_connect` hook when the `rag` feature is compiled in. The `VECTOR(N)`
column type renders via `ColumnType::Vector(u32)` in `storage/rel/dialect.rs`.

This path is wired in the dialect and schema layers but the PostgreSQL search backend
itself is not yet implemented. The current implementation uses Tantivy (BM25) and
sqlite-vec (RAG) regardless of the infra backend.

## SQLite: Tantivy BM25

Tantivy index directories live at `{tantivy_dir}/{volume_id}/`. The index is:

- **Ephemeral**: a pod restart or volume remount loses the index. Must re-index after
  restart via `search.index`.
- **Warm check**: `search.status` reports `bm25_warm: false` when the directory is
  absent. `search.query` adds a `warning` field in this case.
- **Idempotent**: an index call replaces the entries of a path in one commit.
- **Rebuildable**: call `search.index` with `recursive: true` to re-index a tree.

## SQLite: sqlite-vec

sqlite-vec 0.1.9 (pinned, pre-v1) provides `vec0` virtual tables for vector KNN. The
extension is registered once at process start via `sqlite3_auto_extension` when the
`rag` cargo feature is on. The extension is statically linked via the `cc` crate,
so no system library is needed (same posture as `rusqlite` with `bundled`).

Vectors are stored in a `vec0` virtual table alongside a `search_vec_meta` companion
table. The DDL uses the `SchemaSet::virtual_tables` field in the schema renderer.

## Chunking

### Entries are the chunks of the default set (SPEC-0101, US-0279, 2026-11-22)

Search never cuts text: the character chunker (`search/chunker.rs`, `AUTO_CHUNK_*`) is gone.
`SearchBackend::index_chunks_checked(volume, path, &[IndexedChunk], guard)` replaces the path's
entries by one entry per chunk of the file's default set: the text column holds the indexed text,
`chunk_idx` the chunk number (1 based). No engine schema change, so an old Tantivy index opens.
Indexed text (DEC-010): `<document title>[ > <heading path titles joined by " > ">]`, a newline,
then the chunk text, so a word of a heading or of the file name finds the chunk.
`CombinedBackend` writes the vector side first (embedding is what fails), then BM25.

`search::indexer::index_default_set` is the one indexing path: `search.index`, the mode change full
index, the hooks without a scheduler, the companion of a converted document and the scheduler rebuild
call it. It reads the node's `sha256`; a current default set is indexed as it is unless `recut` (the
rebuild and the companion recut). Otherwise `chunking::service::rebuild_file_sets` cuts EVERY set of the
file without writing (default set with the server defaults of the moment, created when missing; extra
sets from their stored `request_key`, so their settings and key never follow the defaults), the engine
indexes the default chunks (guarded), and only then `Built::commit` writes the sets, one transaction
each, guard checked before each. So an embedding failure or a discard leaves the previous sets AND
entries (FR-NEW-022, US-0280 E2E-002/003). A default whose key changed with the defaults takes the new
key; `ChunkStore::replace_set` removes, in its transaction, any other set of the file holding that key
(a hand made set absorbed, DEC-005). The default set is `structural` (a text without heading is the
`fixed` fallback).

### The rebuild builds the sets (SPEC-0101, US-0280, 2026-11-22)

`Scheduler::new(..., defaults)` takes the `ChunkDefaults` of the start (`app.rs`); a restart with new
`search.chunking.*` values rebuilds default sets with them. `Scheduler::rebuild_file`: a file gone is
`done`; a file that is not UTF-8 loses its sets (`ChunkStore::delete_sets_of`) and entries, job `done`,
unless it has a current artifact set (converted document), which keeps its sets `out_of_date`. A
project with indexing off schedules nothing, so no automatic default set; a mode change to `none`
wipes entries but keeps sets; back to active, the full index rebuilds each stale default set. Nothing
converts at boot: a file indexed before the upgrade keeps its character entries until its first
rebuild. Known limit: a file without a default set that already holds 5 hand made sets fails its
rebuild (`a file holds at most 5 chunk sets`). Tests: `chunking/e2e.rs`, US-0280 section
(`Env::restart_with` changes the config across a restart, `Options::config` at start).

`search.query` results carry `chunk_number` and `heading_path` (`tools/search_semantic.rs::query`,
shared by the MCP handler and the test registry): for a file with a default set, the text after the
header line, `chunk_idx` and the heading path of that chunk; for a file without one (entries built
before the upgrade, FR-NEW-026) the text unchanged, `chunk_number` 0, `heading_path` `[]`. Cost: one
set lookup per distinct path, one chunk lookup per such result. Fusion and ranking are unchanged
(`SearchResult::chunk_idx` rides along).

Engine unit tests index through `search::index_text` (cuts with the default structural settings)
or hand built `IndexedChunk` lists when a test needs exact entries.

### Chunk sets: the structural strategy (SPEC-0101, US-0277, 2026-11-22)

`chunking::cut` (`chunking/mod.rs`) cleans the text (NUL out, CRLF to LF) then dispatches.
`structural` parses the blocks once (`chunking/blocks.rs`, paragraph v3a 5.1, one pass, first
matching rule: code, `---` page marker or break, heading, table or metadata table, figure, list,
paragraph). No `Heading` block (a `#` line inside code is code) means the `fixed` fallback with the
server fixed defaults, `fallback_from: structural`, the set settings being those fixed defaults; an
empty text is that fallback with 0 chunks. Otherwise `chunking/structural.rs`: sections cut at a
heading leaving at most `heading_cut_depth` open (the stack counts open headings, not `#`), heading
path = first 2 titles of the stack when the section starts, pages from the markers, figures as
references on the chunk of the last unit starting before them (the first unit if none), units
(prose runs; table, list, code atomic; a paragraph ending with `:` gives its last line to the list),
lists above `list_max_items` in batches marked split, then merging when the top level titles are
equal or both absent (FR-NEW-006, the deliberate difference from v3a 5.4) and the `\n\n` joined text
fits `max_tokens`. A unit above `max_tokens` stays whole until US-0278 cuts it. Tests: texts with no
heading begin with `# T` (spec section 9); `chunking::e2e::prose(n)` gives exactly `n` tokens.

## RRF Fusion

`mode=both` merges BM25 and vector results using Reciprocal Rank Fusion with k=60.
A document appearing in both lists scores higher than one in only one list.
The merge is deterministic: same inputs always produce the same output.

## Reranking

When `search.reranking.enabled: true` and `rerank: true` (the default on
`search.query`), the results are re-scored by the configured endpoint using the
Cohere/Jina rerank API shape:

- POST `{"model": "...", "query": "...", "documents": ["text1", ...]}`
- Response `{"results": [{"index": 0, "relevance_score": 0.9}, ...]}`

A reranking failure is non-fatal: the original order is returned with a log warning.

## Index lifecycle

1. `search.index` (required before any query)
2. `search.query` (BM25, RAG, or both)
3. `search.delete` (removes a path)
4. `search.status` (reports `bm25_docs`, `vector_chunks`, `bm25_warm`, `mode`)

`search.index(mount_id, path, recursive)` builds or refreshes the default set of each text file
then indexes it; `indexed` counts the chunks. Calling it twice gives the same count, the same set
id and chunks. `chunk_size` or `chunk_overlap` are refused: `ERR_INVALID_ARGUMENT: chunk_size and
chunk_overlap are no longer accepted` (kept in `SearchIndexArgs` with `#[schemars(skip)]`, so not in
the schema). A single path with no text: `ERR_NOT_SUPPORTED: no text to cut for <path>`; inside a
recursive call such a file is counted in `skipped`. An embedding failure on a single path answers
`embedding service unreachable` and leaves entries and set as they were.

## Authorization

All `search.*` tools call `state.authorize(mount_id, person)` before any storage
access, the same gate as `fs.*`. Platform admins do not get implicit search access;
they must be a project member.

## Testing

Use the `testkit::harness_with_search` helper in unit tests. It injects a real
`TantivyBm25Backend` so tests run offline without any embedding server. For RAG/both
mode integration tests, use `scripts/search_embedding_fake.py` as a stub server.

## Risks

1. **sqlite-vec is pre-v1** (pinned at 0.1.9). Breaking changes expected before 1.0.
   Blast radius is contained to `search/vector_sqlite.rs`.

2. **Tantivy index is ephemeral** on SQLite. Re-index after any pod restart. The
   `search.status` tool and the `warning` field on `search.query` signal this.

3. **Embedding dimension freeze**: once vectors are stored at dimension N, changing
   `search.embedding.dimensions` breaks all stored vectors silently. Delete chunks
   and re-index after any dimension change.

4. **`delete_all` on Tantivy removes the index directory** (`{tantivy_dir}/{volume_id}/`)
   rather than deleting term by term, which avoids segment fragmentation and is safe
   because the directory is always rebuildable. The cached open index is evicted with
   it, so the next write reopens a fresh one.

## The trash rule (2026-10-13)

"The trash" is every path at or under `/{trash_dir}` (`safety.trash_dir`, default
`.mcp_trash`), top level only: `/docs/.mcp_trash/a.txt` is an ordinary, indexed file
(SPEC-0025). Five places that used to disagree with this now all follow it:

- A mode change backfill and a recursive `search.index` skip the trash
  (`search::indexer::is_trash_path`, used by `full_index` and `files_under`).
- `fs.move` into the trash folder drops the source's chunks and does not index the
  destination (same walkers).
- The auto purge sweep (`purge::sweep_project_files`) removes the original path's
  chunks with `SearchBackend::delete_path`, right after the move to the trash
  succeeds, when the project's index mode is active (`admin.get_index_mode(..).
  is_active()`). A failed delete is a WARN log, never a reason to undo the move or
  fail the sweep: an index failure never blocks a purge.
- `fs.trash_restore` calls `search::indexer::after_move` after the rename, exactly
  like `fs.move`: it clears the trash path's chunks and indexes the restored path,
  `_restored`/`_restoredN` collisions included. Same failure rule: detached, logged,
  never fails the restore.

An explicit, non recursive `search.index` on one named trash path still indexes it:
the caller named it, so `search.query` can legitimately return a trash path in that
one case.

`purge::sweep_project_files` and `purge::run_cycle` both take an
`Option<&Arc<dyn SearchBackend>>`; the background loop (`app.rs`) and the CLI `purge`
verb (`cli.rs`, same backend the grace sweep already built) pass the server's
configured backend, `None` when search is disabled.


## Guarded index update (SPEC-0100, US-0264)

`SearchBackend::index_path_checked(..., still_current)` was a provided method before SPEC-0101; since then `index_chunks_checked` replaces it and `index_path` (the engines and `CombinedBackend` implement the new method). The chunking and
the embedding run first, then `still_current` (a plain synchronous closure) runs right
before the commit: false answers `Ok(None)` and stores nothing, an error before the
commit stores nothing either, so the previous entries of the path stay. Tantivy checks
under its `write_lock` before the single delete plus add commit; PostgreSQL BM25,
pgvector and sqlite-vec check before the delete, then run delete plus inserts in one
transaction (US-0265, 2026-11-21): a failed insert drops the transaction, so a reader sees
all the old rows or all the new ones. Embeddings are computed before it opens.
`CombinedBackend` updates the vector side first (the embedding can fail), BM25 second.

## Rebuild scheduler (SPEC-0100, US-0268, 2026-11-21)

`search/derived/scheduler.rs`. `app.rs` wraps the engine in `search::ScheduledBackend`
(DEC-014), which delegates every engine call and answers `scheduler()`; the hooks
`after_write`, `after_write_reread` (so `after_patch` too) call `Scheduler::schedule`
when it is `Some`, else keep the old immediate detached index (only tests that build an
`AppState` by hand). `after_companion`, `search.index`, `search.status`, move and copy
are unchanged (DEC-015). The hook still checks the project's mode first, so a project
with mode `none` schedules nothing.

- One `tokio::sync::Mutex` over a map `(project, path, kind)` to `{waiting, running}`, one
  dispatcher task woken by a `Notify`. Never held while a rebuild runs.
- A change: an existing waiting job gets `due = now + delay` (no row write, same job id);
  else a new row `waiting` is inserted. A running job of the key is marked stale.
- Delay 0: the waiting entry is `hidden` (not listed, starts at once, DEC-007).
- The dispatcher starts every due job whose key has nothing running (row `running`) and
  spawns the rebuild, which reads the file then (DEC-021) and calls
  `index_path_checked` with the guard `!stale`. Outcome: refused or stale `superseded`,
  error `failed` with DEC-019's reason (`embedding service unreachable` for the two
  `embed` failure texts, else `<ERR_CODE>: <message>`), else `done`. Not text or gone is
  `done`. `JobStore::finish` deletes the earlier finished row of the key (DEC-006). No
  retry on its own.
- Job times follow the tokio clock from a wall clock origin taken at start, so a paused
  test clock moves them too.
- `fs.list_derived_jobs` reads waiting and running jobs from the scheduler's memory
  (`seconds_left` rounded up) and finished jobs from the table.
- `fs.recalculate` (US-0273) calls `Scheduler::recalculate` once `tools::derived::recalculate`
  has passed the refusal order of design 5.2. The key's waiting job is dropped and its row
  deleted (no record). Nothing running: a new `manual` row is written `running` and the
  rebuild is spawned in the call. A running job: it is marked stale (ends `superseded`), and
  the new job is a listed waiting entry with `due = now` (`seconds_left` 0, not `hidden`,
  DEC-007), started by the dispatcher when the key frees. At most one running job per key.
- Kind `chunks` (SPEC-0101 US-0282, DEC-012): `Scheduler::recalculate_chunks` uses the key
  `(project, path, JobKind::Chunks)`, so it has its own slot beside the `search_index` one and
  neither cancels nor delays it. Manual only. The tool layer resolves the set (the default
  when `set_id` is empty), reads the text (`chunking::service::source_text`, which needs
  `AppState` for a converted document) and the file sha, and hands them to the scheduler as a
  `ChunkWork`; the job cuts that one set with `service::rebuild_one_set` and writes it
  behind the stale guard (`rebuild_chunk_set`). A default set with `reindex` (search on and
  the project indexes) first writes the search entries from its new chunks. The row carries
  `set_id` (nullable column of `derived_job`, additive `ColumnMigration`). A delete treats
  both kinds of the path, a move drops a `chunks` job (its text belongs to the old path), a
  restart ends every unfinished `chunks` row `failed` ("interrupted by a server restart")
  without a new job, since its text lived in memory only.
- Shutdown aborts the dispatcher; rows stay. Boot recovery is US-0270.
- Tests of kind `chunks`: `chunking/e2e.rs` (`e2e_0101_110` to `115`, `138`, `052`, `mod003`,
  `us0282_*`). SPEC-0100 `e2e_0100_069`, `013_015` and `us0273_refusal_order` were updated
  for the new refusal order and kind text (E2E-MOD-003).
- Tests: `search/derived_e2e.rs` (`e2e_0100_`), paused clock, `Stub` engine (in process
  embedding stand in with a delay queue, a down switch and start and end instants;
  the default build has no embedding client).


### Chunk sets and the lifecycle of their file (SPEC-0101, 2026-11-21)
Sets follow the node rows in the same storage transaction (`storage/meta.rs`): a move carries them, a delete or a move to the trash removes them, a copy copies none. A soft delete of a project removes every set through a second admin hook (`AdminBackend::set_project_deleted_hook`, wired in `app.rs` whatever the index mode); the first hook (`set_project_gone_hook`) belongs to the scheduler jobs. A hard delete clears the volume tables, chunk tables included.
