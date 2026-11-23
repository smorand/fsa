# US-0295: replacing, upgrading, other paths and the docs

> Spec: SPEC-0103 (sdlc/specs/SPEC-0103-converter-answer-with-the-full-document-metadata/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0294
> min_tier: 2
> Files touched: 6
> Closes spec: yes

## Objective
Prove and document the lifecycle: a re-conversion replaces the whole set, a failed one changes nothing, documents converted before keep their data and nothing converts at upgrade, the built in extraction is unchanged, and the full journeys (program form, upload, a second member) hold. Close the spec: README, AGENTS.md and agent docs describe manifest version 2, and the whole acceptance set runs green.

## Requirements covered
- SPEC-0103/FR-NEW-019: WHEN a document is converted again THE Documents context SHALL replace the images, tables, OCR text, CSV and structure of the document as one set.
- SPEC-0103/FR-NEW-020: The Documents context SHALL keep the images and tables of documents converted before this increment as they are, and SHALL NOT convert any document because of the upgrade.
- SPEC-0103/FR-NEW-021: The Documents context SHALL keep the behavior and the report of the built in text extraction unchanged.
- SPEC-0103/FR-NEW-024: IF a conversion fails THEN THE Documents context SHALL keep the previous set of the document as it was.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0103_ e2e_0100_ e2e_0101_ e2e_0102_ docs:: tools::artifacts tools::converter_bound` and `make check`. The lifecycle tests convert twice with different manifests, make the converter fail or time out, restart the platform, mix an old and a new document, and run the built in extraction.

## Design excerpt
### Decisions
- DEC-013 (SPEC-0103): A re-conversion and a failed conversion keep today's behavior: the new rows are part of the one `commit_artifact_set` transaction, which supersedes the previous set or fails with nothing written (blobs of a refused commit are released). **Obliged by:** SPEC-0103/FR-NEW-019, FR-NEW-024. **Rationale:** no new mechanism needed. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `storage/meta.rs:932-1000`, `capture.rs:159-229`.
- DEC-014 (SPEC-0103): Nothing converts at upgrade and no data migration runs: new columns read null and `doc_headings` is empty for older sets. **Obliged by:** SPEC-0103/FR-NEW-020. **Rationale:** additive schema. **Alternatives considered:** a background reconversion (not asked). **Source:** spec. **Evidence:** n/a.
- DEC-012 (SPEC-0103): Table entry `i` applies to pipe table `seq` `i` (every pipe table counts, malformed ones included, so numbers match the report's `table-<n>`); an entry for a malformed table is unused and gives no line. **Obliged by:** SPEC-0103/FR-NEW-007, FR-NEW-010. **Rationale:** `table-N` numbering already counts every pipe table. **Alternatives considered:** number only kept tables (would shift ids). **Source:** existing code. **Evidence:** `docs/artifacts/parse.rs:19-33`, `capture.rs:125`.
### Data model
No schema change.
### Contracts
No new contract: the behaviors are those of `commit_artifact_set` (one transaction, supersede) and the built in path (`PendingSet::builtin`, no structure, no metadata line); a test fixture writes a set the way the previous version did (null new columns, no heading rows).
### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/manifest_e2e.rs` | modify: these tests and an old set fixture |
| `README.md`, `AGENTS.md` | modify: manifest version 2, report lines, test command `cargo test -p fsa-core --lib -- e2e_0103_` |
| `.agent_docs/architecture.md`, `.agent_docs/tools.md`, `.agent_docs/testing.md` | modify: converter contract, the end of the "Converter limitation" notes at `architecture.md:169` and `tools.md:482` updated to say version 2 is defined |
### Existing patterns
README manifest section `README.md:219-259`; `commit_artifact_set` `storage/meta.rs:932-1000`; built in path `capture.rs:59`, `core/fs_ops.rs:1762`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0103/E2E-001: a full version 2 answer from the program form
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_001_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2, two pictures (`fig/a.png` with caption `Sales` and OCR text `Total 42`, `fig/b.png` with caption `Costs` and OCR text `Net 7`), two tables (the first with CSV file `tab/t1.csv`, the second with none) and four headings; `/report.pdf` of 3 pages
- **When** Alice converts it with the program form
- **Then** the conversion succeeds and its report is `2 images, 2 tables`, then `metadata: 2 images with OCR text, 1 tables with exact CSV, 4 headings`
- **And** `fs.list_tables` shows the first table `CSV quality: exact` and the second `CSV quality: approximate`; `fs.get_table` of the first in CSV answers the exact content of `tab/t1.csv`
- **Priority:** Critical

### SPEC-0103/E2E-062: the same manifest twice
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_062_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the same document converted twice with the same manifest
- **When** Alice converts it twice
- **Then** both reports are equal
- **And** the lists are the same, with no duplicate image or table
- **Priority:** Medium

### SPEC-0103/E2E-068: an extraction option does not change a converted document
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_068_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/notes.docx` converted with a converter, then the built in extraction run on it with the refresh option
- **When** Alice runs the built in extraction
- **Then** the built in extraction replaces the set, tables read exact, no metadata line
- **And** no OCR text remains
- **Priority:** Medium

### SPEC-0103/E2E-069: a document converted before is read after a restart
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_069_*`)
- **Given** a document converted by the previous version
- **When** the operator restarts the platform and Alice lists its tables
- **Then** the lists are unchanged
- **And** the converter stub received no call
- **Priority:** Medium

