# SPEC-0108: Vectors at the project dimension and their recalculation, design

> Spec: sdlc/specs/SPEC-0108-vectors-per-project-dimension-and-recalculation/spec.md
> Nature: FEAT
> Depth: M
> Security: none
> Designed on: 2026-11-26
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22): `sqlx`, `rusqlite` and `sqlite-vec` already hold both engines.

## 2. Approaches
### Chosen: the embedding source resolves a target per project; each engine owns one store per kind and dimension
`EmbedTargets` gains `target_for(project)` and `image_target_for(project)` (default: the old single target, so every existing fake keeps working). `RegistryTargets` reads the profile. PostgreSQL: one table per kind and dimension created on first use, the legacy `search_chunks` kept for the platform width. SQLite: the per project in memory connection recreates its `vec0` table when the dimension changes. Pros: no migration of stored vectors, two widths coexist, queries never write. Cons: a table per width.
### Rejected: one table with an untyped `vector` column
pgvector cannot index or compare without a fixed dimension per index; a partial index per dimension would need the same DDL on the fly and a cast on every query (DEC-001).
### Rejected: a separate recalculation scheduler
`derived_job` already persists jobs, recovery and listing (DEC-005).

## 3. Architecture
```
 mcp/server.rs (4 #[tool]) -> tools/vectors.rs (auth, args) -> SearchBackend::vectors_of / Scheduler::start_vectors
 SearchBackend (vector_pg, vector_sqlite) -> EmbedTargets::target_for -> RegistryTargets -> profile row + model + provider
 Scheduler (derived/vectors.rs): runner task per project -> rebuild_file (+ images) -> JobStore progress, finish, clear marker
 Registry::try_update_model -> SettingsStore::mark_model_projects_stale
```

## 4. Data model
`derived_job`: `progress_done`, `progress_total` nullable integers (`ColumnMigration`). New values `vectors` (kind) and `cancelled` (state). PostgreSQL: `search_chunks_<dim>` (text, dim other than the platform width) and `search_images_<dim>`, same columns as `search_chunks` (`volume_id, path, chunk_idx, chunk_text, embedding`), the name built from an integer only. SQLite: tables `search_vec_meta`/`search_vec` (text) and `search_img_meta`/`search_img` (image), created lazily at the dimension.

## 5. Contracts and interfaces
- `fs.get_vectors(mount_id, path, kind?, after?, limit?)`, `admin.recalculate_start(project_id, path?)`, `admin.recalculate_status(project_id)`, `admin.recalculate_cancel(project_id)`.
- `SearchBackend::vectors_of(volume, path, kind, after, limit) -> VectorPage`; `index_images_checked(volume, path, &[ImageInput], guard)`.
- `check_vector(&[f32], dimension)` in `search::embedding`.

## 6. Decisions
- DEC-001 (S-2B): per kind and dimension stores, see 2. Obliged by FR-NEW-002.
- DEC-002 (S-10): the dimension in force is the model's; the platform setting only names the legacy table and the fallback. Obliged by FR-NEW-001.
- DEC-003 (S-05): a vector query on a dimension with no store answers nothing and creates nothing on SQLite; on PostgreSQL it creates the empty table (idempotent DDL). Obliged by FR-NEW-005.
- DEC-004 (AUTO-006): editing a used model's dimension or name MARKS its projects out of date, it does not refuse. Refusing would block an administrator from correcting a wrong dimension, and the marker is the state a model switch already produces (FR-NEW-009 of SPEC-0107). Obliged by FR-NEW-010.
- DEC-005 (S-10): the recalculation is a `derived_job` row of kind `vectors`, path `/` for a project; one running per project; the task lives in the scheduler, not in a slot. Obliged by FR-NEW-008, 011, 013, 016.
- DEC-006: marker cleared with `UPDATE ... WHERE vectors_stale_at = <value read at job start>`, so a model change during the job survives. Obliged by FR-NEW-009.
- DEC-007: a file changed while the job reads it makes the guard false (the scheduler marks the in flight flag on `schedule`), the file is skipped and counted, its own waiting rebuild indexes it. Obliged by FR-NEW-015.
- DEC-008 (AUTO-004): image input is a `data:<mime>;base64,` string in `input`. Obliged by FR-NEW-008.
- DEC-009 (AUTO-005): restart ends the row failed and starts the same scope again (resume equals restart: every file is rebuilt idempotently). Obliged by FR-NEW-016.

## 7. File structure impact
| Path | Action | Story |
|---|---|---|
| `search/embedding.rs`, `settings/registry.rs`, `settings/store.rs`, `settings/profile_store.rs` | modify: per project target, `check_vector`, model usage queries | US-0324 |
| `search/vector_pg.rs`, `vector_sqlite.rs`, `search/mod.rs` | modify: per dimension stores, `vectors_of`, images | US-0324 |
| `tools/vectors.rs`, `mcp/server.rs`, `tools/all.rs` | create and modify: `fs.get_vectors` | US-0324 |
| `settings/registry.rs` | modify: update_model marks projects | US-0325 |
| `search/derived/*`, `storage/admin.rs` | modify: kind, state, progress, runner, recovery | US-0326 |
| `tools/recalc.rs`, `mcp/server.rs`, `app.rs` | create and modify: three admin tools, resume at boot | US-0326 |
| `search/vectors_e2e.rs`, `settings/profile_pg_e2e.rs` | create: tests | US-0324 to 0326 |
| docs, contract | modify | US-0326 |

## 8. Stories
| Story | Title | Tier | Depends |
|---|---|---|---|
| US-0324 | vectors at the project dimension and `fs.get_vectors` | 1 | none |
| US-0325 | a model edit marks its projects out of date, PostgreSQL profile tests | 2 | US-0324 |
| US-0326 | the recalculation job, its tools, restart, docs and contract (closes spec) | 1 | US-0325 |

## 9. Risks
pgvector DDL race between processes (single process assumed, one in process mutex); SQLite vectors are in memory (pre existing); fingerprint and tool count assertions.
