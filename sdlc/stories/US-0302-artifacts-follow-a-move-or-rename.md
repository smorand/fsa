# US-0302: artifacts follow a moved or renamed document or folder

> Spec: SPEC-0105 (sdlc/specs/SPEC-0105-document-artifacts-follow-their-document/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 1
> Files touched: 5
> Closes spec: no

## Objective
Make a move or rename of a converted document, or of a folder holding converted documents, keep every artifact at the new path and none at the old, with no conversion and nothing created for plain files. Creates the test module.

## Requirements covered
- SPEC-0105/FR-NEW-001: WHEN a member moves or renames a converted document THE Documents context SHALL keep its artifacts and its conversion text at the new path and none at the old path.
- SPEC-0105/FR-NEW-002: WHEN a member moves or renames a folder THE Documents context SHALL keep the artifacts of every converted document in it, at any depth, at the new path of each.
- SPEC-0105/FR-NEW-011 (partial: move and rename of a plain file): A move, copy, trash or restore of a file that has no artifacts SHALL NOT give it or its copy any artifacts.
- SPEC-0105/FR-MOD-001 (partial: moved or renamed no longer discards): WHEN a converted document is replaced, edited, overwritten by a plain file, extracted over by an archive or deleted for good THE Documents context SHALL discard its artifacts and its conversion text, AND SHALL keep them when it is moved, renamed or trashed.
- SPEC-0105/FR-NEW-010 (partial: a refused move changes nothing): A refused or failed move, copy or restore SHALL NOT change the artifacts of any document, those of the source, of the destination and of the trash item.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0105_ tools::artifacts storage::`. Tests convert document D, move and rename it, read its images, tables, structure and view at the new path (and refuse the old), run the artifact sweep and read again, and move folders of 1, 3 and 100 converted documents.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0105): All tests live in `tools/artifacts_follow_e2e.rs` and reuse the SPEC-0103 stand ins. **Obliged by:** SPEC-0105/FR-NEW-001 (tests of every requirement). **Rationale:** one module, one run command; `tools/artifacts.rs` is 4 400 lines. **Alternatives considered:** extending `tools/artifacts.rs` tests. **Source:** design. **Evidence:** `docs/manifest_e2e.rs`, `tools/read_metadata_e2e.rs`.
- DEC-002 (SPEC-0105): `tx_rename` takes a second flag `carry_artifacts`. For each moved node, with its new revision `r`: delete the pointer row at the new path, then `UPDATE doc_artifact_sets SET path=new, node_rev=r WHERE path=old AND superseded_at IS NULL`, then `UPDATE doc_artifact_current SET path=new, node_rev=r WHERE path=old`, all inside the existing loop and transaction. `rename` passes true; sets left at an overwritten destination keep their old revision and are swept. **Obliged by:** SPEC-0105/FR-NEW-001, FR-NEW-002, FR-NEW-006. **Rationale:** a move already rewrites revisions per node and carries chunk sets the same way; one transaction means all or nothing (FR-NEW-010). **Alternatives considered:** key by node id (schema and reader change); update the sets at the call site (not atomic with the rename). **Source:** existing code. **Evidence:** `storage/meta.rs:1841-1900`, `:1176-1184`, `:1277-1290`.
- DEC-012 (SPEC-0105): No schema change and no new dependency. **Obliged by:** SPEC-0105/FR-NEW-012. **Rationale:** the data already carries path and revision. **Alternatives considered:** a weight column. **Source:** DEC-002, DEC-007. **Evidence:** `storage/meta.rs:233-310`.
- DEC-014 (SPEC-0105): Each story updates the existing SPEC-0019 tests of its operation to the new rule, naming them in its commit: 088 and 097 (US-0302), 095 (US-0302), 090 (US-0304), 089, 093, 099 and `a_copy_is_charged_the_file_bytes_only` (US-0306). **Obliged by:** SPEC-0105/FR-MOD-001, FR-MOD-002. **Rationale:** the old expectation is the behavior this spec removes. **Alternatives considered:** leaving them red. **Source:** spec. **Evidence:** `tools/artifacts.rs:3859-4330`.
### Data model
No schema change. A move rewrites `path` and `node_rev` of the unsuperseded `doc_artifact_sets` row and of the `doc_artifact_current` pointer of each moved node, in the transaction of the rename.
### Contracts
`tx_rename(..., carry_sets, carry_artifacts)` (new last flag; `rename` passes true, `rename_into_trash` false until US-0304). Order in the per node loop: delete the pointer at the new path, update the unsuperseded sets at the old path (path, node_rev = the node's new revision), update the pointer. Existing refusals stay: `ERR_NOT_FOUND: '<src>' does not exist`, `ERR_NO_CLOBBER: '<dst>' exists (pass overwrite=true)`, `ERR_NO_CLOBBER: '<parent>' already exists and is not a directory`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/storage/meta.rs` | modify: `tx_rename` carries artifacts, tests of the sweep after a move |
| `crates/core/src/tools/artifacts.rs` | modify: existing tests 088, 095, 097 to the new rule (E2E-MOD-001, MOD-004, MOD-006) |
| `crates/core/src/tools/artifacts_follow_e2e.rs`, `crates/core/src/tools/mod.rs` | create / modify: test module and registration |
### Existing patterns
Chunk sets carried in the same loop `storage/meta.rs:1893-1903`; the sweep join `storage/meta.rs:1176-1184`; reader join `:1277-1290`; the SPEC-0019 tests to change `tools/artifacts.rs` (`spec_0019_e2e_088`, `095`, `097`). Tests in `crates/core/src/tools/artifacts_follow_e2e.rs` reuse the SPEC-0103 stand ins in `docs/manifest_e2e.rs` (`program()`, bundle helpers, `old_set`, the shared `atlas` fixture) and the SPEC-0104 helpers in `tools/read_metadata_e2e.rs`; they read artifacts through `fs.list_images`, `fs.list_tables`, `fs.get_document_structure` and `fs.get_document_view`, move with `fs.move`, copy with `fs.copy`, delete with `fs.delete`, restore with `fs.trash_restore`; the sweep is run with an explicit now.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0105/E2E-001: a converted document is moved
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_001_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-MOD-001
- **Given** project `proj`, Alice (`alice@example.com`) a member, D converted
- **When** Alice moves `/docs/report.pdf` to `/archive/report.pdf`
- **Then** reading `/archive/report.pdf` gives the 2 images (`image-1` caption `Figure 1`, OCR text `Revenue 4.2 M EUR`), the table `table-1` captioned `Costs`, the headings `Report` and `Costs` and the full view, all as before the move
- **And** reading the images of `/docs/report.pdf` is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **And** the document was not converted again
- **Priority:** Critical

### SPEC-0105/E2E-002: a converted document is renamed in place
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_002_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** D converted
- **When** Alice renames `/docs/report.pdf` to `/docs/report-2026.pdf`
- **Then** `/docs/report-2026.pdf` gives the same artifacts as before, and the old name is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **Priority:** High

### SPEC-0105/E2E-003: a folder holding converted documents is moved
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_003_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002, FR-MOD-001
- **Given** `/docs/report.pdf` (D) and `/docs/sub/other.pdf` (D2, one image captioned `Chart`) both converted
- **When** Alice moves `/docs` to `/library`
- **Then** `/library/report.pdf` and `/library/sub/other.pdf` give the artifacts they had, and nothing is readable under `/docs`
- **Priority:** High

### SPEC-0105/E2E-004: the conversion text follows without a converter call
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_004_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012, FR-NEW-001
- **Given** D converted through a converter that counts its calls (1 call so far)
- **When** Alice moves D, then reads its extracted text and its view
- **Then** both answers equal those given before the move, and the converter was called 1 time in total
- **Priority:** High

### SPEC-0105/E2E-013: a move onto an existing folder without overwrite
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_013_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D converted; `/archive` is an existing folder
- **When** Alice moves `/docs/report.pdf` onto `/archive` without overwrite
- **Then** the move is refused with `ERR_NO_CLOBBER: '/archive' exists (pass overwrite=true)`; D gives its artifacts at `/docs/report.pdf`
- **Priority:** High

### SPEC-0105/E2E-014: a move of a document that does not exist
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_014_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** no file `/docs/ghost.pdf`
- **When** Alice moves `/docs/ghost.pdf` to `/archive/ghost.pdf`
- **Then** the move is refused with `ERR_NOT_FOUND: '/docs/ghost.pdf' does not exist` and no artifact appears at `/archive/ghost.pdf`
- **Priority:** Medium

### SPEC-0105/E2E-026: a move under a plain file
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_026_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D converted and a plain file `/flat.txt`
- **When** Alice moves D to `/flat.txt/report.pdf`
- **Then** the move is refused with `ERR_NO_CLOBBER: '/flat.txt' already exists and is not a directory`; D keeps its artifacts
- **Priority:** Medium

### SPEC-0105/E2E-027: the same move twice
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_027_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-010
- **Given** D converted and moved to `/archive/report.pdf`
- **When** Alice repeats the move of `/docs/report.pdf`
- **Then** she is refused with the missing source refusal and `/archive/report.pdf` still gives D's artifacts
- **Priority:** Medium

### SPEC-0105/E2E-031: a plain file moved and copied
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_031_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-011
- **Given** `/notes.txt` never converted
- **When** Alice moves it to `/n.txt` and copies it to `/m.txt`
- **Then** reading the images of either is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`; no artifact was created
- **Priority:** High

### SPEC-0105/E2E-032: an empty folder moved
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_032_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002, FR-NEW-011
- **Given** an empty folder `/empty`
- **When** Alice moves it to `/vide`
- **Then** the move succeeds and nothing is created
- **Priority:** Low

### SPEC-0105/E2E-033: nested folders with three converted documents
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_033_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** `/a/x.pdf`, `/a/b/y.pdf`, `/a/b/c/z.pdf` converted with 1, 2 and 3 images
- **When** Alice moves `/a` to `/z`
- **Then** `/z/x.pdf`, `/z/b/y.pdf` and `/z/b/c/z.pdf` give 1, 2 and 3 images with their captions
- **Priority:** High

### SPEC-0105/E2E-034: a folder mixing converted and plain files
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_034_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002, FR-NEW-011
- **Given** `/mix/a.pdf` converted and `/mix/b.txt` plain
- **When** Alice moves `/mix` to `/mixed`
- **Then** `/mixed/a.pdf` gives its artifacts and `/mixed/b.txt` has none
- **Priority:** Medium

### SPEC-0105/E2E-035: a document moved back to its original path
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_035_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** D converted and moved to `/archive/report.pdf`
- **When** Alice moves it back to `/docs/report.pdf`
- **Then** `/docs/report.pdf` gives D's artifacts
- **Priority:** Medium

### SPEC-0105/E2E-038: a hundred converted documents in one folder
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_038_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** `/many` holds 100 converted documents, each with 1 image captioned `n<k>`
- **When** Alice moves `/many` to `/lots`
- **Then** all 100 give their own image caption, and the move completes within 10 seconds
- **Priority:** Medium

### SPEC-0105/E2E-039: two moves in a row
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_039_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** D converted
- **When** Alice moves it to `/a.pdf` then to `/b.pdf`
- **Then** `/b.pdf` gives D's artifacts and `/a.pdf` and `/docs/report.pdf` are refused
- **Priority:** Medium

### SPEC-0105/E2E-048: the images keep their ids and the table its caption
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_048_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012
- **Given** D converted
- **When** Alice moves D
- **Then** the images are `image-1` and `image-2`, the table is `table-1` captioned `Costs`, the same ids and captions as before
- **Priority:** Medium

### SPEC-0105/E2E-071: a rename under a plain file
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_071_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-010
- **Given** D converted and a plain file `/flat.txt`
- **When** Alice renames D to `/flat.txt/report.pdf`
- **Then** the rename is refused with `ERR_NO_CLOBBER: '/flat.txt' already exists and is not a directory` and D still gives its artifacts
- **Priority:** Medium

### SPEC-0105/E2E-MOD-001: a move no longer discards (was SPEC-0019 test 088)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_mod001_*`)
- **Validated before:** after a converted document is moved the artifacts are gone or the old rule applies.
- **Validates now:** after a converted document is moved the artifacts follow as FR-NEW-001 says (they follow the document to the new path).
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** D converted
- **When** A converted document is moved by Alice
- **Then** reading the artifacts at the destination gives those of before the operation (and at the old path is refused as not a file)

