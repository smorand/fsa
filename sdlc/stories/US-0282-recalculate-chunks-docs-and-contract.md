# US-0282: recalculate chunks by hand, docs and contract

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0280, US-0281
> min_tier: 2
> Files touched: 8
> Closes spec: yes

## Objective
Add the kind `chunks` to `fs.recalculate` with the optional `set_id`: one set is rebuilt at once from the current text (the default set also refreshes the search entries when search is on and the project indexes), with the refusal order of the spec and a job of kind `chunks` that the job list shows. Close the spec: regenerate the contract, update every doc, run the whole acceptance set.

## Requirements covered
- SPEC-0101/FR-NEW-019: WHEN a member of a project asks `fs.recalculate` for the kind `chunks` of a file THE system SHALL rebuild that set at once from the current text, ignoring the settle delay, and answer with the job as for the kind `search_index`.
- SPEC-0101/FR-MOD-003: WHEN a member asks `fs.recalculate` for a kind THE system SHALL accept `search_index` and `chunks`.
- SPEC-0101/FR-NEW-017 (partial: the refusals of `fs.recalculate` for the kind `chunks`): IF a request to a chunk tool is not allowed or not valid THEN THE system SHALL refuse it, change nothing, and answer with the exact text of the rule that failed.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_ e2e_0100_` and `make check`. For example `fs_recalculate(proj, /guide.md, chunks)` with new text saved 5 s ago (settle delay 30 s) answers a job and within 7 s the default set is `current` with the new chunks, while the waiting `search_index` rebuild is still listed.

## Design excerpt
### Decisions
- DEC-012 (SPEC-0101): The rebuild (`JobKind::SearchIndex`) cuts the sets first; a new `JobKind::Chunks` (manual only, key per file and kind, `set_id` column on `derived_job`) cuts one set. Both reuse the guard, supersede and stale rules of SPEC-0100. **Obliged by:** SPEC-0101/FR-NEW-019, FR-NEW-021, FR-NEW-022, FR-MOD-002, FR-MOD-003. **Rationale:** the scheduler already carries kind in its key. **Alternatives considered:** a second scheduler. **Source:** existing code. **Evidence:** `search/derived/mod.rs:21`, `search/derived/scheduler.rs:182,439`.
- DEC-005 (SPEC-0101): `request_key` (canonical JSON of the requested strategy and settings with server defaults filled in) is part of the primary key `(volume_id, path, request_key)`; creating an equal request returns the existing row; the default set is the row with `is_default` 1 and its key follows the server defaults at each rebuild (a hand made set with the same new key is absorbed into the default). **Obliged by:** SPEC-0101/FR-NEW-029, FR-NEW-013. **Rationale:** the primary key makes two members creating the same set at once safe on both engines (the loser reads the winner). **Alternatives considered:** unique index (the schema layer has none), a lock. **Source:** design. **Evidence:** `storage/rel/schema.rs` (`Index` is non unique).
### Data model
`derived_job` gains the nullable column `set_id` (additive `ColumnMigration`, schema golden regenerates).
### Contracts
`JobKind::Chunks` (`"chunks"`); `tools/derived.rs::recalculate(state, person, mount, path, kind, set_id)`; kind text `ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'`; order: project, member, kind unknown, path absent, folder, then for `search_index`: `set_id` given `ERR_INVALID_ARGUMENT: set_id applies to kind 'chunks' only`, search not enabled, indexing off; for `chunks`: set unknown `ERR_NOT_FOUND: chunk set not found: <set_id>`, no default set `ERR_NOT_FOUND: the file has no default chunk set`, no text `ERR_NOT_SUPPORTED: no text to cut for <path>`. `chunks` works when search is off or the project does not index. One job per file and kind whatever the set; supersede rules of SPEC-0100/FR-NEW-008 per kind; it does not cancel a waiting `search_index` job. Answer: job object of `fs.list_derived_jobs` plus `set_id` for kind `chunks`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/search/derived/{mod,scheduler,store}.rs`, `storage/admin.rs` | modify: kind, `set_id` |
| `crates/core/src/tools/derived.rs`, `mcp/server.rs` | modify: `recalculate` arguments and order |
| `crates/core/src/chunking/e2e.rs` | modify: these tests; SPEC-0100 `e2e_0100_069` updated to the new refusal order (E2E-MOD-003) |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, schema golden fixtures, count tests | regenerate and fix |
| `README.md`, `AGENTS.md`, `.agent_docs/{tools,search,architecture,testing,config}.md` | modify: tools (123), counts, kinds, chunking section, test command `cargo test -p fsa-core --lib -- e2e_0101_` |
### Existing patterns
`tools/derived.rs:218-249` (checks and order), `Scheduler::recalculate` `search/derived/scheduler.rs:204`, tool counts as US-0273 of SPEC-0100 (rg for the old totals).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-110: recalculating the default set by hand
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_110_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 30 s; `/guide.md` has a default set; Alice saved new text 5 s ago (a rebuild waits)
- **When** Alice calls `fs.recalculate` with kind `chunks` on `/guide.md`, then lists its sets and searches a word of the new text
- **Then** the answer names a job of kind `chunks` (state `running` or `done`); within 7 s the default set is `current` with the new chunks and the search finds the new text
- **And** the waiting `search_index` rebuild of the file is still listed with its time left
- **Priority:** Critical

