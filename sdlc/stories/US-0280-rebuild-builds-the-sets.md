# US-0280: the rebuild builds the chunk sets before the search entries

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0274, US-0279
> min_tier: 1
> Files touched: 8
> Closes spec: no

## Objective
Make the SPEC-0100 rebuild build the sets: when the rebuild of a file runs it recuts every set of the file from the current text (the default set with the server defaults of the moment, created when missing, extra sets with their own settings), replaces them in one transaction keeping their ids, then indexes the default set; a file that is no longer valid UTF-8 loses its sets (or keeps them out of date for a converted document); a project that does not index builds nothing; a failed or discarded rebuild keeps the previous sets. Nothing converts at start (upgrade rule).

## Requirements covered
- SPEC-0101/FR-NEW-021: WHEN the rebuild of the search entries of a file runs, or the Markdown companion of a converted document is indexed, THE system SHALL first rebuild every chunk set of that file from its current text, the default set included, and SHALL create the default set when the file has none.
- SPEC-0101/FR-NEW-022: WHILE a rebuild of the sets of a file waits or runs THE system SHALL keep serving the previous sets, marked `out_of_date`, and SHALL replace them in one step when the rebuild ends.
- SPEC-0101/FR-NEW-026: The system SHALL leave the search entries of a file as they were until the first rebuild of that file after the upgrade creates its default set.
- SPEC-0101/FR-NEW-012 (partial: the fallback and hand made sets at every rebuild): IF a set with the strategy `structural` is built from a text that holds no heading THEN THE system SHALL build it with the strategy `fixed` and the server default fixed settings, and SHALL report the set with the strategy `fixed` and `fallback_from` equal to `structural`.
- SPEC-0101/FR-NEW-014 (partial: state while a rebuild waits or runs): WHEN a member of a project asks for the chunk sets of a file THE system SHALL list each set with its id, strategy, settings, whether it is the default set, `fallback_from` when it applies, its number of chunks, its token count, its state and the time it was built.
- SPEC-0101/FR-NEW-027 (partial: the defaults used by the default set): The system SHALL read the default settings of the chunk sets from the server configuration keys `search.chunking.fixed_size_tokens`, `search.chunking.fixed_overlap_tokens`, `search.chunking.heading_cut_depth`, `search.chunking.max_tokens` and `search.chunking.list_max_items`.
- SPEC-0101/FR-MOD-002: WHEN a rebuild of the search entries starts THE system SHALL build the chunk sets of the file first (FR-NEW-021).

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_00 e2e_0101_03 e2e_0101_05 e2e_0101_06 e2e_0101_11 e2e_0101_13 e2e_0101_14 e2e_0100_`. The SPEC-0100 tests must stay green. For example: save `/guide.md`, advance the clock past the settle delay, list the sets and see one default `structural` set, `current`, then save again and list at once to see it `out_of_date` until the rebuild ends.

## Design excerpt
### Decisions
- DEC-004 (SPEC-0101): Chunk sets and chunks are rows of two tables of the per volume meta store (`chunk_sets`, `chunks`), created beside `doc_artifact_*`; `state` is computed from `source_sha` against `nodes.sha256`. **Obliged by:** SPEC-0101/FR-NEW-009, FR-NEW-014, FR-NEW-022. **Rationale:** metadata next to the document, volume scoped, no extra service; a sha compare is stable across moves (`rev` changes on a move, `sha256` does not). **Alternatives considered:** the admin database, the search index, a stored `out_of_date` flag. **Source:** existing code. **Evidence:** `storage/meta.rs:231-330` (artifact tables), `storage/meta.rs:1607` (rev on rename).
- DEC-005 (SPEC-0101): `request_key` (canonical JSON of the requested strategy and settings with server defaults filled in) is part of the primary key `(volume_id, path, request_key)`; creating an equal request returns the existing row; the default set is the row with `is_default` 1 and its key follows the server defaults at each rebuild (a hand made set with the same new key is absorbed into the default). **Obliged by:** SPEC-0101/FR-NEW-029, FR-NEW-013. **Rationale:** the primary key makes two members creating the same set at once safe on both engines (the loser reads the winner). **Alternatives considered:** unique index (the schema layer has none), a lock. **Source:** design. **Evidence:** `storage/rel/schema.rs` (`Index` is non unique).
- DEC-012 (SPEC-0101): The rebuild (`JobKind::SearchIndex`) cuts the sets first; a new `JobKind::Chunks` (manual only, key per file and kind, `set_id` column on `derived_job`) cuts one set. Both reuse the guard, supersede and stale rules of SPEC-0100. **Obliged by:** SPEC-0101/FR-NEW-019, FR-NEW-021, FR-NEW-022, FR-MOD-002, FR-MOD-003. **Rationale:** the scheduler already carries kind in its key. **Alternatives considered:** a second scheduler. **Source:** existing code. **Evidence:** `search/derived/mod.rs:21`, `search/derived/scheduler.rs:182,439`.
- DEC-013 (SPEC-0101): Set replacement is one transaction (delete chunks, insert chunks, update the set row) keeping `set_id`; a failed or discarded rebuild never reaches it. **Obliged by:** SPEC-0101/FR-NEW-022. **Rationale:** readers see all old or all new chunks. **Alternatives considered:** write new set then swap id (breaks the kept id rule). **Source:** design. **Evidence:** n/a.
- DEC-017 (SPEC-0101): A set made by hand keeps its requested strategy and settings and is rebuilt with them; `structural` requested on a text with no heading is stored as `fixed` with `fallback_from` and rebuilds as `structural` once a heading exists. **Obliged by:** SPEC-0101/FR-NEW-012. **Rationale:** the spec says so; requested values are stored beside used values. **Alternatives considered:** store used only (loses the request). **Source:** spec. **Evidence:** n/a.
### Data model
No schema change.
### Contracts
`chunking::service::rebuild_file_sets(ctx, project, path, text, defaults, guard) -> Result<Option<Built>>` used by `Scheduler::rebuild` for `JobKind::SearchIndex`: for the default set and each extra set cut again, `ChunkStore::replace_set` keeps `set_id` and sets `built_at`; if the default set does not exist it is created (`is_default` 1, requested `structural` with the server defaults); a default whose requested key changed with the server defaults takes the new key and absorbs a hand made set with that key. The scheduler gets `ChunkDefaults` at construction (`Scheduler::new(..., defaults)`, `app.rs`). Non UTF-8: a file with a current artifact set keeps its sets; any other file deletes its sets and entries, the job ends `done`. A project with indexing off or search disabled builds no default set. `indexer::after_companion` (the Markdown companion of a converted document, indexed at once) first builds that companion's default set through the same function (FR-NEW-021).
### Paths
| Path | Action |
|---|---|
| `crates/core/src/search/derived/scheduler.rs` | modify: rebuild builds sets, takes defaults |
| `crates/core/src/chunking/service.rs`, `chunking/store.rs` | modify: `rebuild_file_sets`, `replace_set`, default key handling |
| `crates/core/src/search/indexer.rs` | modify: `full_index` after a mode change uses the same function; mode `none` keeps sets |
| `crates/core/src/app.rs` | modify: pass `ChunkDefaults` to the scheduler |
| `crates/core/src/chunking/e2e.rs` | modify: these tests (stub engine with a down switch, paused clock) |
| `.agent_docs/search.md`, `.agent_docs/architecture.md` | modify |
### Existing patterns
Rebuild skeleton `search/derived/scheduler.rs:439` (read, guard, supersede, finish) and the e2e fixtures of SPEC-0100 (`search/derived_e2e.rs`: `Env`, `Stub`, paused clock, restart helper).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-001: a saved Markdown file gets its default set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_001_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` is saved with a title, 3 sections under `## ` headings of about 300 tokens each
- **When** Alice saves it and, 15 s later, lists its chunk sets and searches for a word of the second section
- **Then** the list holds one set: strategy `structural`, `default` true, `state` `current`, `chunk_count` 1 (the three sections merge, the total fits 2048 tokens), `token_count` equal to the count of the chunk; the search finds `/guide.md` with `chunk_number` 1
- **And** the settings of the set read `heading_cut_depth` 2, `max_tokens` 2048, `list_max_items` 40
- **Priority:** Critical

