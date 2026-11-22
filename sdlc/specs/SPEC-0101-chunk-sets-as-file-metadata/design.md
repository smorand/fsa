# SPEC-0101: chunk sets as file metadata, design

> Spec: sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/spec.md
> Nature: FEAT
> Depth: L
> Security: n/a
> Designed on: 2026-11-21
> Target tier: 2 (default, no `Tier:` in `sdlc/constitution.md`, no `.spec.json`)

## 1. Stack and skills
| Element | Choice | Version | DEC |
|---|---|---|---|
| Language, runtime, web, MCP | Rust 2024, tokio, axum, rmcp | workspace manifest (rmcp 3.5.0) | existing |
| Relational layer | `storage::rel` (SQLite default, PostgreSQL optional) | in tree | DEC-004 |
| Search engines | tantivy, sqlx + pgvector, sqlite-vec | in tree | DEC-009 |
| Tokenizer | `tiktoken-rs` (new dependency), `cl100k_base` ranks embedded in the crate | 0.12.1 (crates.io, MIT, updated 2026-09-24; needs Rust 1.85 or newer) | DEC-001 |
| Ids, times, JSON, base64 | `uuid`, `chrono`, `serde_json`, `base64` | workspace manifest | DEC-005, DEC-013 |

Skill loaded: `rust` (already in context). Effect: `make check` is the gate (format, clippy `-D warnings --all-features`, tests), tests inside `crates/core/src/**` modules, comments explain WHY, no dashes as punctuation, `ToolError` for errors.
context7 had no quota left on 2026-11-21 (`Monthly quota exceeded`), so the version above comes from the crates.io API (`max_stable_version` 0.12.1) and the crate README: this is the fallback allowed by the process. The function names used by the stories (`cl100k_base_singleton`, `encode_ordinary`, `decode_bytes`) are to be confirmed against docs.rs at the first story that touches them (US-0276); a differing name changes one line of `chunking/tokens.rs`.

## 2. Approaches
### Chosen: a pure `chunking` module, chunk sets persisted in the volume meta store, a rebuild that builds sets before entries
Cutting is pure functions (text and settings in, chunks out) so it is testable without a server and deterministic (FR-NEW-010). Sets and chunks are rows of two new tables of the per volume meta store, next to `doc_artifact_*`, and follow the file's rows in the same transaction (move, delete). The SPEC-0100 scheduler's rebuild cuts the sets first and hands the default set's chunks to the search engines through a new `index_chunks_checked`. Pros: one source of chunks for tools and search; lifecycle consistency by transaction; no new service. Cons: text is stored twice (file and chunks). Scope: new `chunking/`, `storage/meta.rs`, `search/*`, `tools/chunks.rs`, `mcp/server.rs`, `config.rs`.
### Rejected: chunks only inside the search index
The entry says chunks are metadata and vectors are not; engines differ per project mode (BM25 only has no vectors) and cannot serve `fs.get_chunk` with neighbours. Rejected (DEC-004).
### Rejected: compute chunks on every read
Reading chunk N with neighbours would cut the whole file per call, and "a set" would not be a stored thing with an id and a state. Rejected (DEC-004).
### Rejected: chunk tables in the admin database
The admin database is global and small; chunks are large, volume scoped and would need project cleanup. The volume meta store already holds `doc_artifact_*` the same way. Rejected (DEC-004).
### Rejected: the `tokenizers` crate
It needs a `tokenizer.json` file shipped and located at runtime; `tiktoken-rs` embeds the ranks and is the reference vocabulary of `cl100k_base` (DEC-001).

## 3. Architecture
```
 fs.create_chunk_set / list_chunk_sets / list_chunks / get_chunk / delete_chunk_set / fs.recalculate(kind chunks)
                  |  tools/chunks.rs  (authorize, check order, texts)
                  v
   chunking::service  --- source text (UTF-8 file, or document view of a converted doc)
        |   cut(text, spec) -> chunking::{fixed, structural}  (pure, tokens.rs ruler)
        v
   chunking::store (ChunkStore, volume meta tables chunk_sets, chunks)  <--- same txn as rename/delete in storage/meta.rs
        ^
        |  rebuild (scheduler, SPEC-0100): cut sets, then default set chunks -> SearchBackend::index_chunks_checked
   search::derived::Scheduler
```
Bounded contexts: Documents (`chunking/`, `tools/chunks.rs`, meta tables), Search (`search/*`), Derived data (scheduler, jobs). Existing structure cited: scheduler rebuild `search/derived/scheduler.rs:439`, `JobKind` `search/derived/mod.rs:21`, `recalculate` `tools/derived.rs:218`, tool pattern `mcp/server.rs:2318-2342`, meta schema `storage/meta.rs:231-330`, node rename `storage/meta.rs:1568`, document view `tools/artifacts.rs:307`.

