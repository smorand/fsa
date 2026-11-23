# US-0306: copy a converted document with its artifacts

> Spec: SPEC-0105 (sdlc/specs/SPEC-0105-document-artifacts-follow-their-document/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0302
> min_tier: 1
> Files touched: 6
> Closes spec: no

## Objective
A copied converted document gets its own artifacts, charged to the session allowance with its file, refused whole when they do not fit, and the source keeps its own. A copy over a document replaces its set, and a file copied onto itself keeps its set.

## Requirements covered
- SPEC-0105/FR-NEW-003: WHEN a member copies a converted document THE Documents context SHALL give the copy its own artifacts, equal to those of the source, AND SHALL leave the artifacts of the source unchanged.
- SPEC-0105/FR-NEW-005 (partial: single file): IF the weight of the artifacts to copy, added to the files, exceeds what remains of the session write allowance THEN THE Documents context SHALL refuse the copy and keep nothing of it, not the files nor the artifacts.
- SPEC-0105/FR-NEW-006 (partial: copy over, copy onto itself): WHEN a member moves or copies a document onto an existing document with overwrite THE Documents context SHALL replace the destination's artifacts by those of the incoming document, AND the destination SHALL have no artifacts when the incoming document has none.
- SPEC-0105/FR-MOD-002: WHEN a converted document is copied THE Documents context SHALL give the copy its own artifacts under FR-NEW-003.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0105_ tools::artifacts quota::`. Tests copy D with room, with exactly enough (3 600 bytes) and with 3 500 bytes (refused, nothing created), over a converted and a plain document, onto itself, to a new folder and with accents, edit and convert the copy, and read the allowance by exact writes.

## Design excerpt
### Decisions
- DEC-007 (SPEC-0105): The weight of a set is computed in Rust from its stored rows with the same formula as `PendingSet::bytes()`: per image `size + caption + ocr_text`, per table `csv_text` (or the CSV rendered from the cells) `+ Markdown + caption`, the heading titles, and `text_len`; `artifact_set_weight` reads the rows and `docs::artifacts::weight::of_rows` applies the formula. **Obliged by:** SPEC-0105/FR-NEW-003. **Rationale:** a copy is charged what the conversion was charged, with no new column. **Alternatives considered:** a stored weight column (schema change, null for old sets). **Source:** existing code. **Evidence:** `docs/artifacts/capture.rs:197-212`.
- DEC-008 (SPEC-0105): `copy_artifact_sets(pairs)` runs in one transaction: for each pair with a current set it inserts a new set (new id, destination path, the destination node's revision, `created_at` now) and copies the image, table and heading rows with `INSERT ... SELECT`, adds one reference per image blob and one for the text blob, marks the unsuperseded sets at the destination `superseded_at`, and upserts the destination pointer. **Obliged by:** SPEC-0105/FR-NEW-003, FR-NEW-004, FR-NEW-006. **Rationale:** equal independent sets (editing one hides only its own). **Alternatives considered:** share the set between paths (an edit of one would hide both). **Source:** existing code. **Evidence:** `storage/meta.rs:1005-1130`, `:1609-1660`.
- DEC-009 (SPEC-0105): Admission before writing: a single file copy calls `quota::admit_file_with(..., extra_session = weight)`, a folder copy calls `admit_paths(session_bytes = total weight, ...)`; both refuse before anything is written, so nothing is kept (FR-NEW-005). If `copy_artifact_sets` fails after the files were written, the copied files are removed and the session charge refunded; a failure after an overwrite is reported as an internal error. **Obliged by:** SPEC-0105/FR-NEW-005, FR-NEW-010. **Rationale:** the allowance check is the only expected refusal. **Alternatives considered:** write the sets inside `put_file` (couples the volume to documents). **Source:** existing code. **Evidence:** `quota.rs:47-62,137-160`, `core/fs_ops.rs:1352-1385`.
- DEC-010 (SPEC-0105): A file copied onto itself with overwrite skips the rewrite, is charged as a copy and refreshes the modification time; a folder copy into itself works from its snapshot, so each document is copied once. **Obliged by:** SPEC-0105/FR-NEW-006. **Rationale:** rewriting would renew the revision and hide the artifacts. **Alternatives considered:** refuse it (the spec chose to allow). **Source:** spec. **Evidence:** `core/fs_ops.rs:1352-1385`, `storage/volume.rs:277-295`.
- DEC-014 (SPEC-0105): Each story updates the existing SPEC-0019 tests of its operation to the new rule, naming them in its commit: 088 and 097 (US-0302), 095 (US-0302), 090 (US-0304), 089, 093, 099 and `a_copy_is_charged_the_file_bytes_only` (US-0306). **Obliged by:** SPEC-0105/FR-MOD-001, FR-MOD-002. **Rationale:** the old expectation is the behavior this spec removes. **Alternatives considered:** leaving them red. **Source:** spec. **Evidence:** `tools/artifacts.rs:3859-4330`.
### Data model
No schema change. `copy_artifact_sets` inserts one new set per copied document (new `set_id`, `path` = destination, `node_rev` = the destination node's revision) with copies of its `doc_images`, `doc_tables` and `doc_headings` rows and a blob reference per image and for the text.
### Contracts
`RelationalMetaStore::artifact_set_weight(path) -> Result<i64>` (0 with no current set), `RelationalMetaStore::copy_artifact_sets(pairs) -> Result<usize>`, `quota::admit_file_with(client, safety, person, mount_id, path, new_size, extra_session)`, `docs::artifacts::weight::of_rows`. `copy_path` single file: compute the weight, `admit_file_with(... len, weight)`, write the file, `copy_artifact_sets([(src, dst)])`; on its failure remove the written file and refund. File onto itself: no rewrite, same charge. Refusal: `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded` before anything is written.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/meta.rs` | modify: `artifact_set_weight`, `copy_artifact_sets` |
| `crates/core/src/docs/artifacts/weight.rs`, `crates/core/src/docs/artifacts/mod.rs` | create / modify: weight formula |
| `crates/core/src/quota.rs` | modify: `admit_file_with` |
| `crates/core/src/core/fs_ops.rs` | modify: single file copy |
| `crates/core/src/tools/artifacts.rs` | modify: existing tests 089, 093, 099, `a_copy_is_charged_the_file_bytes_only` (E2E-MOD-002, MOD-005, MOD-007) |
| `crates/core/src/tools/artifacts_follow_e2e.rs` | modify: tests |
### Existing patterns
`PendingSet::bytes()` `docs/artifacts/capture.rs:197-212`; `admit_file` `quota.rs:47-62`; blob refcounts `storage/meta.rs:1609-1660`; commit of a set `storage/meta.rs:1005-1130`; single file copy `core/fs_ops.rs:1376-1385`. Tests in `crates/core/src/tools/artifacts_follow_e2e.rs` reuse the SPEC-0103 stand ins in `docs/manifest_e2e.rs` (`program()`, bundle helpers, `old_set`, the shared `atlas` fixture) and the SPEC-0104 helpers in `tools/read_metadata_e2e.rs`; they read artifacts through `fs.list_images`, `fs.list_tables`, `fs.get_document_structure` and `fs.get_document_view`, move with `fs.move`, copy with `fs.copy`, delete with `fs.delete`, restore with `fs.trash_restore`; the sweep is run with an explicit now.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0105/E2E-005: a converted document is copied
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_005_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003, FR-MOD-002
- **Given** D converted; the session write allowance has 100 000 bytes left
- **When** Alice copies `/docs/report.pdf` to `/docs/report-copy.pdf`
- **Then** `/docs/report-copy.pdf` gives the same 2 images, 1 table and 2 headings as D, and D still gives its own
- **And** the allowance left is lower by the file's own size plus the 3 000 bytes of D's artifacts
- **Priority:** Critical

### SPEC-0105/E2E-010: overwrite by copy
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_010_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** D converted and `/docs/old.pdf` converted with 1 image captioned `Old`
- **When** Alice copies `/docs/report.pdf` onto `/docs/old.pdf` with overwrite
- **Then** `/docs/old.pdf` gives D's artifacts, none of `Old`; D still gives its own
- **Priority:** High

### SPEC-0105/E2E-012: the source of a copy keeps its artifacts
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_012_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003, FR-MOD-002
- **Given** D converted and copied to `/docs/c.pdf`
- **When** Alice edits `/docs/c.pdf`, then reads the artifacts of both
- **Then** `/docs/c.pdf` has no artifacts (the edit discards them as before) and D still gives all of its own
- **Priority:** High

### SPEC-0105/E2E-018: a copy that does not fit the session allowance
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_018_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** D converted (file of 600 bytes, artifacts of 3 000 bytes); the allowance left is 3 500 bytes
- **When** Alice copies `/docs/report.pdf` to `/docs/c.pdf`
- **Then** the copy is refused with `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded`, `/docs/c.pdf` does not exist
- **And** D still gives its artifacts
- **And** the allowance left is still 3 500 bytes
- **Priority:** Critical

### SPEC-0105/E2E-024: a copy into a folder that does not exist yet
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_024_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** D converted and no folder `/nowhere`
- **When** Alice copies D to `/nowhere/c.pdf`
- **Then** the copy succeeds, `/nowhere` is created and `/nowhere/c.pdf` gives D's artifacts, charged as in E2E-005
- **Priority:** Medium

### SPEC-0105/E2E-037: names with accents and spaces
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_037_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-003
- **Given** D converted
- **When** Alice moves it to `/Archives été/Rapport n° 1.pdf` and copies it to `/Archives été/Copie 🌐.pdf`
- **Then** both give D's artifacts
- **Priority:** Medium

### SPEC-0105/E2E-041: a converted copy converts again on its own
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_041_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** D copied to `/c.pdf` and both converted again by a converter returning 1 image captioned `New`
- **When** Alice converts `/c.pdf` again
- **Then** `/c.pdf` gives only `New`; D still gives its own previous artifacts
- **Priority:** Medium

### SPEC-0105/E2E-049: OCR text and headings follow
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_049_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012
- **Given** D converted
- **When** Alice moves and copies D
- **Then** the moved and the copied document give the OCR text `Revenue 4.2 M EUR` and the headings `Report` and `Costs`
- **Priority:** Medium

### SPEC-0105/E2E-050: a second copy of a copy
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_050_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** D copied to `/c1.pdf`
- **When** Alice copies `/c1.pdf` to `/c2.pdf`
- **Then** `/c2.pdf` gives D's artifacts and the allowance is charged for each copy
- **Priority:** Low

### SPEC-0105/E2E-051: a copy is charged to the allowance, a move is not
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_051_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003, FR-NEW-001, FR-NEW-012
- **Given** D converted; the allowance left noted
- **When** Alice moves D, then copies D
- **Then** the move leaves the allowance left unchanged and the copy lowers it by the file's size plus the 3 000 bytes of artifacts
- **Priority:** High

### SPEC-0105/E2E-052: project storage is not charged for artifacts
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_052_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** D converted; the project's used storage noted
- **When** Alice copies D
- **Then** the used storage grows by the copied file's size only
- **Priority:** High

### SPEC-0105/E2E-059: a copy that fits the allowance exactly
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_059_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-005
- **Given** D converted (file of 600 bytes, artifacts of 3 000 bytes); the allowance left is exactly 3 600 bytes
- **When** Alice copies D
- **Then** the copy succeeds with its artifacts, and a further write of 1 byte is refused for quota
- **Priority:** Medium

### SPEC-0105/E2E-067: a file copied onto itself with overwrite
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_067_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-003
- **Given** D converted; the allowance left noted
- **When** Alice copies `/docs/report.pdf` onto `/docs/report.pdf` with overwrite
- **Then** D still gives all its artifacts and the allowance is charged as for a copy (file size plus 3 000 bytes)
- **Priority:** Medium

### SPEC-0105/E2E-069: a copy onto an existing document without overwrite
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_069_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-010
- **Given** D and `/docs/old.pdf` (1 image captioned `Old`) converted
- **When** Alice copies D onto `/docs/old.pdf` without overwrite
- **Then** the copy is refused with `ERR_NO_CLOBBER: '/docs/old.pdf' exists (pass overwrite=true)`; both documents keep their own artifacts and nothing is charged
- **Priority:** High

### SPEC-0105/E2E-MOD-002: a copy no longer starts with none (was SPEC-0019 test 089)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_mod002_*`)
- **Validated before:** after a converted document is copied the artifacts are gone or the old rule applies.
- **Validates now:** after a converted document is copied the artifacts follow as FR-NEW-003 says (the copy has its own equal set).
- **Scenario:** SC-003
- **Requirements:** FR-MOD-002
- **Given** D converted
- **When** A converted document is copied by Alice
- **Then** reading the artifacts at the copy gives those of the source, and the source still gives its own

### SPEC-0105/E2E-MOD-005: a copy over a document replaces its artifacts (was SPEC-0019 test 093)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_mod005_*`)
- **Validated before:** after D is copied over `/docs/old.pdf` with overwrite the artifacts are gone or the old rule applies.
- **Validates now:** after D is copied over `/docs/old.pdf` with overwrite the artifacts follow as FR-NEW-006 says (the destination takes the incoming artifacts).
- **Scenario:** SC-004
- **Requirements:** FR-MOD-002
- **Given** D converted
- **When** D is copied over `/docs/old.pdf` with overwrite by Alice
- **Then** the destination gives the artifacts of the incoming document and none of its own previous ones

### SPEC-0105/E2E-MOD-007: a copy is charged the file and its artifacts (was SPEC-0019 test 099 and a_copy_is_charged_the_file_bytes_only)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_mod007_*`)
- **Validated before:** after D is copied the artifacts are gone or the old rule applies.
- **Validates now:** after D is copied the artifacts follow as FR-NEW-003 says (the copy is charged the file and its artifacts).
- **Scenario:** SC-003
- **Requirements:** FR-MOD-002
- **Given** D converted
- **When** D is copied by Alice
- **Then** the allowance left is lower by the file size plus the weight of the artifacts (before: by the file size only)

## Constraints
- The folder copy is US-0307.
- Project storage is never charged for artifacts.
- No schema change.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0308.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0306: copy a converted document with its artifacts` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0105` and the requirement ids.