### SPEC-0101/E2E-002: a failed rebuild keeps the previous sets
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_002_*`)
- **Given** project `proj` indexing its files (mode `both`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a current default set of 2 chunks; the embedding service is down
- **When** Alice saves `/guide.md` with new text and waits 15 s
- **Then** the rebuild of the file is `failed` with `embedding service unreachable`; the list of sets shows the same default set, `state` `out_of_date`, still 2 chunks of the old text
- **And** searching a word of the old text still finds `/guide.md`
- **Priority:** High

### SPEC-0101/E2E-003: a save during a rebuild discards its sets
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_003_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; a rebuild of `/guide.md` is running (the embedding service answers in 10 s)
- **When** Alice saves `/guide.md` again while it runs
- **Then** when the running rebuild ends its sets are not stored; the list still shows the previous default set `out_of_date` until the next rebuild ends, then `current` with the chunks of the last save
- **And** the chunks of the discarded text never appear in a list
- **Priority:** High

### SPEC-0101/E2E-004: a project that does not index has no default set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_004_*`)
- **Given** project `plain` (index mode `none`), Alice a member; `/guide.md` saved
- **When** Alice waits 15 s and lists the sets of `/guide.md`
- **Then** the list is empty: `{sets: []}`
- **And** creating a set by hand (E2E-030) still works in this project
- **Priority:** High

