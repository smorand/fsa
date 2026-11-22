# US-0281: chunk sets follow moves, copies and deletions

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0280
> min_tier: 2
> Files touched: 4
> Closes spec: no

## Objective
Keep chunk sets consistent with the file: a move (file or folder) carries the sets to the new path, a move over an existing file removes the replaced file sets first, a delete, a move to the trash and a project removal remove them, and a copy gets only the default set built by its own rebuild. It works in every index mode because it lives in the storage transaction of the node.

## Requirements covered
- SPEC-0101/FR-NEW-025: WHEN a file is moved, copied or deleted, or its project is deleted, THE system SHALL carry the sets of a moved file to the new path, give the copy only a default set built by its own rebuild, and remove the sets of a deleted file or of a deleted project.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_10 e2e_0101_146 storage::meta`. For example: `/a.md` with its default set and an extra fixed set is moved to `/b.md`; `/b.md` lists the same two ids and chunks, `/a.md` is refused as unknown; deleting `/b.md` and writing a new `/b.md` lists no set.

## Design excerpt
### Decisions
- DEC-007 (SPEC-0101): Sets follow their file in the same transaction as the node rows: `tx_rename` rewrites the path prefix of `chunk_sets` for a move; a delete, `rename_into_trash` and `remove_subtree` delete the sets of the path or subtree; a copy copies nothing. **Obliged by:** SPEC-0101/FR-NEW-025. **Rationale:** works in every index mode, which the search hooks (gated by the mode) cannot do. **Alternatives considered:** hooks in `search/indexer.rs` (skipped when indexing is off). **Source:** existing code. **Evidence:** `storage/meta.rs:1568-1620`, `2073-2260`; `search/indexer.rs:193` (the gate).
### Data model
No schema change; the same transaction as the node rows.
### Contracts
In `storage/meta.rs`: `tx_rename` rewrites `chunk_sets.path` with the subtree prefix for a plain move; the delete family (`delete_file`, `remove_subtree`, `rmdir`) and `rename_into_trash` delete `chunk_sets` rows of the path or subtree and their `chunks`; project removal for PostgreSQL adds the two tables to the list of tables cleared (`storage/mod.rs`, near the SQLite file removal). `core/fs_ops.rs` move with overwrite deletes the destination first, so the replaced file sets go through the delete.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/meta.rs` | modify: rename, deletes, trash |
| `crates/core/src/storage/mod.rs` | modify: project removal clears the tables on PostgreSQL |
| `crates/core/src/chunking/e2e.rs` | modify: these tests |
| `crates/core/src/storage/conformance.rs` | modify: one conformance check for both engines |
### Existing patterns
Prefix rewriting with `SubtreeMatch` (`storage/meta.rs:30-57`, `1568-1620`); trash in one transaction `rename_into_trash` (`storage/meta.rs:2230-2255`); the conformance suite that runs on every engine.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-100: a move carries the sets
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_100_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` has its default set and an extra `fixed` set
- **When** Alice moves `/a.md` to `/b.md`
- **Then** listing the sets of `/b.md` shows the same two set ids with the same chunks; `/a.md` is refused as unknown
- **And** no rebuild is needed for the sets to be readable
- **Priority:** High

### SPEC-0101/E2E-101: a copy gets its own default set only
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_101_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` has its default set and an extra `fixed` set
- **When** Alice copies `/a.md` to `/c.md` and waits 15 s
- **Then** `/c.md` lists one set, its default set; `/a.md` keeps its two sets
- **And** the extra set was not copied
- **Priority:** High

### SPEC-0101/E2E-102: a delete removes the sets
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_102_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` has two sets
- **When** Alice deletes `/a.md`
- **Then** listing its sets answers `ERR_NOT_FOUND: file not found: /a.md`; writing a new `/a.md` and listing gives an empty list until its rebuild
- **And** the new file gets no set of the old one
- **Priority:** High

### SPEC-0101/E2E-103: a move over an existing file
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_103_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` and `/b.md` each have a default set
- **When** Alice moves `/a.md` over `/b.md`
- **Then** `/b.md` lists the sets of the moved file; the sets of the replaced file are gone
- **And** `/a.md` is unknown
- **Priority:** Medium

### SPEC-0101/E2E-104: a project deleted then restored
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_104_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` has a default set
- **When** the platform admin deletes `proj`, then restores it
- **Then** while deleted, listing sets answers `ERR_PROJECT_NOT_FOUND`; after the restore `/a.md` lists no set until its next rebuild
- **And** no set of the deleted files reappears
- **Priority:** High

### SPEC-0101/E2E-105: a folder move
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_105_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/docs/a.md` and `/docs/b.md` each have a default set
- **When** Alice moves `/docs` to `/manual`
- **Then** `/manual/a.md` and `/manual/b.md` list their sets
- **And** nothing under `/docs` lists a set
- **Priority:** Medium

### SPEC-0101/E2E-146: copying and deleting a folder
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_146_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/docs/a.md` and `/docs/b.md` each have a default set
- **When** Alice copies `/docs` to `/copy`, waits 15 s, then deletes `/docs`
- **Then** `/copy/a.md` and `/copy/b.md` each list only a default set; `/docs/a.md` is refused with `ERR_NOT_FOUND: file not found: /docs/a.md`
- **And** no set of `/docs` remains
- **Priority:** Medium

## Constraints
- No hook in `search/indexer.rs` (it is gated by the index mode).
- A copy copies no set.
- Never forget `volume_id` in a `WHERE`.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0282.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.

- [ ] One commit `US-0281: chunk sets follow moves, copies and deletions` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
