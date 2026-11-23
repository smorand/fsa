# US-0304: trash and restore keep the artifacts

> Spec: SPEC-0105 (sdlc/specs/SPEC-0105-document-artifacts-follow-their-document/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0303
> min_tier: 1
> Files touched: 5
> Closes spec: no

## Objective
Keep the artifacts of a deleted document or folder with the trashed item, including when the retention sweep trashes it, give them back on restore (renamed on collision), and refuse reading them at the trash location.

## Requirements covered
- SPEC-0105/FR-NEW-007: WHEN a member deletes a converted document or a folder holding converted documents THE Documents context SHALL keep their artifacts with the item in the trash.
- SPEC-0105/FR-NEW-008: WHEN a member restores a trashed document or folder THE Documents context SHALL give the restored documents their artifacts at the path where each is restored.
- SPEC-0105/FR-MOD-001 (partial: trashed no longer discards): WHEN a converted document is replaced, edited, overwritten by a plain file, extracted over by an archive or deleted for good THE Documents context SHALL discard its artifacts and its conversion text, AND SHALL keep them when it is moved, renamed or trashed.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0105_ tools::trash tools::artifacts`. Tests delete D and a folder, read at the old path and the trash path (refused), restore, restore after a collision (`/docs/report.pdf_restored`), restore twice, and run the retention sweep.

## Design excerpt
### Decisions
- DEC-003 (SPEC-0105): `rename_into_trash` keeps deleting chunk sets but passes `carry_artifacts = true`; `restore` already goes through `rename`. **Obliged by:** SPEC-0105/FR-NEW-007, FR-NEW-008. **Rationale:** the trash path then holds the set with the new revision, so the sweep keeps it while the node exists. **Alternatives considered:** a separate trash table (a second store). **Source:** existing code. **Evidence:** `storage/meta.rs:2523-2538`, `tools/trash.rs:229-249`.
- DEC-004 (SPEC-0105): `current_set` in `tools/artifacts.rs` refuses a path under the trash directory with `No artifacts: document not extracted`, using `safety.config().trash_dir`. **Obliged by:** SPEC-0105/FR-NEW-007. **Rationale:** the artifacts of a trashed item must not be readable before the restore. **Alternatives considered:** a column marking trashed sets. **Source:** spec. **Evidence:** `tools/artifacts.rs:291-296`, `tools/trash.rs:137`.
- DEC-014 (SPEC-0105): Each story updates the existing SPEC-0019 tests of its operation to the new rule, naming them in its commit: 088 and 097 (US-0302), 095 (US-0302), 090 (US-0304), 089, 093, 099 and `a_copy_is_charged_the_file_bytes_only` (US-0306). **Obliged by:** SPEC-0105/FR-MOD-001, FR-MOD-002. **Rationale:** the old expectation is the behavior this spec removes. **Alternatives considered:** leaving them red. **Source:** spec. **Evidence:** `tools/artifacts.rs:3859-4330`.
### Data model
No schema change.
### Contracts
`rename_into_trash` calls `tx_rename(..., false, true)` after deleting the chunk sets; `restore` unchanged (it uses `rename`). `tools::artifacts::current_set` takes the trash directory and answers `ERR_NOT_FOUND: No artifacts: document not extracted` for a path under `/<trash_dir>/`. Existing texts: `ERR_NOT_FOUND: '<trash_path>' is not a trashed entry`, `ERR_NO_CLOBBER: '/docs' already exists and is not a directory`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/meta.rs` | modify: `rename_into_trash` |
| `crates/core/src/tools/artifacts.rs` | modify: trash path refusal in the readers; existing test 090 to the new rule (E2E-MOD-003) |
| `crates/core/src/tools/artifacts_follow_e2e.rs` | modify: tests |
### Existing patterns
`rename_into_trash` `storage/meta.rs:2523-2538`; restore `tools/trash.rs:195-249`; trash root `tools/trash.rs:137`; retention sweep `purge.rs:57-125`. Tests in `crates/core/src/tools/artifacts_follow_e2e.rs` reuse the SPEC-0103 stand ins in `docs/manifest_e2e.rs` (`program()`, bundle helpers, `old_set`, the shared `atlas` fixture) and the SPEC-0104 helpers in `tools/read_metadata_e2e.rs`; they read artifacts through `fs.list_images`, `fs.list_tables`, `fs.get_document_structure` and `fs.get_document_view`, move with `fs.move`, copy with `fs.copy`, delete with `fs.delete`, restore with `fs.trash_restore`; the sweep is run with an explicit now.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0105/E2E-007: trash then restore
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_007_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** D converted
- **When** Alice deletes `/docs/report.pdf` (it goes to the trash) then restores it from the trash
- **Then** `/docs/report.pdf` gives its images, table, headings and view as before the deletion
- **And** between the deletion and the restore, reading the images of `/docs/report.pdf` is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **Priority:** Critical

### SPEC-0105/E2E-008: restore under a new name after a collision
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_008_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008
- **Given** D converted; Alice deleted it; she then created a plain file `/docs/report.pdf` with the text `new`
- **When** Alice restores the trashed document
- **Then** it comes back as `/docs/report.pdf_restored` and gives D's artifacts; the plain file `/docs/report.pdf` still reads `new` and has no artifacts
- **Priority:** High

### SPEC-0105/E2E-016: restore of an unknown trash item
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_016_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008
- **Given** a trash holding no item at `/.mcp_trash/missing`
- **When** Alice restores the trash item `/.mcp_trash/missing`
- **Then** the restore is refused with `ERR_NOT_FOUND: '/.mcp_trash/missing' is not a trashed entry`
- **Priority:** Medium

### SPEC-0105/E2E-021: restoring the same item twice
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_021_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008, FR-NEW-010
- **Given** D trashed and restored once
- **When** Alice restores the same trash item again
- **Then** the second restore is refused with `ERR_NOT_FOUND: '<trash path of D>' is not a trashed entry`, and D still gives its artifacts at the restored path
- **Priority:** Medium

### SPEC-0105/E2E-023: reading the old path after a trash
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_023_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** D trashed
- **When** Alice reads the images, tables, structure and view of `/docs/report.pdf`
- **Then** each is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **And** the artifacts of the trashed item are readable only after its restore: reading them at the trash location is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Priority:** Medium

### SPEC-0105/E2E-025: a restore of a document whose folder was removed
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_025_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008, FR-NEW-010
- **Given** D trashed, then `/docs` replaced by a plain file named `/docs`
- **When** Alice restores D
- **Then** the restore is refused with `ERR_NO_CLOBBER: '/docs' already exists and is not a directory`, and D stays in the trash with its artifacts
- **Priority:** Medium

### SPEC-0105/E2E-028: a non member cannot read the artifacts of a trashed document
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_028_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** D trashed; Eve is not a member
- **When** Eve reads the images of `/docs/report.pdf`
- **Then** she is refused with `ERR_FORBIDDEN: 'eve@example.com' is not a member of 'proj'`
- **Priority:** Medium

### SPEC-0105/E2E-042: another file at the old path of a trashed document
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_042_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-011
- **Given** D trashed and a plain file created at `/docs/report.pdf`
- **When** Alice reads its images
- **Then** `ERR_NOT_FOUND: No artifacts: document not extracted`; the trashed D still returns its artifacts after a restore under another name
- **Priority:** High

### SPEC-0105/E2E-043: restore after the original path was reused
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_043_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008
- **Given** D trashed; a different converted document `/docs/report.pdf` created (1 image captioned `Other`)
- **When** Alice restores D
- **Then** D comes back as `/docs/report.pdf_restored` with its own 2 images; `/docs/report.pdf` still gives `Other`
- **Priority:** High

### SPEC-0105/E2E-044: a folder trashed and restored
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_044_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** `/docs` with D and D2 converted
- **When** Alice deletes `/docs` and restores it
- **Then** both documents give their artifacts at their original paths
- **Priority:** High

### SPEC-0105/E2E-045: a document trashed by the retention sweep keeps its artifacts
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_045_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** D converted and not read for longer than the file retention, so the automatic sweep moves it to the trash
- **When** Alice restores D from the trash
- **Then** D comes back at `/docs/report.pdf` with its artifacts
- **Priority:** Medium

### SPEC-0105/E2E-053: the trash listing is unchanged
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_053_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** D converted
- **When** Alice deletes D and lists the trash
- **Then** the listing shows the same fields as before this change (trash path, original path, size, kind, deletion time, deleted by, days to purge)
- **Priority:** Medium

### SPEC-0105/E2E-064: a stale file trashed by the retention sweep and a member's delete are alike
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_064_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** D and D2 converted; D deleted by Alice, D2 trashed by the retention sweep
- **When** Alice restores both
- **Then** both come back with their artifacts
- **Priority:** Medium

### SPEC-0105/E2E-MOD-003: trash and restore keep the artifacts (was SPEC-0019 test 090)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_mod003_*`)
- **Validated before:** after a document is trashed and restored the artifacts are gone or the old rule applies.
- **Validates now:** after a document is trashed and restored the artifacts follow as FR-NEW-007 and FR-NEW-008 say (they stay with the trashed item and return on restore).
- **Scenario:** SC-005
- **Requirements:** FR-MOD-001
- **Given** D converted
- **When** A document is trashed and restored by Alice
- **Then** after the restore the artifacts are at the restored path; while in the trash the original path is refused as not a file

## Constraints
- No change to the trash listing fields.
- Do not delete trashed sets; hard delete is US-0305.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/tools.md` at US-0308.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0304: trash and restore keep the artifacts` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0105` and the requirement ids.
