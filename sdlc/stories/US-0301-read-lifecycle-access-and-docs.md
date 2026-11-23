# US-0301: access, older documents, re-conversion and the docs

> Spec: SPEC-0104 (sdlc/specs/SPEC-0104-read-tools-for-the-document-metadata/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0300
> min_tier: 2
> Files touched: 6
> Closes spec: yes

## Objective
Close the spec: older documents and the built in reading read without error, a non member and an unknown project are refused on every new read, nothing is written by a read, a new conversion replaces what is read, and the docs describe the new reads.

## Requirements covered
- SPEC-0104/FR-NEW-007: A document converted before the metadata existed, or by the platform's own reading, SHALL NOT be refused or converted again by any of these reads.
- SPEC-0104/FR-NEW-009: The reads of this increment SHALL be open to the members of the project only, AND SHALL NOT change any file, any access time, the storage used or the document's conversion.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0104_` then `make check`. Tests read a document converted by a previous release shape and a built in docx, convert twice with different manifests, refuse Eve and `ghost`, and compare the file list, access times and used storage around reads.

## Design excerpt
### Decisions
- DEC-009 (SPEC-0104): The new reads touch no access time and write nothing (no call to `touch_atime`). **Obliged by:** SPEC-0104/FR-NEW-009. **Rationale:** the existing artifact reads already call none; only `tools/diff_files.rs:87` does. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `tools/diff_files.rs:87-88`.
- DEC-011 (SPEC-0104): A document converted before SPEC-0103 or by the built in reading needs no code path: its null OCR text and missing heading rows read as `null` and an empty list. **Obliged by:** SPEC-0104/FR-NEW-007. **Rationale:** additive columns, nothing converts. **Alternatives considered:** a migration (forbidden by SPEC-0103 FR-NEW-020). **Source:** SPEC-0103. **Evidence:** `storage/meta.rs:274,301`.
### Data model
No schema change.
### Contracts
No new contract. If a test finds a defect it is fixed here and said so. An old set fixture writes null OCR text and no heading rows (the helper `old_set` of `docs/manifest_e2e.rs`).
### Paths
| Path | Action |
|---|---|
| `crates/core/src/tools/read_metadata_e2e.rs` | modify: tests |
| `README.md`, `AGENTS.md` | modify: tool count 136, the new reads, the test command `cargo test -p fsa-core --lib -- e2e_0104_` |
| `.agent_docs/tools.md`, `.agent_docs/api.md`, `.agent_docs/testing.md` | modify: structure tool, `view`, `image-file`, test entry |
### Existing patterns
README manifest section; `.agent_docs/tools.md` artifact tools; counts mentioned in `AGENTS.md`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0104/E2E-001: a converted document read end to end
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_001_*`)
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001, FR-MOD-002, FR-NEW-001, FR-NEW-002, FR-NEW-006, FR-NEW-007
- **Given** project `proj`, Alice (`alice@example.com`) a member, document D converted
- **When** Alice lists the images, gets `image-1`, downloads `image-1`, reads the structure, then reads the full view
- **Then** the list shows `image-1` with OCR text `Revenue 2024: 4.2 M EUR` and `image-2` with no text; the get answer for `image-1` carries the same text and its picture; the download is the picture file of `image-1`; the structure lists `Report`, `Costs`, `Outlook` in that order; the full view shows `[OCR image-1: Revenue 2024: 4.2 M EUR]` on the line after the image line of `image-1`
- **And** nothing in the project's files changed: the file list before and after is identical
- **Priority:** Critical

### SPEC-0104/E2E-011: a non member reads nothing
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_011_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-009, FR-MOD-001, FR-NEW-001, FR-NEW-002, FR-NEW-004
- **Given** document D converted; Eve (`eve@example.com`) is not a member of `proj`
- **When** Eve lists the images, downloads `image-1`, reads the structure and reads the view
- **Then** each is refused with `ERR_FORBIDDEN: 'eve@example.com' is not a member of 'proj'`
- **And** no picture bytes, OCR text or heading appears in any answer
- **Priority:** High

### SPEC-0104/E2E-023: an unknown project
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_023_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-009
- **Given** no project `ghost`
- **When** Alice reads the structure of `/report.pdf` in `ghost` and downloads `image-1` from it
- **Then** each is refused with `ERR_PROJECT_NOT_FOUND: project 'ghost' not found`
- **Priority:** Medium

### SPEC-0104/E2E-026: a document converted before the metadata existed
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_026_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** `/old.pdf` converted by a release that stored no OCR text and no structure, one image captioned `Old figure`
- **When** Alice lists the images, reads the structure, reads the view as `full`
- **Then** the image has OCR text `null`; the structure is an empty list with no marker, not a refusal; the full view equals the standard view and carries an empty structure
- **And** nothing is converted again: the document's conversion is untouched
- **Priority:** High

### SPEC-0104/E2E-030: a new conversion replaces what is read
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_030_*`)
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001, FR-NEW-002, FR-NEW-006
- **Given** document D converted, then converted again by a converter returning one image with OCR text `Q3 chart` and one heading `Summary`
- **When** Alice lists the images, reads the structure and the full view
- **Then** one image with OCR text `Q3 chart`; one heading `Summary`; the full view shows `[OCR image-1: Q3 chart]` and nothing of the first conversion
- **Priority:** High

### SPEC-0104/E2E-035: a document converted by the built in reading
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_035_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-003
- **Given** `/doc.docx` converted by the platform's own reading, with no converter
- **When** Alice reads its structure and its view as `full`
- **Then** the structure is an empty list; the full view equals the standard view plus an empty structure
- **Priority:** Medium

### SPEC-0104/E2E-039: a download changes nothing
- Level / Driver / Location: functional e2e / the real McpServer tool methods and the real router, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/read_metadata_e2e.rs` (`e2e_0104_039_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-009
- **Given** document D converted; the file list, its access times and the project's used storage noted
- **When** Alice downloads `image-1` and reads the structure
- **Then** the file list, the access time of `/report.pdf` and the used storage are unchanged
- **And** the conversion of `/report.pdf` is still the same one
- **Priority:** High

## Constraints
- No production behavior change expected.
- No dashes as punctuation in docs.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: All the files named in Paths.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.
- [ ] Closes spec: the spec full acceptance set green (every E2E-001 to E2E-051) and every requirement of SPEC-0104 cited by at least one story with status done or landed.
- [ ] One commit `US-0301: access, older documents, re-conversion and the docs` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0104` and the requirement ids.
