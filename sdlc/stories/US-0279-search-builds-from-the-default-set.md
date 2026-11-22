# US-0279: search builds its entries from the default set

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0278
> min_tier: 1
> Files touched: 9
> Closes spec: no

## Objective
Make the search engines index chunks instead of cutting text: `index_chunks_checked` replaces `index_path`, the character cutter is deleted, entries carry the header line and the chunk number, `search.index` builds or refreshes the default set and indexes it (its two size arguments are refused), and search results carry `chunk_number` and `heading_path`. A search finds a file through its structural chunks.

## Requirements covered
- SPEC-0101/FR-NEW-023: The system SHALL build the search entries of a file from the chunks of its default set, one entry per chunk, with the text `<document title> > <heading path titles joined by " > ">` on its own line before the chunk text, and SHALL NOT cut files itself.
- SPEC-0101/FR-NEW-024: WHEN a member calls `search.index` for a file THE system SHALL index the default set of that file, building it first when the file has none, and answer with the number of chunks indexed.
- SPEC-0101/FR-MOD-001: WHEN the search entries of a file are built THE system SHALL cut them as the default set of the file says, in tokens, and SHALL NOT use character windows.
- SPEC-0101/FR-NEW-026 (partial: a result of an entry built before the upgrade): The system SHALL leave the search entries of a file as they were until the first rebuild of that file after the upgrade creates its default set.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_12 e2e_0101_14 e2e_0101_MOD search:: tools::search_semantic`. For example: `/guide.md` with `# Setup` and body text, call `search.index`, query a word that is only in the heading and get the chunk with `chunk_number` 1 and `heading_path` [`Setup`]; call it with `chunk_size` 500 and get the spec text.

## Design excerpt
### Decisions
- DEC-009 (SPEC-0101): `SearchBackend::index_chunks_checked(volume, path, &[IndexedChunk], guard)` replaces `index_path`; each engine replaces a path's entries as before (guard, one transaction or one commit), storing the indexed text in its text column and the chunk number in `chunk_idx`; the character chunker and `AUTO_CHUNK_*` are deleted. **Obliged by:** SPEC-0101/FR-NEW-023, FR-MOD-001. **Rationale:** search must not cut files; no engine schema change keeps old Tantivy indexes openable. **Alternatives considered:** keep `index_path` with a tokens meaning (two cutters), new columns (reindex needed). **Source:** existing code. **Evidence:** `search/mod.rs:62-100`, `search/bm25_sqlite.rs:125`, `search/chunker.rs:11`.
- DEC-010 (SPEC-0101): Indexed text is `<title>[ > <heading path titles joined by " > ">]\n<chunk text>`. A search result's text is the text after the first newline for an entry of a file that has a default set; its `chunk_number` is `chunk_idx` and its `heading_path` is read from the default set's chunk of that number (one lookup per distinct path); a file with no default set reads `chunk_number` 0 and an empty `heading_path`, text unchanged. **Obliged by:** SPEC-0101/FR-NEW-023, FR-NEW-026. **Rationale:** engines store no heading path, the metadata holds it. **Alternatives considered:** a header marker column per engine. **Source:** design. **Evidence:** `tools/search_semantic.rs:177-185` (result mapping).
- DEC-011 (SPEC-0101): `search.index` builds the default set when missing or `out_of_date`, then indexes it through the same code as the rebuild; `chunk_size` and `chunk_overlap` stay in the argument struct only to be refused with the spec text (hidden from the schema). **Obliged by:** SPEC-0101/FR-NEW-024. **Rationale:** one indexing path. **Alternatives considered:** deleting the fields (serde would answer an unknown field error text). **Source:** existing code. **Evidence:** `mcp/server.rs:1524-1528`.
### Data model
No schema change: the text column holds the indexed text, `chunk_idx` the chunk number (1 based).
### Contracts
Trait: `SearchBackend::index_chunks_checked(volume_id, path, &[IndexedChunk{number, text}], still_current) -> Result<Option<usize>>` replaces `index_path` and `index_path_checked` in every engine and in `CombinedBackend` (vector first, then BM25); `IndexedChunk.text` is `<title>[ > <heading titles joined by " > ">]\n<chunk text>`. `search.index(mount_id, path, recursive)`: for each text file build the default set when missing or `out_of_date` (`chunking::service::ensure_default_set`), then index it; a single non text path `ERR_NOT_SUPPORTED: no text to cut for <path>`; `chunk_size` or `chunk_overlap` passed `ERR_INVALID_ARGUMENT: chunk_size and chunk_overlap are no longer accepted` (fields kept in the args struct with `#[schemars(skip)]`). `search.query` result gains `chunk_number` and `heading_path`; text is the part after the first newline when the file has a default set; a file with no default set gives `chunk_number` 0, empty `heading_path`, text unchanged.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/search/mod.rs`, `bm25_sqlite.rs`, `bm25_pg.rs`, `vector_sqlite.rs`, `vector_pg.rs` | modify: trait method and engines; unit tests migrated to a test helper that cuts with the default structural settings |
| `crates/core/src/search/chunker.rs` | delete |
| `crates/core/src/search/indexer.rs`, `search/derived/scheduler.rs` | modify: `full_index` and the rebuild call `ensure_default_set` and `index_chunks_checked`; `AUTO_CHUNK_*` removed |
| `crates/core/src/tools/search_semantic.rs`, `mcp/server.rs` | modify: `search.index`, `search.query` |
| `crates/core/src/chunking/service.rs` | modify: `ensure_default_set` |
| `crates/core/src/chunking/e2e.rs` | modify: these tests |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json` | regenerate (the `search.index` schema loses two arguments) |
### Existing patterns
Guarded commit per engine: the shape of `index_path_checked` in each engine (SPEC-0100 US-0264 and US-0265: guard after embedding, one transaction or one commit); engine unit tests today call `index_path` (about 60 call sites in `search/*.rs`); results mapping `tools/search_semantic.rs:177-185`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-120: search results carry the chunk and its heading path
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_120_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` with `# Setup`, text with the word `kubelet`, `# Usage`, text; its default set is current
- **When** Alice searches `kubelet`
- **Then** the first result is `/guide.md` with `chunk_number` 1 and `heading_path` [`Setup`], and the existing fields
- **And** a search for a word only in the title (`guide`) also finds the chunk
- **Priority:** Critical

