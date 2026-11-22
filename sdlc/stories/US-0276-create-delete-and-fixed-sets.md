# US-0276: creating and deleting sets, and the fixed strategy

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0274, US-0275
> min_tier: 1
> Files touched: 9
> Closes spec: no

## Objective
Add the `cl100k_base` ruler and the `fixed` strategy, the source of the text (text files and converted documents), and the tools `fs.create_chunk_set` and `fs.delete_chunk_set` with all their refusals, the 5 sets limit and the idempotent creation. A member can make a fixed set by hand, read it with the tools of US-0275, and delete it. The strategy `structural` is validated but answers `ERR_INTERNAL_ERROR: strategy structural is not implemented` until US-0277 replaces that line.

## Requirements covered
- SPEC-0101/FR-NEW-010 (partial: the ruler, the cleaning and the bounds for `fixed`): The system SHALL produce, for the same text and the same settings, the same chunks character for character, and SHALL produce no chunk above `max_tokens` tokens with the strategy `structural` or above `size_tokens` tokens with the strategy `fixed`.
- SPEC-0101/FR-NEW-011: WHEN a set with the strategy `fixed` is built THE system SHALL cut the text into chunks of `size_tokens` tokens, each starting `size_tokens` minus `overlap_tokens` tokens after the previous one, and SHALL apply no sentence rule.
- SPEC-0101/FR-NEW-013: WHEN a member of a project asks for a chunk set that the file of that project does not hold, with a strategy and optional settings, THE system SHALL build that set from the text of the file and answer with the set.
- SPEC-0101/FR-NEW-017 (partial: the create and delete checks): IF a request to a chunk tool is not allowed or not valid THEN THE system SHALL refuse it, change nothing, and answer with the exact text of the rule that failed.
- SPEC-0101/FR-NEW-018: WHEN a member of a project deletes a set that is not the default set THE system SHALL remove the set and its chunks and answer `{deleted: true}`.
- SPEC-0101/FR-NEW-020: The system SHALL cut the text of the file when the file is a text file, and the complete text kept with the conversion of the file, with its image captions, recognized text and tables, when the file is a converted document.
- SPEC-0101/FR-NEW-028: The system SHALL allow the chunk tools to the members of the project of the file only.
- SPEC-0101/FR-NEW-029: WHEN a member of a project asks for a chunk set that the file already holds with the same requested strategy and the same requested settings THE system SHALL answer with that set and build nothing.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_03 e2e_0101_04 e2e_0101_051 e2e_0101_139 chunking::`. For example: a text of 3000 tokens, `fs_create_chunk_set` fixed 1000 and overlap 200, assert 4 chunks starting 800 tokens apart and each at most 1000 tokens by the ruler, then create it again and get the same set id.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0101): Add the dependency `tiktoken-rs` 0.12.1 for the `cl100k_base` ruler; the ranks are embedded in the crate, so no file or network is needed at runtime. **Obliged by:** SPEC-0101/FR-NEW-010 (sizes in `cl100k_base` tokens, offline). **Rationale:** the owner imposes the ruler (spec 3.3); this crate ships it. **Alternatives considered:** `tokenizers` (needs a `tokenizer.json` file at runtime), hand written BPE (months of risk), a character based estimate (violates the ruler). **Source:** design; crates.io API 2026-11-21, README. **Evidence:** tiktoken-rs 0.12.1 MIT, `max_stable_version` 0.12.1; context7 had no quota, ddg and crates.io used as fallback. A new third party dependency is category 2 of the questions: it is reported under Needs you, with this choice as the default.
- DEC-002 (SPEC-0101): A `chunking` module of pure functions: `tokens` (ruler singleton, count, encode, lossy decode), `fixed`, `blocks` plus `structural` (paragraph v3a), `model`, `service` (sources, build, replace), `store`. **Obliged by:** SPEC-0101/FR-NEW-001 to FR-NEW-012. **Rationale:** determinism and testability without a server. **Alternatives considered:** put cutting in `search/` (couples it to engines). **Source:** design. **Evidence:** `search/chunker.rs:11` is the code replaced.
- DEC-005 (SPEC-0101): `request_key` (canonical JSON of the requested strategy and settings with server defaults filled in) is part of the primary key `(volume_id, path, request_key)`; creating an equal request returns the existing row; the default set is the row with `is_default` 1 and its key follows the server defaults at each rebuild (a hand made set with the same new key is absorbed into the default). **Obliged by:** SPEC-0101/FR-NEW-029, FR-NEW-013. **Rationale:** the primary key makes two members creating the same set at once safe on both engines (the loser reads the winner). **Alternatives considered:** unique index (the schema layer has none), a lock. **Source:** design. **Evidence:** `storage/rel/schema.rs` (`Index` is non unique).
- DEC-006 (SPEC-0101): At most 5 sets per file is counted inside the creating transaction after the equality check. **Obliged by:** SPEC-0101/FR-NEW-017. **Rationale:** last check, only for a new request, as the spec orders. **Alternatives considered:** check before building (wrong order). **Source:** spec. **Evidence:** n/a.
- DEC-008 (SPEC-0101): Text source: a file whose bytes are valid UTF-8 is cut as is; otherwise a file with a current artifact set is cut from the text `tools::artifacts::document_view` builds (captions true, tables as Markdown); otherwise `ERR_NOT_SUPPORTED: no text to cut for <path>`. **Obliged by:** SPEC-0101/FR-NEW-020. **Rationale:** reuse of the one document text the product already composes. **Alternatives considered:** the raw conversion text without captions (misses image text). **Source:** existing code. **Evidence:** `tools/artifacts.rs:307`, `docs/artifacts/render.rs:107`.
- DEC-015 (SPEC-0101): The five tools live in `tools/chunks.rs` and call one engine each; the MCP surface only. No REST twin. **Obliged by:** SPEC-0101/FR-NEW-013 to FR-NEW-018. **Rationale:** the spec names tools; the screen (BL-0175) will add REST. **Alternatives considered:** REST now (not asked). **Source:** spec. **Evidence:** `mcp/server.rs:2318-2342` (pattern).
- DEC-016 (SPEC-0101): Cuts run in `spawn_blocking`; the ruler is a `OnceLock` singleton. **Obliged by:** SPEC-0101/FR-NEW-010 (bounded linear cost), NFR 7.1. **Rationale:** a 30 000 token text must not block the runtime. **Alternatives considered:** inline cut. **Source:** design. **Evidence:** n/a.
- DEC-017 (SPEC-0101): A set made by hand keeps its requested strategy and settings and is rebuilt with them; `structural` requested on a text with no heading is stored as `fixed` with `fallback_from` and rebuilds as `structural` once a heading exists. **Obliged by:** SPEC-0101/FR-NEW-012. **Rationale:** the spec says so; requested values are stored beside used values. **Alternatives considered:** store used only (loses the request). **Source:** spec. **Evidence:** n/a.
### Data model
No new table. `insert_set` and `delete_set` of `ChunkStore` use the primary key `(volume_id, path, request_key)` for equal requests; the 5 sets limit is counted in the same transaction after the equality lookup.
### Contracts
Tools `fs.create_chunk_set(mount_id, path, strategy, settings = {})` and `fs.delete_chunk_set(mount_id, path, set_id)`; answers of design 5.1. Settings per strategy: fixed `size_tokens`, `overlap_tokens`; structural `heading_cut_depth`, `max_tokens`, `list_max_items`; check order for each request setting in order: unknown name `ERR_INVALID_ARGUMENT: unknown setting '<name>'`, not for the strategy `ERR_INVALID_ARGUMENT: setting '<name>' does not apply to strategy '<strategy>'`, not a whole number `ERR_INVALID_ARGUMENT: <name> must be a whole number between <min> and <max>`, out of range `ERR_INVALID_ARGUMENT: <name> must be between <min> and <max>` (`overlap_tokens` 0..8191), then overlap against size `ERR_INVALID_ARGUMENT: overlap_tokens must be smaller than size_tokens`. Other texts: strategy `ERR_INVALID_ARGUMENT: strategy must be 'fixed' or 'structural'`, no text `ERR_NOT_SUPPORTED: no text to cut for <path>`, 6th set `ERR_INVALID_ARGUMENT: a file holds at most 5 chunk sets`, default delete `ERR_INVALID_ARGUMENT: the default chunk set cannot be deleted`, unknown set `ERR_NOT_FOUND: chunk set not found: <set_id>`. Rust: `chunking::tokens::{count, encode, decode_lossy}`, `chunking::fixed::cut(text, title, size, overlap) -> Vec<Chunk>`, `chunking::service::{source_text, create_set, delete_set}`.
### Paths
| Path | Action |
|---|---|
| `Cargo.toml`, `crates/core/Cargo.toml`, `Cargo.lock` | modify: `tiktoken-rs = "0.12.1"` (confirm the function names on docs.rs) |
| `crates/core/src/chunking/{tokens,fixed,service}.rs` | create |
| `crates/core/src/chunking/store.rs` | modify: `insert_set` with the limit, `delete_set` |
| `crates/core/src/tools/chunks.rs`, `mcp/server.rs` | modify: two tools |
| `crates/core/src/chunking/e2e.rs` | modify: these tests, a token exact text helper |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, tool count tests and docs | regenerate and fix (121 to 123) |
### Existing patterns
Same tool wiring as US-0275; text source reads bytes through `VolumeClient::read_bytes` (`search/indexer.rs:182` shows the UTF-8 test) and, for a converted document, `tools::artifacts::document_view(state, mount, path, true, Some("markdown"))` (`tools/artifacts.rs:307`) whose `text` field is the document text; the `sha256` of the node comes from `VolumeClient::stat`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-021: a person who is not a member
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_021_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; Dave (`dave@example.com`) is not a member
- **When** Dave lists the sets, lists the chunks, reads a chunk, creates a set and deletes a set of `/big.md`
- **Then** each answer is `ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'`
- **And** no set was created or removed
- **Priority:** High

