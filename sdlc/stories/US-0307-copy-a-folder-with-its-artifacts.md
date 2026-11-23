# US-0307: copy a folder of converted documents

> Spec: SPEC-0105 (sdlc/specs/SPEC-0105-document-artifacts-follow-their-document/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0306
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
A folder copy gives each copied converted document its own artifacts, charges their total weight in one amount (the files charge nothing as today), is refused whole when it does not fit, merges onto an existing folder, and works into itself.

## Requirements covered
- SPEC-0105/FR-NEW-004: WHEN a member copies a folder THE Documents context SHALL give every copied converted document its own artifacts under FR-NEW-003, charging the weight of the copied artifacts to the session write allowance in one amount; the folder's files charge nothing to the session, as today.
- SPEC-0105/FR-NEW-005 (partial: folder: the artifact weight alone): IF the weight of the artifacts to copy, added to the files, exceeds what remains of the session write allowance THEN THE Documents context SHALL refuse the copy and keep nothing of it, not the files nor the artifacts.
- SPEC-0105/FR-NEW-006 (partial: folder copy merge and into itself): WHEN a member moves or copies a document onto an existing document with overwrite THE Documents context SHALL replace the destination's artifacts by those of the incoming document, AND the destination SHALL have no artifacts when the incoming document has none.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0105_ tools::lifecycle`. Tests copy `/docs` (D and D2) with room (allowance lower by 4 000), with room for D only (refused, no `/docs-copy`), onto an existing folder with a counterpart and a bystander, and into itself.

## Design excerpt
### Decisions
- DEC-008 (SPEC-0105): `copy_artifact_sets(pairs)` runs in one transaction: for each pair with a current set it inserts a new set (new id, destination path, the destination node's revision, `created_at` now) and copies the image, table and heading rows with `INSERT ... SELECT`, adds one reference per image blob and one for the text blob, marks the unsuperseded sets at the destination `superseded_at`, and upserts the destination pointer. **Obliged by:** SPEC-0105/FR-NEW-003, FR-NEW-004, FR-NEW-006. **Rationale:** equal independent sets (editing one hides only its own). **Alternatives considered:** share the set between paths (an edit of one would hide both). **Source:** existing code. **Evidence:** `storage/meta.rs:1005-1130`, `:1609-1660`.
- DEC-009 (SPEC-0105): Admission before writing: a single file copy calls `quota::admit_file_with(..., extra_session = weight)`, a folder copy calls `admit_paths(session_bytes = total weight, ...)`; both refuse before anything is written, so nothing is kept (FR-NEW-005). If `copy_artifact_sets` fails after the files were written, the copied files are removed and the session charge refunded; a failure after an overwrite is reported as an internal error. **Obliged by:** SPEC-0105/FR-NEW-005, FR-NEW-010. **Rationale:** the allowance check is the only expected refusal. **Alternatives considered:** write the sets inside `put_file` (couples the volume to documents). **Source:** existing code. **Evidence:** `quota.rs:47-62,137-160`, `core/fs_ops.rs:1352-1385`.
- DEC-011 (SPEC-0105): A folder copy onto an existing folder merges: `copy_tree` rewrites counterparts and leaves other destination files alone; artifact pairs are built from the snapshot of source documents, so a destination document with a counterpart takes the counterpart's set (or none) and the others are untouched. **Obliged by:** SPEC-0105/FR-NEW-006. **Rationale:** matches how files merge today. **Alternatives considered:** clearing the destination folder (data loss). **Source:** existing code. **Evidence:** `storage/volume.rs:277-295`.
### Data model
No schema change.
### Contracts
`copy_path` folder branch: snapshot `client.meta.subtree(src)`, pairs for every file, total weight, `admit_paths(session_bytes = total weight, ...)`, `copy_tree`, `copy_artifact_sets(pairs)`; failure after the tree is written reports an internal error (files stay, no sets).
### Paths
| Path | Action |
|---|---|
| `crates/core/src/core/fs_ops.rs` | modify: folder branch |
| `crates/core/src/tools/artifacts_follow_e2e.rs` | modify: tests |
| `crates/core/src/storage/volume.rs` | modify only if `copy_tree` must return the pairs |
### Existing patterns
Folder copy `core/fs_ops.rs:1352-1370`, `storage/volume.rs:277-295`, `admit_paths` `quota.rs:137-160`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0105/E2E-006: a folder holding converted documents is copied
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_006_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-004
- **Given** `/docs/report.pdf` (D) and `/docs/sub/other.pdf` (D2) converted; the allowance left is 100 000 bytes
- **When** Alice copies `/docs` to `/docs-copy`
- **Then** both copies give their own artifacts and both sources keep theirs
- **Priority:** High

### SPEC-0105/E2E-019: a folder copy where the last document's artifacts do not fit
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_019_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-004, FR-NEW-005, FR-NEW-010
- **Given** `/docs` holds D (artifacts 3 000 bytes) and D2 (artifacts 1 000 bytes) converted; the allowance left is 3 500 bytes
- **When** Alice copies `/docs` to `/docs-copy`
- **Then** the copy is refused with `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded` and `/docs-copy` does not exist at all
- **And** the sources keep their artifacts
- **Priority:** High

### SPEC-0105/E2E-057: a folder copy charges the artifacts of each document and keeps every source
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_057_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-004
- **Given** `/docs` holds D and D2 converted (artifacts 3 000 and 1 000 bytes); the allowance left noted
- **When** Alice copies `/docs` to `/docs-copy`
- **Then** the allowance left is lower by 4 000 bytes (the files themselves charge nothing for a folder copy, as before), and both sources still give their artifacts
- **Priority:** Medium

### SPEC-0105/E2E-062: a folder copied onto an existing folder with overwrite
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_062_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-004
- **Given** `/src` holds D converted and `/dst` holds `/dst/report.pdf` converted (1 image captioned `Old`) and `/dst/keep.pdf` converted (1 image captioned `Keep`)
- **When** Alice copies `/src` onto `/dst` with overwrite
- **Then** `/dst/report.pdf` gives D's artifacts and none of `Old`, `/dst/keep.pdf` still gives `Keep`
- **Priority:** Medium

### SPEC-0105/E2E-068: a folder copied into itself
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_068_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-004, FR-NEW-006
- **Given** `/docs` holds D converted
- **When** Alice copies `/docs` to `/docs/inner` with overwrite
- **Then** `/docs/inner/report.pdf` gives D's artifacts once, `/docs/report.pdf` still gives its own, and no document is copied twice
- **Priority:** Medium

## Constraints
- Files of a folder copy keep charging nothing to the session.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0308.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0307: copy a folder of converted documents` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0105` and the requirement ids.
