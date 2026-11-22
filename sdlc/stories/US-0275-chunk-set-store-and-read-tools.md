# US-0275: chunk set store and the read tools

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 1
> Files touched: 9
> Closes spec: no

## Objective
Create the persistence of chunk sets (two tables in the volume meta store, the `ChunkStore`) and the three read tools `fs.list_chunk_sets`, `fs.list_chunks`, `fs.get_chunk`, with paging, previews, neighbour windows, the computed `state` and every refusal. Nothing builds sets yet: tests seed rows through the store. A member can already ask what a file holds and get an honest empty answer.

## Requirements covered
- SPEC-0101/FR-NEW-009 (partial: the fields of a chunk in answers): The system SHALL give each chunk its text, the title of its document, its heading path, its number in the set (1, 2, 3 and so on), its unit number, its first and last page, its token count and its figure references.
- SPEC-0101/FR-NEW-014: WHEN a member of a project asks for the chunk sets of a file THE system SHALL list each set with its id, strategy, settings, whether it is the default set, `fallback_from` when it applies, its number of chunks, its token count, its state and the time it was built.
- SPEC-0101/FR-NEW-015: WHEN a member of a project asks for the chunks of a set THE system SHALL list them by number with, for each, its number, unit number, heading path, pages, token count, figure references and a preview of its first 200 characters.
- SPEC-0101/FR-NEW-016: WHEN a member of a project asks for chunk N of a set with `before` and `after` THE system SHALL answer with the full chunk N, the `before` chunks that precede it and the `after` chunks that follow it, in order of number.
- SPEC-0101/FR-NEW-017 (partial: the read tool refusals and their order): IF a request to a chunk tool is not allowed or not valid THEN THE system SHALL refuse it, change nothing, and answer with the exact text of the rule that failed.
- SPEC-0101/FR-NEW-028 (partial: read tools): The system SHALL allow the chunk tools to the members of the project of the file only.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_01 e2e_0101_02 chunking::`. Tests seed sets of 20 and 250 chunks through `ChunkStore`, call the three handlers as a member and as a non member and assert the answers; a store unit test replaces a set keeping its id.

## Design excerpt
### Decisions
- DEC-004 (SPEC-0101): Chunk sets and chunks are rows of two tables of the per volume meta store (`chunk_sets`, `chunks`), created beside `doc_artifact_*`; `state` is computed from `source_sha` against `nodes.sha256`. **Obliged by:** SPEC-0101/FR-NEW-009, FR-NEW-014, FR-NEW-022. **Rationale:** metadata next to the document, volume scoped, no extra service; a sha compare is stable across moves (`rev` changes on a move, `sha256` does not). **Alternatives considered:** the admin database, the search index, a stored `out_of_date` flag. **Source:** existing code. **Evidence:** `storage/meta.rs:231-330` (artifact tables), `storage/meta.rs:1607` (rev on rename).
- DEC-005 (SPEC-0101): `request_key` (canonical JSON of the requested strategy and settings with server defaults filled in) is part of the primary key `(volume_id, path, request_key)`; creating an equal request returns the existing row; the default set is the row with `is_default` 1 and its key follows the server defaults at each rebuild (a hand made set with the same new key is absorbed into the default). **Obliged by:** SPEC-0101/FR-NEW-029, FR-NEW-013. **Rationale:** the primary key makes two members creating the same set at once safe on both engines (the loser reads the winner). **Alternatives considered:** unique index (the schema layer has none), a lock. **Source:** design. **Evidence:** `storage/rel/schema.rs` (`Index` is non unique).
- DEC-013 (SPEC-0101): Set replacement is one transaction (delete chunks, insert chunks, update the set row) keeping `set_id`; a failed or discarded rebuild never reaches it. **Obliged by:** SPEC-0101/FR-NEW-022. **Rationale:** readers see all old or all new chunks. **Alternatives considered:** write new set then swap id (breaks the kept id rule). **Source:** design. **Evidence:** n/a.
- DEC-015 (SPEC-0101): The five tools live in `tools/chunks.rs` and call one engine each; the MCP surface only. No REST twin. **Obliged by:** SPEC-0101/FR-NEW-013 to FR-NEW-018. **Rationale:** the spec names tools; the screen (BL-0175) will add REST. **Alternatives considered:** REST now (not asked). **Source:** spec. **Evidence:** `mcp/server.rs:2318-2342` (pattern).
### Data model
Tables `chunk_sets` and `chunks` of the volume meta store exactly as design section 4 (primary keys `(volume_id, path, request_key)` and `(volume_id, set_id, number)`, index `idx_chunk_sets_id`). `state` is computed: `out_of_date` when `source_sha` differs from the file `nodes.sha256` (empty string for null).
### Contracts
Tools `fs.list_chunk_sets(mount_id, path)`, `fs.list_chunks(mount_id, path, set_id, marker = "")`, `fs.get_chunk(mount_id, path, set_id, number, before = 0, after = 0)`; shapes of design 5.1; order of the sets: default first, then `built_at` oldest first, then `set_id`; list page 100 by `number`, marker design 5.3, preview first 200 characters; window `before` and `after` 0 to 20, neighbours beyond the ends are absent. Refusal order and texts: project, member, path absent `ERR_NOT_FOUND: file not found: <path>`, folder `ERR_INVALID_ARGUMENT: path must be a file`, set `ERR_NOT_FOUND: chunk set not found: <set_id>`, chunk `ERR_NOT_FOUND: chunk <number> not found in set <set_id>`, window `ERR_INVALID_ARGUMENT: before and after must be between 0 and 20`, marker `ERR_INVALID_ARGUMENT: marker is not valid`. Rust: `MetaBackend::chunk_store(&self) -> Option<ChunkStore>` (provided, default `None`), `ChunkStore::{list_sets, get_set, list_chunks, get_chunk_window, insert_set, replace_set}`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/meta.rs` | modify: the two tables in the schema; `chunk_store()` override |
| `crates/core/src/storage/traits.rs` | modify: provided method `chunk_store()` |
| `crates/core/src/chunking/store.rs`, `chunking/model.rs` | create: `ChunkStore`, `Set`, `Chunk`, marker codec |
| `crates/core/src/tools/chunks.rs`, `tools/mod.rs` | create / modify: engines for the three tools |
| `crates/core/src/mcp/server.rs` | modify: args structs and three `#[tool]` methods |
| `crates/core/src/chunking/e2e.rs` | modify: fixture helpers (seed sets) and these tests |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, `crates/core/tests/fixtures/schema_sqlite.sql`, `schema_postgres.sql` | regenerate: `MCPFS_REWRITE_TOOL_CONTRACT=1 cargo test -p fsa-core --lib tool_contract_golden_is_current`, `MCPFS_REWRITE_SCHEMA_GOLDEN=1 cargo test -p fsa-core --test schema_golden`; fix every test asserting the tool counts (118 to 121) and `AGENTS.md` and `.agent_docs/tools.md` counts and rows, as US-0266 of SPEC-0100 did |
### Existing patterns
Tool and store pattern of SPEC-0100: `tools/derived.rs` (list engine, marker, `authorize_only`) and `search/derived/store.rs`; table declarations `storage/meta.rs:231-330`; paging `storage/meta.rs:1134`; provided trait method returning an optional store `AdminBackend::derived_jobs()` in `storage/traits.rs`; fixture `search/derived_e2e.rs` (`Env`, seeding helpers).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-010: listing sets and paging through chunks
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_010_*`)
- **Given** project `proj` indexing its files, the server configuration has `search.chunking.max_tokens` 64; `/big.md` has a default set of 250 chunks
- **When** Alice lists the chunks of the set, then asks for the next page with the returned marker, then once more
- **Then** the first answer holds chunks 1 to 100 and a marker, the second 101 to 200 and a marker, the third 201 to 250 and no marker; each chunk shows number, unit, heading path, pages, token count, figures and a preview
- **And** the first answer arrives in under 2 s
- **Priority:** Critical

### SPEC-0101/E2E-011: a chunk with its neighbours
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_011_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 5 of the set with `before` 2 and `after` 1
- **Then** the answer holds chunks 3, 4, 5 and 6 in that order, each with its full text, document title `big`, heading path, unit, pages, token count and figures
- **And** chunk 5 text equals the text of chunk 5 seen alone
- **Priority:** High

### SPEC-0101/E2E-012: a window beyond the ends
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_012_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 1 with `before` 3, then chunk 20 with `after` 2
- **Then** the first answer holds chunk 1 alone, the second chunk 20 alone; no error
- **And** both answers are in order of number
- **Priority:** High

### SPEC-0101/E2E-013: widening a window by asking again
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_013_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 10 with `before` 1 and `after` 1, then chunk 8 with `before` 2 and `after` 0
- **Then** the first answer holds chunks 9 to 11 and the second chunks 6 to 8
- **And** the union of the two answers is chunks 6 to 11 with no chunk changed
- **Priority:** Medium

### SPEC-0101/E2E-014: the preview of a short and of a long chunk
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_014_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; a set whose chunk 1 holds 50 characters and chunk 2 holds 3000 characters
- **When** Alice lists the chunks
- **Then** the preview of chunk 1 is its whole text; the preview of chunk 2 is its first 200 characters
- **And** no listed chunk carries a full text field
- **Priority:** Medium

### SPEC-0101/E2E-015: an unknown set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_015_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/big.md` has a default set
- **When** Alice lists the chunks of the set `set-999`
- **Then** the answer is `ERR_NOT_FOUND: chunk set not found: set-999`
- **And** nothing is changed
- **Priority:** High

