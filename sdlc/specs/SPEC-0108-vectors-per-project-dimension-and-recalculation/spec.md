# Vectors at the project dimension and their recalculation: functional specification

> Id: SPEC-0108
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 4 areas (per project embedding target, vector storage per dimension, vector read tool, recalculation job), 17 requirements, 1 new persisted concept (job progress columns on `derived_job`). Auto run, part 2 of 5 (BL-0171); parts 3 to 5 (new search tools, document reconciliation, file metadata screen) are out of this spec.
> Status: implemented
> Generated on: 2026-11-26
> Project: existing
> From backlog: BL-0171
> Depends on: SPEC-0107
> Security: none
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
SPEC-0107 lets a project name its text and image embedding models, but search still embeds with one platform wide model and stores every vector at one platform wide width. This increment makes the profile real: the vectors of a project are computed with the models of its profile and stored at the dimension of those models, in PostgreSQL (pgvector) and in SQLite (sqlite-vec), so two projects with different models and widths live side by side. A member reads the vectors of a document with `fs.get_vectors`. The owner or a platform administrator rebuilds every vector of a project, or of one document, with `admin.recalculate_start`, follows it with `admin.recalculate_status` and stops it with `admin.recalculate_cancel`. The out of date marker of the profile now has one consistent life: set by a model change (profile or registry edit), cleared only by a finished full recalculation.

## 2. Current State (MANDATORY, every depth)
Search vectors are computed by the default text embedding model of the registry, else the `search.embedding` block, and stored in one table `search_chunks` of `VECTOR(N)` with N the platform setting `search.embedding.dimensions` (PostgreSQL) or in one in memory `vec0` table per project (SQLite) created at that same N. A project profile (SPEC-0107) is stored and shown but no engine reads it. No tool returns a stored vector. A rebuild is per file (`fs.recalculate`, SPEC-0100). Images kept from a document (SPEC-0103) have no vector. Editing a model in the registry (`admin.update_model`) changes nothing for the projects using it.

### 2.1 What users can do today
- Rebuild the search entries of one file (14.1 row 1).
- Choose a profile that no engine uses (14.1 row 2).
- Nothing reads a stored vector or rebuilds a whole project on demand (14.1 row 3).

### 2.2 Specs governing this area
- SPEC-0107 (profile, marker): consumed; its marker rules extended (FR-NEW-009, 010).
- SPEC-0100 (derived jobs): the `derived_job` table, the scheduler and `recover` are reused.
- SPEC-0102 (registry): `update_model` gains a side effect (FR-NEW-010).
- SPEC-0105 (artifacts follow their document): the images a vector is built from.

### 2.3 What is verified today
Engine tests with a fake embedding service (`search/e2e.rs`, `vector_*.rs`); no test of a per project model or dimension (14.1 row 4).

## 3. Scope
### 3.1 In scope
- The embedding target and dimension per project, from its profile, with the existing fallbacks.
- Vector storage per dimension, two kinds (text chunks, images), both engines.
- Refusal of an unusable vector (wrong length, zero, not finite).
- `fs.get_vectors`; `admin.recalculate_start`, `admin.recalculate_status`, `admin.recalculate_cancel`.
- Job progress and restart behavior of a recalculation.
- The marker rules; `admin.update_model` marking the projects that use the model.
- PostgreSQL tests of all of it.
### 3.2 Out of scope, and why
- New search tools over these vectors, and score formulas: parts 3 and 4.
- A REST route or screen for recalculation and vectors: not asked (YAGNI).
- Re embedding when a chunk strategy changes: SPEC-0101 owns chunk sets.
- Moving vectors between dimensions: they are rebuilt, never converted.
### 3.3 Constraints imposed by the context
- Tool count 148 to 152; the tool contract is regenerated.
- `derived_job` gains nullable columns through `ColumnMigration`; an older binary ignores them.
- PostgreSQL holds one table per dimension and kind, created on first use; pgvector `ivfflat` indexes cap at 2000 dimensions, wider tables are scanned without index.
- No secret in any answer, record or log.

## 4. Actors
| Actor | Role |
|---|---|
| Member | reads vectors of a document |
| Owner | owner of the project; starts, reads and cancels a recalculation |
| Platform administrator | same as owner on any project; edits models |
| Outsider | refused |

