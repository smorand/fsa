# SPEC-0105: Document artifacts follow their document, design

> Spec: sdlc/specs/SPEC-0105-document-artifacts-follow-their-document/spec.md
> Nature: FEAT
> Depth: L
> Security: n/a
> Designed on: 2026-11-23
> Target tier: 2 (default: no `Tier:` in the constitution, no `.spec.json`)

## 1. Stack and skills
| Element | Choice | Version | DEC |
|---|---|---|---|
| Language, storage, tests | the existing stack (Rust 2024, rusqlite and sqlx, `cargo test`) | as in `Cargo.toml` | none |
| New dependency | none | n/a | DEC-012 |

Skills loaded: `rust` (already in context). Effect: `make check` is the gate; tests are `#[cfg(test)]` modules driven through the real `McpServer` tool methods. Existing project (`git ls-files '*.rs'` far above 300 files).

## 2. Approaches
### Chosen: carry the rows inside the existing transactions
The artifact tables are keyed by path and tied to a node revision. A move already rewrites every moved node's path and revision in one transaction (`tx_rename`) and carries chunk sets in it; the same transaction also rewrites `path` and `node_rev` of the current artifact set and its pointer. Trash and restore use the same function. A copy adds one new store method that duplicates the current sets in one transaction after the files are written, with the weight admitted before anything is written. Pros: no schema change, one code path for move, rename, trash and restore, all or nothing by transaction. Cons: the copy has two steps (files then sets) and its second step can fail after the files exist (DEC-009). Affected scope: `storage/meta.rs`, `storage/volume.rs`, `core/fs_ops.rs`, `quota.rs`, `tools/artifacts.rs`, `tools/trash.rs`. Best suited when the data is already tied to a path and a revision, as here.
### Rejected: key the sets by node id instead of path
Moves would then need no rewrite, but every reader, the sweep, the pointer and the schema change (`doc_artifact_current`, `doc_artifact_sets`), for a gain of two UPDATE statements (DEC-002).
### Rejected: rebuild the artifacts after a move or copy (like the search entries)
It needs the converter, which the spec forbids calling (FR-NEW-012).
### Rejected: delete artifacts in the delete transaction instead of relying on the sweep
Blob counts must be released with the sweep's garbage list (`purge.rs:306-310`); a second place releasing blobs is a second place to get wrong (DEC-005).

## 3. Architecture
```
 tools/lifecycle (move, copy, delete) --> core/fs_ops.rs --> storage/volume.rs --> storage/meta.rs
 tools/trash.rs (delete to trash, restore) ------------------^        (tx_rename, copy_artifact_sets)
 tools/artifacts.rs (reads) -- current_artifact_set (path + node_rev join) -- sweep (purge.rs)
```
Bounded contexts: Files (paths, trash, copies) calls Documents (artifact tables) only through the meta store methods.