### SPEC-0103/E2E-070: a re-conversion replaces the whole set
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_070_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/report.pdf` converted with OCR text on 2 pictures and an exact CSV; then the converter is changed to send version 1 answers
- **When** Alice converts the document again
- **Then** the report is `2 images, 1 tables` with no `metadata:` line; the table reads `CSV quality: approximate`
- **And** `fs.get_table` of that table in CSV answers the CSV derived from the Markdown, not the first conversion's exact file
- **Priority:** High

### SPEC-0103/E2E-071: a failed re-conversion keeps the previous set
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_071_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/report.pdf` converted with a full version 2 answer; the converter now fails
- **When** Alice converts it again
- **Then** the conversion fails with the existing converter error text
- **And** the images, tables and exact CSV of the first conversion are intact
- **Priority:** High

### SPEC-0103/E2E-072: documents converted before keep their data
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_072_*`)
- **Given** a document converted by the previous version, with captions and approximate tables
- **When** the platform is upgraded and Alice lists its images and tables
- **Then** the lists are unchanged and nothing is converted again
- **And** a new conversion produces version 2 data
- **Priority:** High

### SPEC-0103/E2E-073: the built in extraction is unchanged
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_073_*`)
- **Given** project `proj`, Alice a member; no converter service is configured; `/notes.docx`
- **When** Alice extracts its text
- **Then** tables read `CSV quality: exact` as today and the report has no `metadata:` line
- **And** the report is exactly as before this increment
- **Priority:** Medium

### SPEC-0103/E2E-074: a file edited after conversion
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_074_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/report.pdf` converted with full data; the file is then replaced
- **When** Alice lists its images
- **Then** the answer is `No artifacts: document not extracted` as today
- **And** listing its tables gives the same answer
- **Priority:** Medium

### SPEC-0103/E2E-080: conversion by upload carries the same data
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_080_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); Alice uploads `/report.pdf` with the conversion option set
- **When** the upload completes
- **Then** the answer carries the report of E2E-001
- **And** the images and tables are those of E2E-001
- **Priority:** High

### SPEC-0103/E2E-081: a non member cannot convert
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_081_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); Dave (`dave@example.com`) is not a member
- **When** Dave converts `/report.pdf`
- **Then** the answer is `ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'`
- **And** nothing is stored
- **Priority:** High

### SPEC-0103/E2E-083: a second member sees the same data
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_083_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); Carol (`carol@example.com`) is a member too; `/report.pdf` converted by Alice with a full answer
- **When** Carol lists the tables
- **Then** she sees the exact CSV tables
- **And** the converter stub received one call in total
- **Priority:** Medium

### SPEC-0103/E2E-085: a failed conversion changes nothing
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_085_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter times out on `/report.pdf`
- **When** Alice converts it
- **Then** the conversion fails with the existing timeout text
- **And** listing its images and tables answers `No artifacts: document not extracted` (the document was never converted before)
- **Priority:** Medium

### SPEC-0103/E2E-086: an old document and a new one in the same folder
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_086_*`)
- **Given** `/old.pdf` converted before the upgrade and `/new.pdf` converted after, in the same project
- **When** Alice lists the tables of both
- **Then** `/old.pdf` tables read approximate, `/new.pdf` tables read exact
- **And** the two sets are independent
- **Priority:** Medium

### SPEC-0103/E2E-088: the built in extraction of a spreadsheet
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_088_*`)
- **Given** no converter service configured; `/budget.xlsx` with 2 sheets of one table each
- **When** Alice extracts its text
- **Then** the report is `0 images, 2 tables`, both `CSV quality: exact`, as before this increment
- **And** no `metadata:` line
- **Priority:** Medium

### SPEC-0103/E2E-102: the HTTP converter fails
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_102_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP; the service answers an error after a first successful conversion
- **When** Alice converts it again
- **Then** the conversion fails with the existing error text
- **And** the previous images, tables and exact CSV are intact
- **Priority:** Medium

## Constraints
- No production behavior change expected here; if a test finds one, fix it in this story and say so.
- No dashes as punctuation in docs.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: All of `README.md`, `AGENTS.md`, `.agent_docs/*` named in Paths.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.
- [ ] Closes spec: the spec full acceptance set green (every E2E-001 to E2E-107 and E2E-MOD-001) and every requirement of SPEC-0103 cited by at least one story with status done or landed.
- [ ] One commit `US-0295: replacing, upgrading, other paths and the docs` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0103` and the requirement ids.