## 4.5 Bounded contexts
Search (vectors), derived jobs (recalculation), settings (profile marker, model edit).

## 5. Usage scenarios
### SC-001: Two projects, two widths
`a` uses a 384 wide text model, `b` a 1024 wide one. Both index files and answer vector queries; `fs.get_vectors` shows 384 values for `a` and 1024 for `b`.
### SC-002: Read the vectors of a document
A member asks `fs.get_vectors` for a file and gets its chunk numbers, texts and vectors, 20 per page by default, with the next position; `kind` `image` gives the image vectors of a converted document.
### SC-003: Rebuild a project
Bob changes the text model, sees the vectors out of date, starts a recalculation, polls its progress to `done`; the vectors have the new width and the marker is cleared.
### SC-004: Rebuild one document
Bob starts a recalculation of one path; only it is rebuilt; the marker is left as it was.
### SC-005: Stop it
Bob cancels a running recalculation; it ends `cancelled`, the marker stays, a second cancel changes nothing.
### SC-006: Edit a used model
The administrator changes the dimension or the name of a model that projects use; those projects (only) read out of date.
### SC-007: Refusals
Unknown project, unknown file, a relative path, an indexing mode without vectors, a person who may not, an embedding service that is down or answers an unusable vector: each is refused or ends `failed` with a plain reason; nothing half written stays.
### SC-008: Repeat, concurrency, restart
Starting again while one runs answers the running job; starting another scope is refused; a file edited during a run is indexed from its new content; a restart ends the running job `failed` and starts it again.

## 6. Functional requirements
### 6.1 New
- FR-NEW-001: WHEN the platform embeds a chunk or a query of a project THE platform SHALL use the text model of its profile (provider endpoint plus `/embeddings`, model name, the provider bearer), else the server default text model, else the `search.embedding` block, and the dimension of the profile model, else the one of the platform setting.
- FR-NEW-002: THE platform SHALL store the text vectors of a project in a store of the dimension in force for the project, and the image vectors in a store of the dimension of its image model; two projects with different dimensions SHALL both index and query.
- FR-NEW-003: WHEN an embedding answer has a length other than the dimension in force, is a zero vector, or holds a value that is not finite THE platform SHALL refuse it as `ERR_INTERNAL_ERROR` with `embedding has <n> values, the model declares <d>`, `embedding is a zero vector` or `embedding holds a value that is not a finite number`, and store nothing for that file.
- FR-NEW-004: WHEN a file is indexed THE platform SHALL replace all its text vectors whatever their dimension, so a dimension change leaves none behind.
- FR-NEW-005: WHEN a vector query runs THE platform SHALL search only the store of the dimension in force and never delete or change anything.
- FR-NEW-006: WHEN a member calls `fs.get_vectors(mount_id, path, kind, after, limit)` THE platform SHALL answer `path`, `kind`, `dimension`, `vectors` (`number`, `text`, `vector`) in number order and `next_after` (null on the last page); `kind` is `text` (default) or `image`; `limit` 1 to 100 (default 20); `after` the last number of the previous page (default 0).
- FR-NEW-007: IF the file has no vector THEN `fs.get_vectors` SHALL answer an empty list with `dimension` null; IF the file does not exist THEN `ERR_NOT_FOUND`; IF search is off or the project mode has no vectors THEN `ERR_NOT_SUPPORTED`; a bad `kind`, `limit` or `after` is `ERR_INVALID_ARGUMENT`.
- FR-NEW-008: WHEN the owner or a platform administrator calls `admin.recalculate_start(project_id, path)` THE platform SHALL start one job of kind `vectors` over every file of the project (empty path) or over one file, rebuild each file's text vectors and, when the profile has an image model, the vectors of its images, and answer the job (`job_id`, `state`, `path`, `progress`, `already_running`).
- FR-NEW-009: WHEN a whole project recalculation ends `done` and the marker is unchanged since the job started THE platform SHALL clear the marker; a document recalculation, a `failed`, `cancelled` or interrupted job SHALL leave it.
- FR-NEW-010: WHEN `admin.update_model` changes the dimension or the name of a model THE platform SHALL, in the same transaction, set the vectors out of date for every project whose profile names it, and for no other.
- FR-NEW-011: `admin.recalculate_status(project_id)` SHALL answer the running job, else the last finished one, with `state` (`running`, `done`, `failed`, `cancelled`), `progress` (`done`, `total` files), `reason` when failed, `ended_at` when finished, and `vectors_out_of_date`; with no job `job` is null.
- FR-NEW-012: `admin.recalculate_cancel(project_id)` SHALL stop the running job after its current file and answer `cancelled` true, or false when none runs (no error).
- FR-NEW-013: WHILE a job runs, starting the same scope SHALL answer it with `already_running` true; another scope SHALL be `ERR_INVALID_ARGUMENT` `a recalculation of <scope> is already running`.
- FR-NEW-014: IF the project index mode is not `rag` or `both` THEN `admin.recalculate_start` SHALL be `ERR_NOT_SUPPORTED` `the index mode of the project has no vectors`; a path that is not an absolute file of the project is `ERR_INVALID_ARGUMENT` or `ERR_NOT_FOUND`; anyone but the owner or an administrator is `ERR_FORBIDDEN`, for all three tools alike on start and cancel, a member or administrator for status.
- FR-NEW-015: IF a file changes while a job reads it THEN the job SHALL discard its result for that file and the file SHALL be indexed from its new content by its own rebuild.
- FR-NEW-016: WHEN the server restarts THE platform SHALL end a running `vectors` job `failed` with `interrupted by a server restart` and start the same scope again.
- FR-NEW-017: WHEN the index mode of a project changes or the project is removed while a recalculation runs THE platform SHALL stop the job after its file in flight and that file SHALL write nothing.
### 6.2 Modified
- FR-MOD-001 (SPEC-0102 `admin.update_model`): marks the projects of FR-NEW-010; its answer and record are unchanged.
- FR-MOD-002 (SPEC-0100 `derived_job`): kind `vectors`, state `cancelled`, progress columns; `fs.list_derived_jobs` keeps its accepted filters.
### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
A job embeds sequentially, one request at a time; status is one row read. Progress is written after each file.
### 7.2 Security and access
Owner or administrator start and cancel; member or administrator read; vectors are read as the files are (membership).
### 7.3 Privacy and compliance
Vectors derive from file content and are returned only to members.
### 7.4 Usability and accessibility
Refusal texts are plain sentences.
### 7.5 Reliability
A file's vectors are replaced in one transaction; a job ends in exactly one finished state; a failed file ends the job and leaves earlier files rebuilt.
### 7.6 Operability
Upgrade adds nullable columns and creates vector tables on first use; no manual step.