## 4. Data model
No change. Tables `doc_artifact_current`, `doc_artifact_sets`, `doc_images`, `doc_tables`, `doc_headings` (`storage/meta.rs:233-310`). A move rewrites `path` and `node_rev` of the unsuperseded set and `path` and `node_rev` of the pointer row; a copy inserts a new set row (new `set_id`, `path` of the copy, `node_rev` of the copy's node) with copies of its image, table and heading rows and one more reference on each blob.

## 5. Contracts and interfaces
- No tool or route changes; the golden contract is unchanged.
- New refusal: `move_path` answers `ERR_INVALID_ARGUMENT: '<source>' cannot be moved onto or into itself` when the destination is the source, inside the source or an ancestor of it, after the missing source check and before the destination check, with or without overwrite.
- Store: `RelationalMetaStore::copy_artifact_sets(&self, pairs: &[(String, String)]) -> Result<usize>` (source path, destination path; skips a source without a current set; returns the number of sets copied) and `RelationalMetaStore::artifact_set_weight(&self, path: &str) -> Result<i64>` (0 when no current set).
- `quota::admit_file_with(client, safety, person, mount_id, path, new_size, extra_session) -> Result<Admission>`: `admit_file` plus an extra session charge; the project delta stays `new_size - previous`.
- `tools::artifacts::current_set` refuses a path under the trash directory with `ERR_NOT_FOUND: No artifacts: document not extracted`.

## 6. File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/src/storage/meta.rs` | modify: `tx_rename` carries artifacts; `rename_into_trash` keeps them; `copy_artifact_sets`; `artifact_set_weight`; tests | FR-NEW-001, 002, 003, 004, 006, 007, 008 | US-0302 to US-0307 |
| `crates/core/src/core/fs_ops.rs` | modify: move guard, copy admission and artifact copy, self copy | FR-NEW-013, 003, 004, 005, 006 | US-0303, US-0306, US-0307 |
| `crates/core/src/quota.rs` | modify: `admit_file_with` | FR-NEW-005 | US-0306 |
| `crates/core/src/tools/artifacts.rs` | modify: trash path refusal; existing tests changed | FR-NEW-007, FR-MOD-001, FR-MOD-002 | US-0302, US-0304, US-0306 |
| `crates/core/src/docs/artifacts/weight.rs`, `docs/artifacts/mod.rs` | create / modify: weight from stored rows | FR-NEW-003 | US-0306 |
| `crates/core/src/tools/artifacts_follow_e2e.rs`, `crates/core/src/tools/mod.rs` | create / modify: tests `e2e_0105_*` | all | US-0302 to US-0308 |
| `crates/core/src/purge.rs`, `crates/core/src/storage/conformance.rs` | modify only if a test seam is needed | FR-NEW-009 | US-0305 |
| `README.md`, `AGENTS.md`, `.agent_docs/architecture.md`, `.agent_docs/testing.md` | modify | all | US-0308 |
Affected existing tests (all in `tools/artifacts.rs`): `spec_0019_e2e_088` (move), `089` (copy), `090` (trash and restore), `093` (copy over), `095` (folder move), `097` (away and back), `099` and `a_copy_is_charged_the_file_bytes_only` (copy charge): modify, each story changes the ones of its operation (DEC-014). Unchanged: `094`, `108`, `092`, `141`, `137`, `134`.
Affected documentation: as listed.

## 7. Compatibility and risks
Behavior changes: the artifacts of a moved, renamed, trashed or copied document are kept; a move onto itself is refused with a new code and text where the no-clobber refusal or a data loss happened. No schema change, so no migration; documents converted earlier follow too. Risk: the sweep deletes a current set whose node is missing at the set's path or whose revision differs (`meta.rs:1176-1184`), so a move that forgets `path` or `node_rev` loses the set at the next sweep; the story tests run the sweep after each move (DEC-002). Rollback: revert; sets written at moved paths remain valid.

## 9. Test strategy
All tests: functional e2e through the real `McpServer` tool methods, in `crates/core/src/tools/artifacts_follow_e2e.rs`, names `e2e_0105_<id>_<slug>` (`e2e_0105_mod003_...` for modified), converting through the SPEC-0103 stand in converters (`docs/manifest_e2e.rs`). The allowance is set with the safety write quota and read with `SafetyManager::session_remaining` (a test seam); the retention sweep and the project purge run with an explicit `now` (`purge.rs`). Run: `cargo test -p fsa-core --lib -- e2e_0105_`. Test data: generated.

| Stories | Tests |
|---|---|
| US-0302 | E2E-001, 002, 003, 004, 013, 014, 026, 027, 031, 032, 033, 034, 035, 038, 039, 048, 071, MOD-001, MOD-004, MOD-006 |
| US-0303 | E2E-009, 015, 020, 022, 029, 036, 047, 060, 061, 065, 066 |
| US-0304 | E2E-007, 008, 016, 021, 023, 025, 028, 042, 043, 044, 045, 053, 064, MOD-003 |
| US-0305 | E2E-011, 040, 046, 058, 063, 070 |
| US-0306 | E2E-005, 010, 012, 018, 024, 037, 041, 049, 050, 051, 052, 059, 067, 069, MOD-002, MOD-005, MOD-007 |
| US-0307 | E2E-006, 019, 057, 062, 068 |
| US-0308 | E2E-017, 030, 054, 055, 056 |

## 10. Decisions log
- **DEC-001:** All tests live in `tools/artifacts_follow_e2e.rs` and reuse the SPEC-0103 stand ins. **Obliged by:** SPEC-0105/FR-NEW-001 (tests of every requirement). **Rationale:** one module, one run command; `tools/artifacts.rs` is 4 400 lines. **Alternatives considered:** extending `tools/artifacts.rs` tests. **Source:** design. **Evidence:** `docs/manifest_e2e.rs`, `tools/read_metadata_e2e.rs`.
- **DEC-002:** `tx_rename` takes a second flag `carry_artifacts`. For each moved node, with its new revision `r`: delete the pointer row at the new path, then `UPDATE doc_artifact_sets SET path=new, node_rev=r WHERE path=old AND superseded_at IS NULL`, then `UPDATE doc_artifact_current SET path=new, node_rev=r WHERE path=old`, all inside the existing loop and transaction. `rename` passes true; sets left at an overwritten destination keep their old revision and are swept. **Obliged by:** SPEC-0105/FR-NEW-001, FR-NEW-002, FR-NEW-006. **Rationale:** a move already rewrites revisions per node and carries chunk sets the same way; one transaction means all or nothing (FR-NEW-010). **Alternatives considered:** key by node id (schema and reader change); update the sets at the call site (not atomic with the rename). **Source:** existing code. **Evidence:** `storage/meta.rs:1841-1900`, `:1176-1184`, `:1277-1290`.
- **DEC-003:** `rename_into_trash` keeps deleting chunk sets but passes `carry_artifacts = true`; `restore` already goes through `rename`. **Obliged by:** SPEC-0105/FR-NEW-007, FR-NEW-008. **Rationale:** the trash path then holds the set with the new revision, so the sweep keeps it while the node exists. **Alternatives considered:** a separate trash table (a second store). **Source:** existing code. **Evidence:** `storage/meta.rs:2523-2538`, `tools/trash.rs:229-249`.
- **DEC-004:** `current_set` in `tools/artifacts.rs` refuses a path under the trash directory with `No artifacts: document not extracted`, using `safety.config().trash_dir`. **Obliged by:** SPEC-0105/FR-NEW-007. **Rationale:** the artifacts of a trashed item must not be readable before the restore. **Alternatives considered:** a column marking trashed sets. **Source:** spec. **Evidence:** `tools/artifacts.rs:291-296`, `tools/trash.rs:137`.
- **DEC-005:** Removal for good relies on the existing sweep: a hard delete removes the node, so the next sweep deletes its current set without grace and releases its blobs; project removal clears the tables with the volume. A test runs the sweep and asserts the rows are gone. **Obliged by:** SPEC-0105/FR-NEW-009. **Rationale:** one place releases blobs. **Alternatives considered:** delete in the delete transaction (a second blob release path). **Source:** existing code. **Evidence:** `storage/meta.rs:1176-1184`, `purge.rs:262,306-310`, `storage/mod.rs:522-528`.
- **DEC-006:** `move_path` checks, after the source exists and before the destination exists, whether the destination equals the source, lies under it, or is an ancestor of it (path prefix with a `/` boundary, `/` itself an ancestor of everything), and refuses with `ToolError::invalid_argument("'<source>' cannot be moved onto or into itself")`. **Obliged by:** SPEC-0105/FR-NEW-013. **Rationale:** today overwrite deletes the destination first. **Alternatives considered:** guard in `tx_rename` only (the delete has already run). **Source:** existing code. **Evidence:** `core/fs_ops.rs:1296-1328`.
- **DEC-007:** The weight of a set is computed in Rust from its stored rows with the same formula as `PendingSet::bytes()`: per image `size + caption + ocr_text`, per table `csv_text` (or the CSV rendered from the cells) `+ Markdown + caption`, the heading titles, and `text_len`; `artifact_set_weight` reads the rows and `docs::artifacts::weight::of_rows` applies the formula. **Obliged by:** SPEC-0105/FR-NEW-003. **Rationale:** a copy is charged what the conversion was charged, with no new column. **Alternatives considered:** a stored weight column (schema change, null for old sets). **Source:** existing code. **Evidence:** `docs/artifacts/capture.rs:197-212`.
- **DEC-008:** `copy_artifact_sets(pairs)` runs in one transaction: for each pair with a current set it inserts a new set (new id, destination path, the destination node's revision, `created_at` now) and copies the image, table and heading rows with `INSERT ... SELECT`, adds one reference per image blob and one for the text blob, marks the unsuperseded sets at the destination `superseded_at`, and upserts the destination pointer. **Obliged by:** SPEC-0105/FR-NEW-003, FR-NEW-004, FR-NEW-006. **Rationale:** equal independent sets (editing one hides only its own). **Alternatives considered:** share the set between paths (an edit of one would hide both). **Source:** existing code. **Evidence:** `storage/meta.rs:1005-1130`, `:1609-1660`.
- **DEC-009:** Admission before writing: a single file copy calls `quota::admit_file_with(..., extra_session = weight)`, a folder copy calls `admit_paths(session_bytes = total weight, ...)`; both refuse before anything is written, so nothing is kept (FR-NEW-005). If `copy_artifact_sets` fails after the files were written, the copied files are removed and the session charge refunded; a failure after an overwrite is reported as an internal error. **Obliged by:** SPEC-0105/FR-NEW-005, FR-NEW-010. **Rationale:** the allowance check is the only expected refusal. **Alternatives considered:** write the sets inside `put_file` (couples the volume to documents). **Source:** existing code. **Evidence:** `quota.rs:47-62,137-160`, `core/fs_ops.rs:1352-1385`.
- **DEC-010:** A file copied onto itself with overwrite skips the rewrite, is charged as a copy and refreshes the modification time; a folder copy into itself works from its snapshot, so each document is copied once. **Obliged by:** SPEC-0105/FR-NEW-006. **Rationale:** rewriting would renew the revision and hide the artifacts. **Alternatives considered:** refuse it (the spec chose to allow). **Source:** spec. **Evidence:** `core/fs_ops.rs:1352-1385`, `storage/volume.rs:277-295`.
- **DEC-011:** A folder copy onto an existing folder merges: `copy_tree` rewrites counterparts and leaves other destination files alone; artifact pairs are built from the snapshot of source documents, so a destination document with a counterpart takes the counterpart's set (or none) and the others are untouched. **Obliged by:** SPEC-0105/FR-NEW-006. **Rationale:** matches how files merge today. **Alternatives considered:** clearing the destination folder (data loss). **Source:** existing code. **Evidence:** `storage/volume.rs:277-295`.
- **DEC-012:** No schema change and no new dependency. **Obliged by:** SPEC-0105/FR-NEW-012. **Rationale:** the data already carries path and revision. **Alternatives considered:** a weight column. **Source:** DEC-002, DEC-007. **Evidence:** `storage/meta.rs:233-310`.
- **DEC-013:** Chunk sets and search entries keep their rules (chunk sets carried on move and restore, removed on trash and delete, not copied); the new code touches none of them and the audit log is unchanged (a restore writes none). **Obliged by:** SPEC-0105/FR-NEW-012. **Rationale:** non regression. **Alternatives considered:** none. **Source:** SPEC-0100, SPEC-0101. **Evidence:** `storage/meta.rs:1893-1903`, `search/indexer.rs:489-556`.
- **DEC-014:** Each story updates the existing SPEC-0019 tests of its operation to the new rule, naming them in its commit: 088 and 097 (US-0302), 095 (US-0302), 090 (US-0304), 089, 093, 099 and `a_copy_is_charged_the_file_bytes_only` (US-0306). **Obliged by:** SPEC-0105/FR-MOD-001, FR-MOD-002. **Rationale:** the old expectation is the behavior this spec removes. **Alternatives considered:** leaving them red. **Source:** spec. **Evidence:** `tools/artifacts.rs:3859-4330`.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0302 | artifacts follow a moved or renamed document or folder | P1 | FR-NEW-001, FR-NEW-002, FR-NEW-011, FR-MOD-001, FR-NEW-010 | main | 1 | no |
| US-0303 | overwrite by move and the move onto itself refusal | P1 | FR-NEW-006, FR-NEW-013, FR-NEW-010, FR-NEW-012 | US-0302 | 2 | no |
| US-0304 | trash and restore keep the artifacts | P1 | FR-NEW-007, FR-NEW-008, FR-MOD-001 | US-0303 | 1 | no |
| US-0305 | removal for good and the rules that still discard | P2 | FR-NEW-009, FR-MOD-001, FR-NEW-010 | US-0304 | 2 | no |
| US-0306 | copy a converted document with its artifacts | P1 | FR-NEW-003, FR-NEW-005, FR-NEW-006, FR-MOD-002 | US-0302 | 1 | no |
| US-0307 | copy a folder of converted documents | P2 | FR-NEW-004, FR-NEW-005, FR-NEW-006 | US-0306 | 2 | no |
| US-0308 | non members, no converter call, audit and the docs | P2 | FR-NEW-010, FR-NEW-011, FR-NEW-012 | US-0307 | 2 | yes |
