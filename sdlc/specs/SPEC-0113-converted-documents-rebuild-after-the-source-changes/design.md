# SPEC-0113: A converted document is rebuilt after its source changes, design

> Spec: sdlc/specs/SPEC-0113-converted-documents-rebuild-after-the-source-changes/spec.md
> Nature: FEAT
> Depth: M
> Security: none
> Designed on: 2026-11-26
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22).

## 2. Approaches
### Chosen: origin table in the volume store, a `conversion` kind in the scheduler, a host trait for the conversion
`companion_origin(volume_id, md_path, source_path, source_sha, md_sha, created_at)` is written by `write_companion` after a landing. The scheduler stays free of `AppState`: a `ConversionHost` trait (one method) is set once after the state exists, implemented over a `Weak<AppState>`, calling the engine `fs_ops::rebuild_companion`. The write hooks call one function (`indexer::follow_source`) that reads the origin and schedules or cancels.
Pros: reuses the scheduler, `derived_job`, the converter resolution, `write_companion` and its admission lock unchanged. Cons: one primary key read per file write when search is on.
### Rejected: a column `origin` on `nodes`
Every node write path (write, move, copy, trash, restore, git, extraction) would have to preserve or reset it; a missed path silently marks a hand file as converted.
### Rejected: a flag cleared by every writer ("hand edited")
A hook on every write path; the sha256 comparison needs none.
### Rejected: giving the scheduler `Arc<AppState>`
A cycle (state holds search holds scheduler); a weak host breaks it and keeps the scheduler testable.

## 3. Architecture
```
write of /a.pdf -> indexer::follow_source -> origin(/a.md) ok? -> scheduler.schedule_conversion(/a.pdf)
settle delay -> Scheduler.rebuild(kind Conversion) -> host.rebuild(project, path, cause, guard)
  -> doc_service_for(project) -> fs_ops::rebuild_companion -> write_companion (admission lock, artifact commit, origin)
  -> indexer::after_companion
fs.recalculate kind conversion -> scheduler.recalculate_kind(.., Conversion)
```

## 4. Data model
Table `companion_origin` (volume store, keyed by `volume_id`, added to `volume_tables`, so teardown and `migrate` copy it): text columns `volume_id`, `md_path`, `source_path`, `source_sha`, `md_sha`, bigint `created_at`; primary key `volume_id, md_path`. A database without it gets it from `CREATE TABLE IF NOT EXISTS` (no row: manual).

## 5. Contracts and interfaces
`fs.recalculate` `kind` accepts `conversion` (contract change). `JobKind::Conversion`, text `conversion`. `fs_ops::rebuild_companion(client, safety, service, project, path, cause, guard) -> Result<Rebuilt>` with `Rebuilt::{Done, Unchanged, Superseded}`. `Scheduler::{set_conversion_host, schedule_conversion, cancel_conversion}`.

## 6. Decisions
- DEC-001 (S-2B) one engine, `write_companion`, with an optional guard.
- DEC-002 (S-05) the host runs the conversion under `system:conversion` and refunds the session part.
- DEC-003 (S-05) `app.rs` moves `recover()` after the state exists, so a recovered due job always finds its host.

## 7. Files touched
`storage/meta.rs`, `storage/rel/schema.rs` (fingerprints), `core/fs_ops.rs`, `search/derived/{mod,scheduler,store}.rs`, new `search/derived/conversion.rs`, `search/indexer.rs`, `tools/derived.rs`, `mcp/server.rs`, `app.rs`, tests `search/derived_e2e.rs` (Options), new `search/conversion_e2e.rs`; docs README.md, AGENTS.md, `.agent_docs/{search,tools,architecture,testing}.md`, `TOOL_CONTRACT.txt`, `tool-contract-golden.json`.

## 8. Stories
- US-0338 the origin of a companion (FR-NEW-001, 002, 013; X8).
- US-0339 the conversion job after a source change (FR-NEW-003 to 009, 011, 012; S1, S3, F1 to F4, X1 to X7, X9, X10).
- US-0340 fs.recalculate conversion, PostgreSQL, docs and contract (FR-NEW-010; S2, X11).

## 9. Risks
- A hook per write costs one read: bounded to one primary key lookup, skipped without a scheduler.
- Two conversions of the same source cannot run together (one running per key); two sources of one companion can: the artifact lock and the admission lock order them, the last landing wins and records its origin.