### SPEC-0101/E2E-016: a chunk number that does not exist
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_016_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 99 of the set, and chunk 0
- **Then** each answer is `ERR_NOT_FOUND: chunk 99 not found in set <id>` and `ERR_NOT_FOUND: chunk 0 not found in set <id>`
- **And** nothing is changed
- **Priority:** High

### SPEC-0101/E2E-017: a window above 20
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_017_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 5 with `before` 21, then with `after` -1
- **Then** each answer is `ERR_INVALID_ARGUMENT: before and after must be between 0 and 20`
- **And** a request with `before` 20 and `after` 20 is accepted
- **Priority:** High

### SPEC-0101/E2E-018: an unusable marker
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_018_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 250 chunks
- **When** Alice lists the chunks with the marker `zzz`
- **Then** the answer is `ERR_INVALID_ARGUMENT: marker is not valid`
- **And** a marker from another set is refused the same way
- **Priority:** High

### SPEC-0101/E2E-019: a file with no set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_019_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/new.md` was just created and its rebuild has not run
- **When** Alice lists its sets
- **Then** the answer is `{sets: []}`
- **And** no error and no failed job
- **Priority:** High

### SPEC-0101/E2E-020: order of the sets of a file
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_020_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/big.md` has its default set built at 10:00:00 and two extra sets built at 10:05:00 and 10:02:00
- **When** Alice lists the sets
- **Then** the order is the default set, the set of 10:02:00, the set of 10:05:00
- **And** ties of the time are ordered by set id
- **Priority:** Medium

### SPEC-0101/E2E-022: a project that does not exist
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_022_*`)
- **Given** no project `ghost`
- **When** Alice lists the sets of `/a.md` in `ghost`
- **Then** the answer is `ERR_PROJECT_NOT_FOUND`
- **And** nothing is changed
- **Priority:** High

### SPEC-0101/E2E-023: an unknown path and a folder
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_023_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/docs` is a folder, `/nope.md` does not exist
- **When** Alice lists the sets of `/nope.md` and of `/docs`
- **Then** the answers are `ERR_NOT_FOUND: file not found: /nope.md` and `ERR_INVALID_ARGUMENT: path must be a file`
- **And** the order of the checks is path absent, then folder
- **Priority:** High

## Constraints
- No tool creates or deletes sets yet.
- No new dependency.
- The schema golden fixtures and the tool counts change in this story; later stories change them again.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/tools.md` rows for the three tools (counts in `AGENTS.md` follow the contract test).
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.

- [ ] One commit `US-0275: chunk set store and the read tools` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