### 3.1 Cutting
`chunking::cut(text, title, spec) -> Cut { used_strategy, fallback_from, chunks }`. `structural` follows paragraph v3a section 5 step by step: clean (NUL out, CRLF to LF), parse blocks (rules 1 to 7 of v3a 5.1, first match wins), build sections (5.2) with the cut decision on open headings, build units (5.3), merge (5.4, parent rule of FR-NEW-006: top level title equal), split above `max_tokens` (5.5, packing 5.8 with the head and tail counted, hard split 5.9), attach figures (5.6), number chunks and units (5.7). A text with no heading line (heading rule of FR-NEW-001, code blocks excluded) is cut by `fixed` with the server fixed defaults and reports `fallback_from: structural`. `fixed` encodes the whole text once with the ruler, slices windows of `size_tokens` every `size_tokens - overlap_tokens` tokens and decodes each slice (lossy), stopping when a chunk already reaches the end.
Complexity: one pass over lines for blocks and sections, one tokenization per block (memoized in the unit), packing is linear in elements; no regex backtracking across lines.

### 3.2 Rebuild
`Scheduler::rebuild` for kind `SearchIndex`: read the file; valid UTF-8: for the default set and every extra set cut again from the same text (default uses the server defaults of the moment), replace each set in one transaction keeping its `set_id`, then index the default set's chunks. Not valid UTF-8: a converted document keeps its sets (they read `out_of_date` by sha), any other file loses its sets and its entries. The guard of SPEC-0100 stays the last check before the sets and the entries are committed. `JobKind::Chunks` (manual only) cuts one set (default when no `set_id`) and, for the default set, re indexes entries when search is enabled and the project indexes.

## 4. Data model
Two tables of the volume meta store (`storage/meta.rs` schema, every row scoped by `volume_id`, DDL rendered per dialect):