### SPEC-0101/E2E-030: a fixed set with overlap by hand
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_030_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` holds 3000 tokens of prose
- **When** Alice creates a set with strategy `fixed` and settings `size_tokens` 1000 and `overlap_tokens` 200
- **Then** the answer is a set with `default` false, strategy `fixed`, `chunk_count` 4 (chunks start at tokens 0, 800, 1600 and 2400), each chunk of at most 1000 tokens, empty heading path, unknown pages
- **And** the list of sets now holds the default set and this set
- **Priority:** High

### SPEC-0101/E2E-031: creating the same set twice
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_031_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` has no extra set
- **When** Alice creates a `fixed` set with `size_tokens` 500 twice
- **Then** both answers carry the same set id
- **And** the list holds one extra set
- **Priority:** High

### SPEC-0101/E2E-033: an unknown strategy
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_033_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates a set with strategy `sentence`
- **Then** the answer is `ERR_INVALID_ARGUMENT: strategy must be 'fixed' or 'structural'`
- **And** no set was created
- **Priority:** High

### SPEC-0101/E2E-034: a setting that does not apply
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_034_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates a `structural` set with `overlap_tokens` 50, then a `fixed` set with `heading_cut_depth` 3
- **Then** the answers are `ERR_INVALID_ARGUMENT: setting 'overlap_tokens' does not apply to strategy 'structural'` and `ERR_INVALID_ARGUMENT: setting 'heading_cut_depth' does not apply to strategy 'fixed'`
- **And** no set was created
- **Priority:** High

