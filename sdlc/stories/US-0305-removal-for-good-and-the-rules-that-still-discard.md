# US-0305: removal for good and the rules that still discard

> Spec: SPEC-0105 (sdlc/specs/SPEC-0105-document-artifacts-follow-their-document/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0304
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
Prove and pin that a hard deleted document and a removed project lose their artifacts (the sweep removes the sets and releases the blobs), that an edit still discards, and that a refused hard delete or edit changes nothing.

## Requirements covered
- SPEC-0105/FR-NEW-009: WHEN a converted document is hard deleted, or its project is removed for good, THE Documents context SHALL remove its artifacts.
- SPEC-0105/FR-MOD-001 (partial: edit, overwrite, permanent deletion still discard): WHEN a converted document is replaced, edited, overwritten by a plain file, extracted over by an archive or deleted for good THE Documents context SHALL discard its artifacts and its conversion text, AND SHALL keep them when it is moved, renamed or trashed.
- SPEC-0105/FR-NEW-010 (partial: refused edit or hard delete keeps the artifacts): A refused or failed move, copy or restore SHALL NOT change the artifacts of any document, those of the source, of the destination and of the trash item.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0105_ purge::`. Tests hard delete D (server option on), run the artifact sweep and count the rows, remove and recreate a project past its grace, edit D after a move, refuse a hard delete with the server option off, and refuse an edit over the allowance.

## Design excerpt
### Decisions
- DEC-005 (SPEC-0105): Removal for good relies on the existing sweep: a hard delete removes the node, so the next sweep deletes its current set without grace and releases its blobs; project removal clears the tables with the volume. A test runs the sweep and asserts the rows are gone. **Obliged by:** SPEC-0105/FR-NEW-009. **Rationale:** one place releases blobs. **Alternatives considered:** delete in the delete transaction (a second blob release path). **Source:** existing code. **Evidence:** `storage/meta.rs:1176-1184`, `purge.rs:262,306-310`, `storage/mod.rs:522-528`.
- DEC-014 (SPEC-0105): Each story updates the existing SPEC-0019 tests of its operation to the new rule, naming them in its commit: 088 and 097 (US-0302), 095 (US-0302), 090 (US-0304), 089, 093, 099 and `a_copy_is_charged_the_file_bytes_only` (US-0306). **Obliged by:** SPEC-0105/FR-MOD-001, FR-MOD-002. **Rationale:** the old expectation is the behavior this spec removes. **Alternatives considered:** leaving them red. **Source:** spec. **Evidence:** `tools/artifacts.rs:3859-4330`.
### Data model
No schema change.
### Contracts
No new contract. Existing refusals: `ERR_NOT_SUPPORTED: hard delete disabled (server started without allow_hard_delete)`, `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded`. If a test finds a set surviving the sweep after a hard delete of a trash path, fix `sweep_stale_artifact_sets` here and say so.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/tools/artifacts_follow_e2e.rs` | modify: tests |
| `crates/core/src/purge.rs` | modify only if a test seam for the explicit now is missing |
| `crates/core/src/storage/meta.rs` | modify only if the sweep needs a fix |
### Existing patterns
Sweep `storage/meta.rs:1167-1260`, `purge.rs:262,306-310`; hard delete `core/fs_ops.rs:1250-1266`; project removal `storage/mod.rs:522-528`. Tests in `crates/core/src/tools/artifacts_follow_e2e.rs` reuse the SPEC-0103 stand ins in `docs/manifest_e2e.rs` (`program()`, bundle helpers, `old_set`, the shared `atlas` fixture) and the SPEC-0104 helpers in `tools/read_metadata_e2e.rs`; they read artifacts through `fs.list_images`, `fs.list_tables`, `fs.get_document_structure` and `fs.get_document_view`, move with `fs.move`, copy with `fs.copy`, delete with `fs.delete`, restore with `fs.trash_restore`; the sweep is run with an explicit now.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0105/E2E-011: a hard deleted document loses its artifacts
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_011_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-009
- **Given** D converted; the server allows hard delete
- **When** Alice hard deletes `/docs/report.pdf`, then writes a new plain file at `/docs/report.pdf`
- **Then** the trash does not list D, and reading the images of the new file is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Priority:** High

### SPEC-0105/E2E-040: an edit after a move discards as before
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_040_*`)
- **Scenario:** SC-007
- **Requirements:** FR-MOD-001
- **Given** D converted and moved to `/archive/report.pdf`
- **When** Alice replaces its bytes with a new version
- **Then** the artifacts are gone (`ERR_NOT_FOUND: No artifacts: document not extracted`)
- **Priority:** High

### SPEC-0105/E2E-046: a project soft deleted and restored
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_046_*`)
- **Scenario:** SC-007
- **Requirements:** FR-MOD-001
- **Given** D converted; the platform administrator removes and restores the project
- **When** Alice reads D's images
- **Then** D gives its artifacts as before (existing behavior, unchanged)
- **Priority:** Medium

### SPEC-0105/E2E-058: removing the project removes the artifacts
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_058_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-009
- **Given** D converted; the platform administrator removes `proj` and its grace period passes with the removal purge
- **When** an administrator recreates a project `proj` and Alice writes `/docs/report.pdf`
- **Then** reading the images of the new file is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`; nothing of D remains
- **Priority:** Medium

### SPEC-0105/E2E-063: a hard delete that the server refuses keeps everything
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_063_*`)
- **Scenario:** SC-006
- **Requirements:** FR-NEW-009, FR-NEW-010
- **Given** D converted; the server does not allow hard delete
- **When** Alice hard deletes D
- **Then** she is refused with `ERR_NOT_SUPPORTED: hard delete disabled (server started without allow_hard_delete)` and D still gives its artifacts
- **Priority:** Medium

### SPEC-0105/E2E-070: an edit refused for quota keeps the artifacts
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_070_*`)
- **Scenario:** SC-007
- **Requirements:** FR-MOD-001, FR-NEW-010
- **Given** D converted; the allowance left is 10 bytes
- **When** Alice replaces D's bytes with a 1 000 byte file
- **Then** the write is refused with `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded` and D still gives its artifacts
- **Priority:** Medium

## Constraints
- Production change only when a test proves a defect.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0308.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0305: removal for good and the rules that still discard` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0105` and the requirement ids.