## 8. Edge cases
- Dimension above 2000 on PostgreSQL: table without index, still exact.
- A project with a text model of the platform width uses the legacy table `search_chunks`, so existing vectors stay valid.
- SQLite vectors live in memory as before; a restart empties them and the recalculation of FR-NEW-016 refills them.
- A trashed or deleted file loses its vectors with its other search entries.

## 9. Acceptance tests (the contract)
### 9.1 Summary
Twenty one tests (E2E-017 is folded into E2E-008, numbers 001 to 021 otherwise) in `crates/core/src/settings/vectors_e2e.rs`, `search/vector_store_e2e.rs` and `settings/profile_pg_e2e.rs`, names `e2e_0108_NNN_slug`, run by `cargo test -p fsa-core --lib --features rag -- e2e_0108_`. Driver: the real `McpServer` tool methods on the SPEC-0102 fixture with `rag` search, a fake embedding service that answers per model name, SQLite; the PostgreSQL ones run with `MCPFS_TEST_PG_DSN` (pgvector) and skip without it.

### 9.2 New tests
Happy (5): E2E-001 two widths side by side (SC-001, FR-NEW-001, 002, 004, 005); E2E-002 read the vectors of a document, paging and images (SC-002, FR-NEW-006); E2E-003 rebuild a project, marker cleared (SC-003, FR-NEW-008, 009, 011); E2E-004 rebuild one document, marker kept (SC-004, FR-NEW-008, 009); E2E-005 fallback to the default and configuration models (FR-NEW-001).
Failures (6): E2E-006 invalid arguments of the four tools (FR-NEW-007, 014); E2E-007 service down ends `failed`, mode `bm25` and `none` refused (SC-007, FR-NEW-014); E2E-008 repeat start, other scope, cancel twice, cancel with none, and the marker left by a cancel, one test covering what the plan numbered E2E-017 (SC-005, SC-008, FR-NEW-009, 012, 013); E2E-009 wrong length, zero and non finite vectors refused, nothing stored (FR-NEW-003); E2E-010 access of the four tools (FR-NEW-014, 007); E2E-011 `get_vectors` of a missing file, of a file with no vector, of search off (FR-NEW-007).
Extra (10): E2E-012 `update_model` marks only the projects that use it (SC-006, FR-NEW-010); E2E-013 SQLite and PostgreSQL parity of the store (FR-NEW-002, 004, 005); E2E-014 file edited during a job (FR-NEW-015); E2E-015 delete removes text and image vectors (edge cases); E2E-016 resume after restart (FR-NEW-016); E2E-018 PostgreSQL dims 384 and 1024 coexist and `search_chunks` stays the platform width table (FR-NEW-002, edge case); E2E-019 PostgreSQL profile set, marker and update_model through the registry; E2E-020 `search.status` counts text vectors of every dimension and the dimension change leaves none behind (FR-NEW-004); E2E-021 a mode change while a job runs stops it and no vector comes back (added by review).
### 9.3 Exploit tests
None: no new injection surface beyond a table name built from an integer (E2E-018 asserts the name).
### 9.4 Modified tests
Tool count assertions (148 to 152), `every_admin_tool_is_registered` (41 to 44), the golden contract; engine tests whose fake answers a wrong length are fixed in the fake, not in the assertion.
### 9.5 Removed tests
None.