### SPEC-0101/E2E-035: a setting out of range
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_035_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates `fixed` sets with `size_tokens` 31 and 8193
- **Then** each answer is `ERR_INVALID_ARGUMENT: size_tokens must be between 32 and 8192`
- **And** sets with `size_tokens` 32 and 8192 are accepted
- **Priority:** High

### SPEC-0101/E2E-036: an overlap not below the size
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_036_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates a `fixed` set with `size_tokens` 100 and `overlap_tokens` 100
- **Then** the answer is `ERR_INVALID_ARGUMENT: overlap_tokens must be smaller than size_tokens`
- **And** a set with `overlap_tokens` 99 is accepted
- **Priority:** High

### SPEC-0101/E2E-037: a sixth set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_037_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` holds its default set and 4 extra sets
- **When** Alice creates a fifth extra set, then deletes one extra set and creates a different one
- **Then** the first creation answers `ERR_INVALID_ARGUMENT: a file holds at most 5 chunk sets`; after the deletion the creation succeeds
- **And** the list never holds more than 5 sets
- **Priority:** High

### SPEC-0101/E2E-038: deleting an extra set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_038_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` has its default set and an extra `fixed` set
- **When** Alice deletes the extra set
- **Then** the answer is `{deleted: true}`; the list holds the default set only; listing the chunks of the deleted set answers `ERR_NOT_FOUND: chunk set not found: <id>`
- **And** the default set is unchanged
- **Priority:** High