### SPEC-0101/E2E-121: the heading is part of the indexed text
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_121_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` with `# Setup` and text that never uses the word `setup` in its body
- **When** Alice searches `setup`
- **Then** the chunk is found
- **And** the text shown in the result is the chunk text without the header line
- **Priority:** High

### SPEC-0101/E2E-122: indexing by hand uses the default set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_122_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has no set yet
- **When** Alice calls `search.index` for `/guide.md`
- **Then** the answer reports the number of chunks of the default set, which now exists
- **And** the search finds the file
- **Priority:** High

### SPEC-0101/E2E-123: the removed arguments
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_123_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md`
- **When** Alice calls `search.index` with `chunk_size` 500
- **Then** the answer is `ERR_INVALID_ARGUMENT: chunk_size and chunk_overlap are no longer accepted`
- **And** nothing was indexed
- **Priority:** High

### SPEC-0101/E2E-124: indexing a folder by hand
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_124_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/docs` holds `a.md`, `b.txt` and `c.png`
- **When** Alice calls `search.index` on `/docs` with `recursive` true
- **Then** `a.md` and `b.txt` get their default sets and entries; `c.png` is skipped with no error
- **And** the answer counts the chunks of the two sets
- **Priority:** Medium

### SPEC-0101/E2E-125: a chunk without heading path
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_125_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/plain.md` with no heading is a fallback set
- **When** Alice searches a word of the text
- **Then** the result's `heading_path` is empty
- **And** the indexed header line is the document title alone
- **Priority:** Medium

### SPEC-0101/E2E-126: embedding down for indexing by hand
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_126_*`)
- **Given** project `proj` indexing its files (mode `both`), Alice a member; `/guide.md` is indexed; the embedding service is down
- **When** Alice calls `search.index` for `/guide.md`
- **Then** the answer is an error `embedding service unreachable`
- **And** the previous entries and the previous default set are unchanged
- **Priority:** High

### SPEC-0101/E2E-142: search.index on a stale default set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_142_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a default set `out_of_date` (saved 1 s ago, settle delay 30 s)
- **When** Alice calls `search.index` for `/guide.md`
- **Then** the default set is rebuilt first and becomes `current`; the answer counts its chunks
- **And** the search finds the new text
- **Priority:** Medium

### SPEC-0101/E2E-143: search.index on a binary file
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_143_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/logo.png` is a PNG image
- **When** Alice calls `search.index` for `/logo.png`
- **Then** the answer is `ERR_NOT_SUPPORTED: no text to cut for /logo.png`
- **And** nothing was indexed
- **Priority:** High

### SPEC-0101/E2E-144: a result from an entry built before the upgrade
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_144_*`)
- **Given** `/old.md` indexed before the upgrade with character chunks
- **When** Alice searches a word of it before its rebuild
- **Then** the result carries `chunk_number` 0 and an empty `heading_path`
- **And** after its rebuild the result carries the real chunk number
- **Priority:** Medium

### SPEC-0101/E2E-MOD-001: status counts chunks, not files (was SPEC-0008/E2E-004)
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_mod001_*`)
- **Validated before:** `/long.md` (5000 characters) and `/doc1.md` (25 characters) indexed with `chunk_size` 1000 and `chunk_overlap` 100: `search.status` reports more than 2 documents.
- **Validates now:** both files are indexed through their default sets (no arguments): `/long.md` holds 1000 tokens in one section, so it is one chunk and `bm25_docs` is 2, equal to the total of `chunk_count`; with `/long.md` holding 3 sections of 1500 tokens under `# ` headings, `bm25_docs` is 4 (3 plus 1).
- **Given** project in mode `bm25`, two files as above
- **When** Alice indexes them with `search.index` and reads `search.status`
- **Then** `bm25_docs` equals the total of `chunk_count` of the default sets, and `mode` reads `bm25`
- **Priority:** High

### SPEC-0101/E2E-MOD-002: re-indexing is idempotent (was SPEC-0008/E2E-011)
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_mod002_*`)
- **Validated before:** indexing twice with the same arguments reports the same inserted count and the same status.
- **Validates now:** indexing twice with no arguments reports the same number of chunks, the default set keeps the same id and chunks, and a query returns each chunk once.
- **Given** `/doc1.md` indexed once
- **When** Alice indexes it again
- **Then** the reported chunk count is equal, `search.status` is unchanged, a query returns each chunk once with no duplicate path and chunk number
- **Priority:** High

## Constraints
- Do not change fusion, ranking or the number of results (BL-0172).
- No engine schema change.
- The scheduler keeps its guard rules.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/search.md` (indexed text, chunk numbers) and `.agent_docs/tools.md` (`search.index` arguments).
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.

- [ ] One commit `US-0279: search builds its entries from the default set` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