## 10. Traceability
| Requirement | Tests |
|---|---|
| FR-NEW-001 | E2E-001, 005 |
| FR-NEW-002 | E2E-001, 013, 018 |
| FR-NEW-003 | E2E-009 |
| FR-NEW-004 | E2E-001, 013, 020 |
| FR-NEW-005 | E2E-001, 013 |
| FR-NEW-006 | E2E-002 |
| FR-NEW-007 | E2E-006, 010, 011 |
| FR-NEW-008 | E2E-003, 004 |
| FR-NEW-009 | E2E-003, 004, 007, 008 |
| FR-NEW-017 | E2E-021 |
| FR-NEW-010 | E2E-012, 019 |
| FR-NEW-011 | E2E-003, 007 |
| FR-NEW-012 | E2E-008 |
| FR-NEW-013 | E2E-008 |
| FR-NEW-014 | E2E-006, 007, 010 |
| FR-NEW-015 | E2E-014 |
| FR-NEW-016 | E2E-016 |
| FR-MOD-001 | E2E-012 |
| FR-MOD-002 | E2E-003, 008 |

## 11. Open questions
None. Assumptions (S-03, S-05): image vectors are produced by the recalculation and by a file rebuild, with the image sent as a data URI string in the `input` of the same OpenAI compatible `/embeddings` call; a recalculation never wipes first (a cancel leaves old and new vectors, never none).

## 12. Glossary (MANDATORY)
| Term | Meaning |
|---|---|
| Dimension in force | the dimension of the project's profile model of the kind, else the platform setting |
| Vector store | the table (PostgreSQL) or `vec0` table (SQLite) holding vectors of one kind and dimension |
| Recalculation | the `vectors` job rebuilding the vectors of a project or a document |

## 13. Implementability gate (MANDATORY)
Every requirement has a test; every test names scenario and requirement; no open question; 17 requirements and 21 tests are inside the depth M budget. Pass.

## 14. Evidence appendix
### 14.1 Current State evidence
| Row | Evidence |
|---|---|
| 1 | `search/derived/scheduler.rs` `recalculate`, `tools/derived.rs` |
| 2 | `settings/profile.rs` (stored), no read in `search/` (`rg text_model_id crates/core/src/search` is empty) |
| 3 | `search/mod.rs` `SearchBackend` has no read of a vector; `rg get_vectors` is empty |
| 4 | `search/vector_pg.rs` tests, `search/e2e.rs`; one fixed dimension |
### 14.2 Class sweep
Per dimension tables: `search_chunks` is created at one width (`vector_pg.rs:44-55`), `search_vec` likewise (`vector_sqlite.rs:72-80`). Job kinds: `JobKind` is matched in `store.rs`, `scheduler.rs`, `tools/derived.rs`.
### 14.3 Design inputs
Reuse `Scheduler`, `JobStore`, `RegistryTargets`, `SettingsStore`.