### SPEC-0101/E2E-039: deleting the default set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_039_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` has its default set
- **When** Alice deletes the default set
- **Then** the answer is `ERR_INVALID_ARGUMENT: the default chunk set cannot be deleted`
- **And** the set is still listed
- **Priority:** High

### SPEC-0101/E2E-040: deleting an unknown set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_040_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice deletes the set `set-999`
- **Then** the answer is `ERR_NOT_FOUND: chunk set not found: set-999`
- **And** nothing is changed
- **Priority:** High

### SPEC-0101/E2E-041: no text to cut
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_041_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/logo.png` is a PNG image
- **When** Alice creates a `fixed` set for it
- **Then** the answer is `ERR_NOT_SUPPORTED: no text to cut for /logo.png`
- **And** no set was created
- **Priority:** High

### SPEC-0101/E2E-042: two members create the same set together
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_042_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; Carol (`carol@example.com`) is a member too
- **When** Alice and Carol create a `fixed` set with `size_tokens` 500 within the same second
- **Then** both answers carry the same set id
- **And** the list holds one extra set
- **Priority:** Medium

### SPEC-0101/E2E-044: fixed chunks with and without overlap
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_044_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` holds 10000 tokens of prose
- **When** Alice creates `fixed` sets with `size_tokens` 2048 and `overlap_tokens` 0 (default), then `size_tokens` 1000 and `overlap_tokens` 200
- **Then** the first set has 5 chunks, the last holding 1808 tokens; the second has 13 chunks, chunk 2 starting 800 tokens after chunk 1
- **And** every chunk of both sets is at most its `size_tokens`
- **Priority:** Medium

### SPEC-0101/E2E-045: a removed member loses access
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_045_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; Carol was a member and the owner removed her
- **When** Carol lists the sets, creates a set and deletes a set of `/essay.md`
- **Then** each answer is `ERR_FORBIDDEN: 'carol@example.com' is not a member of 'proj'`
- **And** the sets of `/essay.md` are unchanged
- **Priority:** High

### SPEC-0101/E2E-051: a document not converted yet
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_051_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/raw.pdf` was uploaded and never converted
- **When** Alice creates a set for it
- **Then** the answer is `ERR_NOT_SUPPORTED: no text to cut for /raw.pdf`
- **And** no set was created
- **Priority:** High

### SPEC-0101/E2E-139: an unknown setting name and a value that is not a whole number
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_139_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates a `fixed` set with the setting `window` 5, then with `size_tokens` `2.5`, then with `size_tokens` `abc`
- **Then** the answers are `ERR_INVALID_ARGUMENT: unknown setting 'window'`, then twice `ERR_INVALID_ARGUMENT: size_tokens must be a whole number between 32 and 8192`
- **And** no set was created
- **Priority:** High

## Constraints
- `structural` is built in US-0277; do not implement it here.
- Nothing builds a default set here.
- Cuts run inside `spawn_blocking`.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/tools.md` rows for the two tools; `.agent_docs/dependencies.md` one line for `tiktoken-rs`.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.

- [ ] One commit `US-0276: creating and deleting sets, and the fixed strategy` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