### SPEC-0101/E2E-111: recalculating an extra set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_111_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has an extra `fixed` set built from older text
- **When** Alice calls `fs.recalculate` with kind `chunks` and that `set_id`
- **Then** within 7 s that set is `current` with the chunks of the current text; the default set is untouched
- **And** the search entries are not rebuilt
- **Priority:** High

### SPEC-0101/E2E-112: recalculating an unknown set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_112_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md`
- **When** Alice calls `fs.recalculate` with kind `chunks` and `set_id` `set-999`
- **Then** the answer is `ERR_NOT_FOUND: chunk set not found: set-999`
- **And** no job started
- **Priority:** High

### SPEC-0101/E2E-113: recalculating when there is no default set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_113_*`)
- **Given** project `plain` (mode `none`); `/guide.md` with no set
- **When** Alice calls `fs.recalculate` with kind `chunks` and no `set_id`
- **Then** the answer is `ERR_NOT_FOUND: the file has no default chunk set`
- **And** no job started
- **Priority:** High

### SPEC-0101/E2E-114: set_id with the wrong kind
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_114_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a default set
- **When** Alice calls `fs.recalculate` with kind `search_index` and a `set_id`
- **Then** the answer is `ERR_INVALID_ARGUMENT: set_id applies to kind 'chunks' only`
- **And** no job started
- **Priority:** High

### SPEC-0101/E2E-115: an unknown kind
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_115_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md`
- **When** Alice calls `fs.recalculate` with kind `concepts`
- **Then** the answer is `ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'`
- **And** no job started
- **Priority:** High

### SPEC-0101/E2E-138: recalculating with search not enabled
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_138_*`)
- **Given** the server has search disabled; project `plain` (mode `none`); `/guide.md` has an extra `fixed` set made by hand
- **When** Alice calls `fs.recalculate` with kind `chunks` and that `set_id`
- **Then** the answer is a job and within 7 s the set is `current` with the chunks of the current text
- **And** the same call with kind `search_index` on an existing file answers `ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config`; a search does not find the new text
- **Priority:** Medium

### SPEC-0101/E2E-052: a converted document that changed
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_052_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/report.pdf` has an extra set; the document is replaced by a new version and converted again
- **When** Alice lists its sets before and after she recalculates the set by hand
- **Then** the set is `out_of_date` before and `current` after, with the chunks of the new text
- **And** the list never shows chunks of the new text under a set marked `current` before the rebuild
- **Priority:** Medium

### SPEC-0101/E2E-MOD-003: a rebuild by hand with search disabled (was SPEC-0100/E2E-069)
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_mod003_*`)
- **Validated before:** with search disabled for the server, `fs.recalculate` of kind `search_index` on `/notes.md` answered `ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config` (the test never created the file).
- **Validates now:** the file `/notes.md` exists and the same answer is given; with an absent path the answer is `ERR_NOT_FOUND: file not found: /notes.md`, because `search not enabled` now comes after `path absent` and `path is a folder`.
- **Given** search disabled for the server, project `proj`, Alice a member, `/notes.md` saved
- **When** Alice calls `fs.recalculate` with kind `search_index` on `/notes.md`, then on `/ghost.md`
- **Then** the answers are `ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config` and `ERR_NOT_FOUND: file not found: /ghost.md`
- **Priority:** High

## Constraints
- No REST route.
- `search.index` is unchanged here.
- Tool counts must match everywhere (rg for the previous totals in docs and tests).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: All of `README.md`, `AGENTS.md`, `.agent_docs/*` named in Paths; no dashes as punctuation.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.
- [ ] Closes spec: the spec full acceptance set green (every E2E-001 to E2E-146 and E2E-MOD-001 to E2E-MOD-003) and every requirement of SPEC-0101 cited by at least one story with status done or landed; regenerate the tool contract and review the diff.
- [ ] One commit `US-0282: recalculate chunks by hand, docs and contract` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