### SPEC-0101/E2E-005: a binary file gets no set and no error
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_005_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; a PNG image `/logo.png` of 2 KB
- **When** Alice saves it and waits 15 s, then lists its sets
- **Then** the list is empty and no failed job is listed for it
- **And** the write answered normally
- **Priority:** High

### SPEC-0101/E2E-006: a file that stops being text loses its sets
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_006_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/data.md` has a current default set
- **When** Alice overwrites it with the bytes of a PNG image and waits 15 s
- **Then** the list of sets is empty
- **And** the rebuild of the file ended `done`
- **Priority:** Medium

### SPEC-0101/E2E-007: the default set is rebuilt with the current server defaults
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_007_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a default set built with `max_tokens` 2048; the operator restarts the server with `search.chunking.max_tokens` set to 512
- **When** Alice saves `/guide.md` (`# T` then 3000 tokens of prose in 6 paragraphs) and waits 15 s
- **Then** the default set reads `max_tokens` 512 and holds chunks of at most 512 tokens
- **And** an extra set made by hand before the restart keeps its own settings
- **Priority:** Medium

### SPEC-0101/E2E-032: a set equal to the default set
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_032_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` has its default set (`structural`, server defaults)
- **When** Alice creates a `structural` set with empty settings
- **Then** the answer is the default set, `default` true
- **And** no new set exists
- **Priority:** High

### SPEC-0101/E2E-053: no automatic default set for a converted document
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_053_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/report.pdf` converted
- **When** Alice waits 15 s and lists the sets
- **Then** the list is empty (its sets are made by hand)
- **And** its Markdown companion `/report.md`, a text file, gets a default set
- **Priority:** Medium

### SPEC-0101/E2E-060: a text without headings falls back to fixed
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_060_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/notes.txt` holds 3000 tokens of prose with no heading line
- **When** Alice saves it, waits 15 s and lists its sets
- **Then** the default set shows strategy `fixed`, `fallback_from` `structural`, settings `size_tokens` 2048 and `overlap_tokens` 0, 2 chunks
- **And** the second chunk holds the remaining 952 tokens
- **Priority:** High

### SPEC-0101/E2E-061: adding a heading ends the fallback
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_061_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/notes.txt` is the fixed fallback
- **When** Alice inserts the line `# Notes` at the top and waits 15 s
- **Then** the default set shows strategy `structural` and no `fallback_from`
- **And** chunk 1 starts with the line `# Notes`
- **Priority:** High