`chunk_sets`: `volume_id` Text, `path` Text, `request_key` Text (canonical JSON of the requested strategy and settings), `set_id` Text, `strategy` Text (strategy used), `settings` Text (JSON of the settings used), `requested_strategy` Text, `requested_settings` Text (JSON), `is_default` BigInt default 0, `fallback_from` Text null, `chunk_count` BigInt, `token_count` BigInt, `source_sha` Text (the file's `nodes.sha256`, empty string when null), `built_at` Text (RFC 3339 UTC seconds). Primary key `(volume_id, path, request_key)` so two equal requests cannot coexist (FR-NEW-029); index `idx_chunk_sets_id (volume_id, set_id)`.
`chunks`: `volume_id`, `set_id`, `number` BigInt, `unit` BigInt, `text` Text, `heading_path` Text (JSON list), `page_start` BigInt null, `page_end` BigInt null, `token_count` BigInt, `figures` Text (JSON list), `document_title` Text. Primary key `(volume_id, set_id, number)`.
`derived_job` (admin store) gains a nullable column `set_id` Text (additive `ColumnMigration`), set only on kind `chunks` jobs.
Search engines: the stored text column of every engine (`chunk_text`) holds the indexed text (header line, newline, chunk text); `chunk_idx` holds the chunk number (1 based) for entries built from a set. No engine schema changes (an old Tantivy index opens as is).
Migration: tables are created by `CREATE TABLE IF NOT EXISTS`; rollback is dropping both tables (nothing else references them). `state` is not stored: it is `out_of_date` when `source_sha` differs from the file's current `sha256`.

## 5. Contracts and interfaces
### 5.1 Tools (all `fs.*`, 5 added: tool count 118 to 123, `fs.*` 47 to 52; contract files regenerate)
- `fs.create_chunk_set(mount_id, path, strategy, settings: object = {})` returns a set.
- `fs.list_chunk_sets(mount_id, path)` returns `{sets: [Set]}`.
- `fs.list_chunks(mount_id, path, set_id, marker = "")` returns `{chunks: [ChunkSummary], marker?}`, 100 per page.
- `fs.get_chunk(mount_id, path, set_id, number, before = 0, after = 0)` returns `{chunks: [Chunk]}`.
- `fs.delete_chunk_set(mount_id, path, set_id)` returns `{deleted: true}`.
- `fs.recalculate(mount_id, path, kind, set_id = "")`: kind `chunks` added (FR-NEW-019).
`Set` = `{set_id, strategy, settings, default, fallback_from?, chunk_count, token_count, state, built_at}`; `Chunk` = `{number, unit, heading_path, page_start, page_end, token_count, figures, document_title, text}`; `ChunkSummary` = `Chunk` with `preview` instead of `text`. Set ids are `set-` plus 8 hex of a uuid.
### 5.2 Exact refusals
Order for the chunk tools: project absent `ERR_PROJECT_NOT_FOUND`, non member `ERR_FORBIDDEN: '<person>' is not a member of '<mount_id>'`, path absent `ERR_NOT_FOUND: file not found: <path>`, folder `ERR_INVALID_ARGUMENT: path must be a file`, then the tool checks, with the texts of FR-NEW-017 verbatim (strategy, unknown setting, not applying, whole number, range, overlap, text, set, number, window, marker, 5 sets). `fs.delete_chunk_set` of the default: `ERR_INVALID_ARGUMENT: the default chunk set cannot be deleted`. `search.index` with `chunk_size` or `chunk_overlap`: `ERR_INVALID_ARGUMENT: chunk_size and chunk_overlap are no longer accepted`.
### 5.3 Marker
`fs.list_chunks` marker: base64 url safe without padding of the JSON `["<set_id>", <last number>]`; a different set id or any decoding failure is `ERR_INVALID_ARGUMENT: marker is not valid`.
### 5.4 Configuration
`search.chunking.fixed_size_tokens` (2048, 32..8192), `fixed_overlap_tokens` (0, 0..8191 and below the size), `heading_cut_depth` (2, 1..6), `max_tokens` (2048, 64..8192), `list_max_items` (40, 2..200); scalar text fields like `search.settle_delay_seconds`, empty or absent means the default, validated at boot whether or not search is enabled; text `ERR_INVALID_ARGUMENT: search.chunking.<key> must be a whole number between <min> and <max>`.
### 5.5 Internal signatures (Rust)
```rust
// chunking/mod.rs
pub enum Strategy { Fixed, Structural }
pub struct Settings { pub size_tokens: Option<u32>, pub overlap_tokens: Option<u32>, pub heading_cut_depth: Option<u32>, pub max_tokens: Option<u32>, pub list_max_items: Option<u32> }
pub struct Chunk { pub number: u32, pub unit: u32, pub text: String, pub heading_path: Vec<String>, pub page_start: Option<u32>, pub page_end: Option<u32>, pub token_count: u32, pub figures: Vec<String>, pub document_title: String }
pub fn cut(text: &str, title: &str, requested: &Requested, defaults: &Defaults) -> Cut;
// chunking/store.rs (volume meta)
pub struct ChunkStore;   // created by MetaBackend::chunk_store() (provided method, default None)
// search/mod.rs, trait SearchBackend (replaces index_path)
async fn index_chunks_checked(&self, volume_id: &str, path: &str, chunks: &[IndexedChunk], still_current: &(dyn Fn() -> bool + Send + Sync)) -> Result<Option<usize>>;
pub struct IndexedChunk { pub number: u32, pub text: String }   // text is the indexed text, header line first
```

## 6. File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/Cargo.toml`, `Cargo.toml`, `Cargo.lock` | modify: `tiktoken-rs = "0.12.1"` | FR-NEW-010 | US-0276 |
| `crates/core/src/config.rs`, `config/local.yaml.template`, `.agent_docs/config.md` | modify: `search.chunking.*` | FR-NEW-027 | US-0274 |
| `crates/core/src/chunking/{mod,model,tokens,fixed,structural,blocks,service,store}.rs` | create | FR-NEW-001..013, 020, 021 | US-0275..US-0280 |
| `crates/core/src/storage/meta.rs`, `storage/traits.rs`, `storage/mod.rs` | modify: tables, `chunk_store()`, removal of tables on project removal | FR-NEW-009, 014, 025 | US-0275, US-0281 |
| `crates/core/src/tools/chunks.rs`, `tools/mod.rs`, `mcp/server.rs` | create / modify: five tools | FR-NEW-013..018, 028, 029 | US-0275, US-0276 |
| `crates/core/src/search/{mod,bm25_sqlite,bm25_pg,vector_sqlite,vector_pg}.rs`, `search/chunker.rs` | modify; chunker deleted | FR-NEW-023, FR-MOD-001 | US-0279 |
| `crates/core/src/tools/search_semantic.rs`, `mcp/server.rs` | modify: `search.index`, `search.query` results | FR-NEW-023, 024 | US-0279 |
| `crates/core/src/search/derived/{mod,scheduler,store}.rs`, `storage/admin.rs`, `tools/derived.rs`, `app.rs` | modify: kind `chunks`, `set_id`, rebuild builds sets | FR-NEW-019, 021, 022, MOD-002, MOD-003 | US-0280, US-0282 |
| `crates/core/src/search/indexer.rs` | modify: `full_index`, hooks | FR-NEW-021, 026 | US-0280 |
| `crates/core/src/chunking/e2e.rs` | create (test module, all spec E2E) | all | US-0274..US-0282 |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, `crates/core/tests/fixtures/schema_*.sql`, count tests | regenerate / modify | FR-NEW-013..018 | US-0275, US-0276, US-0282 |
| `README.md`, `AGENTS.md`, `.agent_docs/{tools,search,architecture,testing,config}.md` | modify | all | US-0282 |
Affected existing tests: SPEC-0008 tests of character windows and `chunk_size` (modified by E2E-MOD-001, E2E-MOD-002), SPEC-0100 `e2e_0100_069` (E2E-MOD-003), the backend unit tests that call `index_path` (migrated to a test helper), tool count and schema golden tests.

## 7. Compatibility and risks
- Observable breaking change, declared in the spec: `search.index` refuses `chunk_size` and `chunk_overlap`; search results gain two fields; the entries of a file change at its first rebuild after the upgrade (FR-NEW-026).
- Risk 1, upgrade: nothing converts at start, so old character entries and new chunk entries never share a file. Result enrichment treats a file with no default set as pre upgrade (`chunk_number` 0).
- Risk 2, tokenizer cost: the vocabulary is parsed at first use (about a second, hundreds of milliseconds after); a `OnceLock` singleton and `spawn_blocking` for cuts keep the request thread free.
- Risk 3, large documents: a set of thousands of chunks is one transaction; chunk text is stored once per set, up to 5 sets per file, not counted in the quota (spec 3.2).
- Risk 4, the guard and the transaction: the guard is checked immediately before the set transaction and again before the engine commit (SPEC-0100 pattern); a change landing between them is repaired by the next rebuild.
- New dependency: `tiktoken-rs` (MIT). Rollback: revert the branch; the tables are additive.

## 9. Test strategy
Every spec test is a functional e2e driven through the real `McpServer` handlers (`fs_write`, `fs_create_chunk_set`, `fs_list_chunk_sets`, `fs_list_chunks`, `fs_get_chunk`, `fs_delete_chunk_set`, `fs_recalculate`, `search_index`, `search_query`) on a fixture with an in memory admin store, a local volume, a stub embedding engine and a paused clock, reusing `search/derived_e2e.rs` helpers (`Env`, `Stub`). Texts of a stated token size come from a helper that builds prose to an exact `cl100k_base` count. All in `crates/core/src/chunking/e2e.rs`, named `e2e_0101_<id>_<slug>`; run with `cargo test -p fsa-core --lib -- e2e_0101_`. Pure cutting rules are also covered by unit tests in `chunking/*.rs` (story local). The table below is the map.

| Test | Level | Driver | Location | Story |
|---|---|---|---|---|
| E2E-131 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0274 |
| E2E-132 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0274 |
| E2E-145 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0274 |
| E2E-010 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-011 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-012 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-013 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-014 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-015 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-016 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-017 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-018 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-019 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-020 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-022 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-023 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0275 |
| E2E-021 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-030 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-031 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-033 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-034 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-035 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-036 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-037 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-038 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-039 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-040 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-041 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-042 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-044 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-045 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-051 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-139 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0276 |
| E2E-043 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-050 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-062 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-063 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-070 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-071 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-072 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-073 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-074 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-075 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-076 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-077 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-084 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-087 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-088 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-089 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-090 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-093 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-095 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0277 |
| E2E-078 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-079 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-080 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-081 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-082 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-083 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-085 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-086 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-091 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-092 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-094 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0278 |
| E2E-120 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-121 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-122 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-123 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-124 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-125 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-126 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-142 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-143 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-144 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-MOD-001 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-MOD-002 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0279 |
| E2E-001 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-002 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-003 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-004 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-005 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-006 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-007 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-032 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-053 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-060 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-061 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-116 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-117 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-130 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-133 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-134 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-135 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-136 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-140 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-141 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0280 |
| E2E-100 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0281 |
| E2E-101 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0281 |
| E2E-102 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0281 |
| E2E-103 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0281 |
| E2E-104 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0281 |
| E2E-105 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0281 |
| E2E-146 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0281 |
| E2E-110 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |
| E2E-111 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |
| E2E-112 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |
| E2E-113 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |
| E2E-114 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |
| E2E-115 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |
| E2E-138 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |
| E2E-052 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |
| E2E-MOD-003 | functional e2e | `McpServer` handler, clock paused | `chunking/e2e.rs` | US-0282 |

Story local unit tests (cutting rules, ruler, store) live in the touched `chunking/*.rs` and `search/*.rs` modules. Test data: generated (texts built to an exact token count, stub engine); nothing user provided.

## 10. Decisions log
- **DEC-001:** Add the dependency `tiktoken-rs` 0.12.1 for the `cl100k_base` ruler; the ranks are embedded in the crate, so no file or network is needed at runtime. **Obliged by:** SPEC-0101/FR-NEW-010 (sizes in `cl100k_base` tokens, offline). **Rationale:** the owner imposes the ruler (spec 3.3); this crate ships it. **Alternatives considered:** `tokenizers` (needs a `tokenizer.json` file at runtime), hand written BPE (months of risk), a character based estimate (violates the ruler). **Source:** design; crates.io API 2026-11-21, README. **Evidence:** tiktoken-rs 0.12.1 MIT, `max_stable_version` 0.12.1; context7 had no quota, ddg and crates.io used as fallback. A new third party dependency is category 2 of the questions: it is reported under Needs you, with this choice as the default.
- **DEC-002:** A `chunking` module of pure functions: `tokens` (ruler singleton, count, encode, lossy decode), `fixed`, `blocks` plus `structural` (paragraph v3a), `model`, `service` (sources, build, replace), `store`. **Obliged by:** SPEC-0101/FR-NEW-001 to FR-NEW-012. **Rationale:** determinism and testability without a server. **Alternatives considered:** put cutting in `search/` (couples it to engines). **Source:** design. **Evidence:** `search/chunker.rs:11` is the code replaced.
- **DEC-003:** `structural` implements paragraph v3a 5.1 to 5.9 literally, except the merge parent rule of FR-NEW-006 (top level titles equal). The block parser is a single pass with the first matching rule. Packing counts head and tail (header rows, fences). **Obliged by:** SPEC-0101/FR-NEW-001 to FR-NEW-008, FR-NEW-010. **Rationale:** the owner wants chunks close to v3a. **Alternatives considered:** a Markdown crate (does not give v3a block rules; a different parse). **Source:** spec constraint. **Evidence:** `/Users/sebastien/projects/perso/search-doc-corpora/specs/paragraph-v3a.md`.
- **DEC-004:** Chunk sets and chunks are rows of two tables of the per volume meta store (`chunk_sets`, `chunks`), created beside `doc_artifact_*`; `state` is computed from `source_sha` against `nodes.sha256`. **Obliged by:** SPEC-0101/FR-NEW-009, FR-NEW-014, FR-NEW-022. **Rationale:** metadata next to the document, volume scoped, no extra service; a sha compare is stable across moves (`rev` changes on a move, `sha256` does not). **Alternatives considered:** the admin database, the search index, a stored `out_of_date` flag. **Source:** existing code. **Evidence:** `storage/meta.rs:231-330` (artifact tables), `storage/meta.rs:1607` (rev on rename).
- **DEC-005:** `request_key` (canonical JSON of the requested strategy and settings with server defaults filled in) is part of the primary key `(volume_id, path, request_key)`; creating an equal request returns the existing row; the default set is the row with `is_default` 1 and its key follows the server defaults at each rebuild (a hand made set with the same new key is absorbed into the default). **Obliged by:** SPEC-0101/FR-NEW-029, FR-NEW-013. **Rationale:** the primary key makes two members creating the same set at once safe on both engines (the loser reads the winner). **Alternatives considered:** unique index (the schema layer has none), a lock. **Source:** design. **Evidence:** `storage/rel/schema.rs` (`Index` is non unique).
- **DEC-006:** At most 5 sets per file is counted inside the creating transaction after the equality check. **Obliged by:** SPEC-0101/FR-NEW-017. **Rationale:** last check, only for a new request, as the spec orders. **Alternatives considered:** check before building (wrong order). **Source:** spec. **Evidence:** n/a.
- **DEC-007:** Sets follow their file in the same transaction as the node rows: `tx_rename` rewrites the path prefix of `chunk_sets` for a move; a delete, `rename_into_trash` and `remove_subtree` delete the sets of the path or subtree; a copy copies nothing. **Obliged by:** SPEC-0101/FR-NEW-025. **Rationale:** works in every index mode, which the search hooks (gated by the mode) cannot do. **Alternatives considered:** hooks in `search/indexer.rs` (skipped when indexing is off). **Source:** existing code. **Evidence:** `storage/meta.rs:1568-1620`, `2073-2260`; `search/indexer.rs:193` (the gate).
- **DEC-008:** Text source: a file whose bytes are valid UTF-8 is cut as is; otherwise a file with a current artifact set is cut from the text `tools::artifacts::document_view` builds (captions true, tables as Markdown); otherwise `ERR_NOT_SUPPORTED: no text to cut for <path>`. **Obliged by:** SPEC-0101/FR-NEW-020. **Rationale:** reuse of the one document text the product already composes. **Alternatives considered:** the raw conversion text without captions (misses image text). **Source:** existing code. **Evidence:** `tools/artifacts.rs:307`, `docs/artifacts/render.rs:107`.
- **DEC-009:** `SearchBackend::index_chunks_checked(volume, path, &[IndexedChunk], guard)` replaces `index_path`; each engine replaces a path's entries as before (guard, one transaction or one commit), storing the indexed text in its text column and the chunk number in `chunk_idx`; the character chunker and `AUTO_CHUNK_*` are deleted. **Obliged by:** SPEC-0101/FR-NEW-023, FR-MOD-001. **Rationale:** search must not cut files; no engine schema change keeps old Tantivy indexes openable. **Alternatives considered:** keep `index_path` with a tokens meaning (two cutters), new columns (reindex needed). **Source:** existing code. **Evidence:** `search/mod.rs:62-100`, `search/bm25_sqlite.rs:125`, `search/chunker.rs:11`.
- **DEC-010:** Indexed text is `<title>[ > <heading path titles joined by " > ">]\n<chunk text>`. A search result's text is the text after the first newline for an entry of a file that has a default set; its `chunk_number` is `chunk_idx` and its `heading_path` is read from the default set's chunk of that number (one lookup per distinct path); a file with no default set reads `chunk_number` 0 and an empty `heading_path`, text unchanged. **Obliged by:** SPEC-0101/FR-NEW-023, FR-NEW-026. **Rationale:** engines store no heading path, the metadata holds it. **Alternatives considered:** a header marker column per engine. **Source:** design. **Evidence:** `tools/search_semantic.rs:177-185` (result mapping).
- **DEC-011:** `search.index` builds the default set when missing or `out_of_date`, then indexes it through the same code as the rebuild; `chunk_size` and `chunk_overlap` stay in the argument struct only to be refused with the spec text (hidden from the schema). **Obliged by:** SPEC-0101/FR-NEW-024. **Rationale:** one indexing path. **Alternatives considered:** deleting the fields (serde would answer an unknown field error text). **Source:** existing code. **Evidence:** `mcp/server.rs:1524-1528`.
- **DEC-012:** The rebuild (`JobKind::SearchIndex`) cuts the sets first; a new `JobKind::Chunks` (manual only, key per file and kind, `set_id` column on `derived_job`) cuts one set. Both reuse the guard, supersede and stale rules of SPEC-0100. **Obliged by:** SPEC-0101/FR-NEW-019, FR-NEW-021, FR-NEW-022, FR-MOD-002, FR-MOD-003. **Rationale:** the scheduler already carries kind in its key. **Alternatives considered:** a second scheduler. **Source:** existing code. **Evidence:** `search/derived/mod.rs:21`, `search/derived/scheduler.rs:182,439`.
- **DEC-013:** Set replacement is one transaction (delete chunks, insert chunks, update the set row) keeping `set_id`; a failed or discarded rebuild never reaches it. **Obliged by:** SPEC-0101/FR-NEW-022. **Rationale:** readers see all old or all new chunks. **Alternatives considered:** write new set then swap id (breaks the kept id rule). **Source:** design. **Evidence:** n/a.
- **DEC-014:** Config keys `search.chunking.*` are text fields with the scalar deserializer and boot validation of `search.settle_delay_seconds`; the defaults struct is passed to the scheduler and the tools. **Obliged by:** SPEC-0101/FR-NEW-027. **Rationale:** same operator behavior. **Alternatives considered:** numeric fields (serde error text). **Source:** existing code. **Evidence:** `config.rs:862-890`, `config.rs:1213`.
- **DEC-015:** The five tools live in `tools/chunks.rs` and call one engine each; the MCP surface only. No REST twin. **Obliged by:** SPEC-0101/FR-NEW-013 to FR-NEW-018. **Rationale:** the spec names tools; the screen (BL-0175) will add REST. **Alternatives considered:** REST now (not asked). **Source:** spec. **Evidence:** `mcp/server.rs:2318-2342` (pattern).
- **DEC-016:** Cuts run in `spawn_blocking`; the ruler is a `OnceLock` singleton. **Obliged by:** SPEC-0101/FR-NEW-010 (bounded linear cost), NFR 7.1. **Rationale:** a 30 000 token text must not block the runtime. **Alternatives considered:** inline cut. **Source:** design. **Evidence:** n/a.
- **DEC-017:** A set made by hand keeps its requested strategy and settings and is rebuilt with them; `structural` requested on a text with no heading is stored as `fixed` with `fallback_from` and rebuilds as `structural` once a heading exists. **Obliged by:** SPEC-0101/FR-NEW-012. **Rationale:** the spec says so; requested values are stored beside used values. **Alternatives considered:** store used only (loses the request). **Source:** spec. **Evidence:** n/a.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0274 | server defaults for the chunking settings | P1 | FR-NEW-027 | main | 2 | no |
| US-0275 | chunk set store and the read tools | P1 | FR-NEW-009, FR-NEW-014, FR-NEW-015, FR-NEW-016, FR-NEW-017, FR-NEW-028 | main | 1 | no |
| US-0276 | creating and deleting sets, and the fixed strategy | P1 | FR-NEW-010, FR-NEW-011, FR-NEW-013, FR-NEW-017, FR-NEW-018, FR-NEW-020, FR-NEW-028, FR-NEW-029 | US-0274, US-0275 | 1 | no |
| US-0277 | the structural strategy: sections, units, merging | P1 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-005, FR-NEW-006, FR-NEW-010, FR-NEW-012, FR-NEW-017, FR-NEW-009 | US-0276 | 1 | no |
| US-0278 | the structural strategy: oversize pieces and list batches | P1 | FR-NEW-007, FR-NEW-008, FR-NEW-010 | US-0277 | 1 | no |
| US-0279 | search builds its entries from the default set | P1 | FR-NEW-023, FR-NEW-024, FR-MOD-001, FR-NEW-026 | US-0278 | 1 | no |
| US-0280 | the rebuild builds the chunk sets before the search entries | P1 | FR-NEW-021, FR-NEW-022, FR-NEW-026, FR-NEW-012, FR-NEW-014, FR-NEW-027, FR-MOD-002 | US-0274, US-0279 | 1 | no |
| US-0281 | chunk sets follow moves, copies and deletions | P2 | FR-NEW-025 | US-0280 | 2 | no |
| US-0282 | recalculate chunks by hand, docs and contract | P1 | FR-NEW-019, FR-MOD-003, FR-NEW-017 | US-0280, US-0281 | 2 | yes |