### SPEC-0105/E2E-MOD-004: a folder move keeps the artifacts (was SPEC-0019 test 095)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_mod004_*`)
- **Validated before:** after a folder of converted documents is moved the artifacts are gone or the old rule applies.
- **Validates now:** after a folder of converted documents is moved the artifacts follow as FR-NEW-002 says (every document of the folder keeps its artifacts).
- **Scenario:** SC-002
- **Requirements:** FR-MOD-001
- **Given** D converted
- **When** A folder of converted documents is moved by Alice
- **Then** reading the artifacts at the destination gives those of before the operation (and at the old path is refused as not a file)

### SPEC-0105/E2E-MOD-006: moving away and back keeps the artifacts (was SPEC-0019 test 097)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_mod006_*`)
- **Validated before:** after a converted document is moved away and back the artifacts are gone or the old rule applies.
- **Validates now:** after a converted document is moved away and back the artifacts follow as FR-NEW-001 says (a move away and a move back keep them).
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** D converted
- **When** A converted document is moved away and back by Alice
- **Then** after the second move the document gives its artifacts at the original path

## Constraints
- Copy, trash, restore and hard delete are later stories: leave `rename_into_trash` as it is.
- No change to a tool contract.
- A move must renew nothing a reader depends on besides the two columns above.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0308.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0302: artifacts follow a moved or renamed document or folder` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0105` and the requirement ids.