### SPEC-0101/E2E-116: a rebuild by hand of the entries builds the sets first
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_116_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` was indexed before the upgrade and has no set
- **When** Alice calls `fs.recalculate` with kind `search_index`
- **Then** within 7 s the file lists a default set and the search entries come from it
- **And** the search results carry `chunk_number`
- **Priority:** High

### SPEC-0101/E2E-117: out of date while a rebuild waits
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_117_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a current default set
- **When** Alice saves new text and lists the sets 1 s later
- **Then** the default set is `out_of_date`; after the rebuild ends it is `current`
- **And** the chunks listed before the end are the old ones
- **Priority:** High

### SPEC-0101/E2E-130: server defaults are used
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_130_*`)
- **Given** the server configuration has `search.chunking.max_tokens` 1024 and `search.chunking.list_max_items` 20; project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member
- **When** Alice saves `/guide.md` (it begins with `# T`) and lists its sets
- **Then** the default set shows `max_tokens` 1024 and `list_max_items` 20
- **And** a set made by hand with empty settings gets the same values
- **Priority:** High

### SPEC-0101/E2E-133: absent or empty defaults
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_133_*`)
- **Given** a configuration with no `search.chunking` keys, then with empty values
- **When** the operator starts the server and Alice saves a Markdown file that begins with `# T`
- **Then** the server starts and the default set uses 2048, 2, 40
- **And** `fixed_size_tokens` 2048 and `fixed_overlap_tokens` 0 apply to a fallback
- **Priority:** Medium

### SPEC-0101/E2E-134: nothing converts at upgrade
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_134_*`)
- **Given** a file `/old.md` indexed before the upgrade with character chunks; the upgraded server starts
- **When** Alice lists its sets and searches a word of it
- **Then** the list is empty and the search still finds `/old.md`
- **And** no rebuild job is listed
- **Priority:** High

### SPEC-0101/E2E-135: the first rebuild converts the file
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_135_*`)
- **Given** same as E2E-134
- **When** Alice saves `/old.md` and waits 15 s
- **Then** the file lists its default set and the search results carry `chunk_number`
- **And** the character chunks are gone from the results
- **Priority:** High

### SPEC-0101/E2E-136: a rebuild by hand converts one file only
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_136_*`)
- **Given** two files `/old1.md` and `/old2.md` indexed before the upgrade
- **When** Alice calls `fs.recalculate` with kind `search_index` on `/old1.md`
- **Then** `/old1.md` lists a default set; `/old2.md` lists none
- **And** the search still finds both files
- **Priority:** Medium

### SPEC-0101/E2E-140: a non text file is not rebuilt automatically
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_140_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/report.pdf` converted with an extra set made by hand
- **When** Alice replaces the PDF with a new version, converts it again and waits 15 s
- **Then** the extra set is `out_of_date` and unchanged; the rebuild of the file ended `done`
- **And** recalculating the set by hand makes it `current`
- **Priority:** Medium

### SPEC-0101/E2E-141: a mode change keeps the sets
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_141_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a default set
- **When** the platform admin sets the index mode of `proj` to `none`, then changes the file and waits 15 s
- **Then** the default set is listed `out_of_date`, unchanged, and no rebuild ran
- **And** setting the mode back to `bm25` builds the default set again
- **Priority:** Medium

## Constraints
- Do not add the kind `chunks` here (US-0282).
- The guard of SPEC-0100 is the last check before the set transaction and before the engine commit.
- No automatic default set for a converted document.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/search.md`, `.agent_docs/architecture.md`.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.

- [ ] One commit `US-0280: the rebuild builds the chunk sets before the search entries` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
