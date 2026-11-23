# US-0293: tables with an exact CSV, a caption and a page

> Spec: SPEC-0103 (sdlc/specs/SPEC-0103-converter-answer-with-the-full-document-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0291, US-0292
> min_tier: 1
> Files touched: 8
> Closes spec: no

## Objective
Apply the manifest `tables` list to the document's pipe tables in order: an exact CSV file makes the table `CSV quality: exact` and is what `fs.get_table` returns in CSV, an unusable file keeps the table approximate with a line, a caption replaces the Markdown caption, a page is stored. A CSV reference that leaves the bundle is never read.

## Requirements covered
- SPEC-0103/FR-NEW-007: WHEN a version 2 manifest carries a `tables` list THE Documents context SHALL apply its entry number `i` to the `i`th table of the document, counting every pipe table of the Markdown in reading order.
- SPEC-0103/FR-NEW-008: WHEN a table entry has `csv` naming a file of the answer's bundle THE Documents context SHALL keep that file's content as the table's CSV and label the table `CSV quality: exact`.
- SPEC-0103/FR-NEW-009: IF the file named by a table entry's `csv` is missing, is not valid UTF-8, is longer than 5 000 000 characters or names a place outside the bundle THEN THE Documents context SHALL keep the table as approximate and add the report line `table-<i>: CSV unavailable`.
- SPEC-0103/FR-NEW-010: IF the `tables` list has more entries than the document has tables THEN THE Documents context SHALL apply the entries that match, ignore the others and add the report line `manifest lists <n> tables, the document has <m>`.
- SPEC-0103/FR-NEW-011: WHEN a table entry carries a `caption` or a `page` THE Documents context SHALL use that caption for the table when it is not empty, and keep that page with the table.
- SPEC-0103/FR-NEW-023 (partial: table entry fields: csv, caption, page types, null fields, an entry that is not an object): IF a field of a `pictures`, `tables` or `headings` entry has a value of the wrong type THEN THE Documents context SHALL treat that field as absent, keep the rest of the entry and of the manifest, and add the report line for that item.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0103_03 e2e_0103_04 e2e_0103_077 e2e_0103_078 e2e_0103_082 e2e_0103_084 e2e_0103_096 e2e_0103_104`. Tests convert documents with 1 to 3 tables and matching, missing, extra and non object entries, CSV files that are missing, not UTF-8, of 5 000 000 and 5 000 001 characters, `../../etc/passwd`, `/etc/passwd` and `http://example.com/t.csv` references, and read the tables with `fs.list_tables` and `fs.get_table`.

## Design excerpt
### Decisions
- DEC-002 (SPEC-0103): The manifest is read by hand from a `serde_json::Value` in `docs/manifest.rs`, field by field, returning `ManifestOutcome { manifest, notes }` and never an error; a whole number is a `Number` that is `u64` or `i64` (so `2.0` and `1e0` are refused); lengths count `chars()`. **Obliged by:** SPEC-0103/FR-NEW-001, FR-NEW-003, FR-NEW-016, FR-NEW-023. **Rationale:** serde derive rejects a whole manifest on one wrong field and cannot say which item was wrong. **Alternatives considered:** serde derive with defaults (today's silent discard), a schema validator crate. **Source:** existing code. **Evidence:** `docs/service.rs:746-754,786-790` (silent ignore today), `docs/service.rs:89-102`.
- DEC-004 (SPEC-0103): The parser returns notes grouped by item (manifest level lines, per picture target, per table entry number, heading lines); `PendingSet` maps a picture target to its image number once it exists, and `report()` writes the groups in the order of FR-NEW-017 (header, metadata, manifest, images by number, tables by number with `table-<i>` lines before `tables entry <i>` lines, headings). A picture that is not kept emits only its existing line (FR-NEW-023). **Obliged by:** SPEC-0103/FR-NEW-017, FR-NEW-023. **Rationale:** image numbers are known only after capture. **Alternatives considered:** number images in the parser (needs the Markdown parse). **Source:** existing code. **Evidence:** `capture.rs:93-135,149-151`.
- DEC-006 (SPEC-0103): Storage is additive in the volume meta store: nullable `doc_images.ocr_text`, `doc_tables.csv_text`, `doc_tables.page`, and a table `doc_headings`, all written by the existing `commit_artifact_set` transaction and deleted with their set by `sweep_stale_artifact_sets`; `quality` is `exact` for a manifest CSV. **Obliged by:** SPEC-0103/FR-NEW-004, FR-NEW-008, FR-NEW-011, FR-NEW-012, FR-NEW-013, FR-NEW-019, FR-NEW-020. **Rationale:** metadata of a set is replaced with the set, and old rows stay valid with nulls. **Alternatives considered:** one JSON column per set (not queryable by BL-0167), a separate store. **Source:** existing code. **Evidence:** `storage/meta.rs:114-140,362-378,932-1000,1060-1125`.
- DEC-008 (SPEC-0103): A table `csv` is resolved with the existing `resolve_target` and looked up only in the in memory `Conversion.files` map (never opened as a file, never read from the blob store); the bytes must be valid UTF-8 with at most 5 000 000 characters, else `table-<i>: CSV unavailable`. CSV files are not stored as files: only their text is kept in `doc_tables.csv_text`. **Obliged by:** SPEC-0103/FR-NEW-008, FR-NEW-009, FR-NEW-022. **Rationale:** the control that already stops picture traversal covers CSV for free. **Alternatives considered:** a second resolver. **Source:** existing code. **Evidence:** `capture.rs:256-298`.
- DEC-010 (SPEC-0103): The storage charge `PendingSet::bytes()` adds the OCR text bytes, the exact CSV bytes (instead of the derived CSV length for such a table) and the title bytes of every heading, so the existing admission and refusal texts (`ERR_PROJECT_QUOTA_EXCEEDED`, `session write quota of <n> bytes exceeded`) apply unchanged. **Obliged by:** SPEC-0103/FR-NEW-022. **Rationale:** one charge, as today. **Alternatives considered:** a second charge. **Source:** existing code. **Evidence:** `capture.rs:137-145`, `core/fs_ops.rs:799-875`.
- DEC-011 (SPEC-0103): `fs.get_table` in CSV returns the stored `csv_text` when the row has one, else the CSV rendered from cells; `fs.list_tables` labels from the stored `quality`. **Obliged by:** SPEC-0103/FR-NEW-008. **Rationale:** the only existing read path for table CSV. **Alternatives considered:** a new tool (BL-0167). **Source:** existing code. **Evidence:** `tools/artifacts.rs:238-306`, `docs/artifacts/render.rs:298`.
- DEC-012 (SPEC-0103): Table entry `i` applies to pipe table `seq` `i` (every pipe table counts, malformed ones included, so numbers match the report's `table-<n>`); an entry for a malformed table is unused and gives no line. **Obliged by:** SPEC-0103/FR-NEW-007, FR-NEW-010. **Rationale:** `table-N` numbering already counts every pipe table. **Alternatives considered:** number only kept tables (would shift ids). **Source:** existing code. **Evidence:** `docs/artifacts/parse.rs:19-33`, `capture.rs:125`.
### Data model
New nullable columns `doc_tables.csv_text` Text and `doc_tables.page` BigInt (`ColumnMigration`); `quality` stores `exact` for a manifest CSV; `DOC_TABLE_COLS` and the insert placeholders change; readers `artifact_table_cells` and the list query add the stored CSV and quality.
### Contracts
Table entry `i` (1 based) applies to pipe table `seq` `i`; an entry is an object with optional `csv`, `caption`, `page`; lines: `table-<i>: CSV unavailable`, `tables entry <i>: not an object`, `tables entry <i>: invalid page`, `tables entry <i>: invalid <field>` (csv, caption), `manifest lists <n> tables, the document has <m>`; null fields absent, `csv` null or empty is no csv; `csv` resolved by `resolve_target` and looked up only in `Conversion.files` (valid UTF-8, at most 5 000 000 chars, else unavailable); manifest caption (non empty) wins over the Markdown caption; a malformed table keeps its `table-<i>: malformed table` line and its entry is unused; report counts `j` of the metadata line = tables exact because of a CSV. `fs.get_table` csv answers `csv_text` when present; `fs.list_tables` label from stored `quality`. CSV files are never written as files.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/manifest.rs` | modify: `tables` reading and notes |
| `crates/core/src/docs/artifacts/capture.rs` | modify: apply entries, csv resolution, lines, charge, commit row |
| `crates/core/src/storage/meta.rs` | modify: columns, `NewDocTable`, insert, readers |
| `crates/core/src/tools/artifacts.rs` | modify: exact CSV in `get_table`, label from stored quality |
| `crates/core/src/storage/rel/schema.rs`, `crates/core/tests/fixtures/schema_*.sql` | modify / regenerate |
| `crates/core/src/docs/manifest_e2e.rs` | modify: these tests |
### Existing patterns
Target control `capture.rs:256-298`; table build and approximate quality `capture.rs:93-135`, `docs/artifacts/mod.rs:29-48`; table tools `tools/artifacts.rs:238-306`; `table_csv` `docs/artifacts/render.rs:298`; exploit style test shape: a stand in converter whose bundle holds a decoy file, then assert the decoy content appears nowhere.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0103/E2E-030: exact CSV for a table
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_030_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has one pipe table, the manifest lists it with `csv` `tab/t1.csv` whose content is `a,b\n1,"x,y"\n`; `version` 2
- **When** Alice converts it and asks `fs.get_table` of `table-1` in CSV
- **Then** the answer is exactly `a,b\n1,"x,y"\n`
- **And** `fs.list_tables` shows `CSV quality: exact`
- **Priority:** Critical

### SPEC-0103/E2E-031: a table entry with a caption
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_031_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has a pipe table with a caption line `Table 1: Sales` before it; the manifest entry says `caption` `Sales per region`; `version` 2
- **When** Alice converts it
- **Then** `fs.list_tables` shows the caption `Sales per region`
- **And** with an empty manifest caption it shows `Table 1: Sales`
- **Priority:** Medium

### SPEC-0103/E2E-032: the CSV file is missing from the bundle
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_032_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the table entry says `csv` `tab/missing.csv`, no such file; `version` 2
- **When** Alice converts it
- **Then** the report holds `table-1: CSV unavailable`
- **And** the table is kept and reads `CSV quality: approximate`
- **Priority:** High

### SPEC-0103/E2E-033: the CSV file is not text
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_033_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the CSV file holds the bytes `FF FE 00`
- **When** Alice converts it
- **Then** the report holds `table-1: CSV unavailable`
- **And** the table reads `CSV quality: approximate`
- **Priority:** High

### SPEC-0103/E2E-034: the CSV file is too large
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_034_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the CSV file holds 5 000 001 characters
- **When** Alice converts it
- **Then** the report holds `table-1: CSV unavailable`
- **And** a file of exactly 5 000 000 characters is kept as exact CSV
- **Priority:** High

### SPEC-0103/E2E-035: a CSV reference that escapes the bundle
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_035_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the table entries say `csv` `../../etc/passwd`, `/etc/passwd` and `http://example.com/t.csv`
- **When** Alice converts it
- **Then** each table reports `table-N: CSV unavailable` and reads `CSV quality: approximate`
- **And** no file outside the bundle was read (the content of `/etc/passwd` appears nowhere)
- **Priority:** High

### SPEC-0103/E2E-036: more entries than tables
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_036_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has 1 pipe table, the manifest lists 3 tables
- **When** Alice converts it
- **Then** the report holds `manifest lists 3 tables, the document has 1`
- **And** the first entry is applied to the table, the two others ignored
- **Priority:** High

### SPEC-0103/E2E-037: fewer entries than tables
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_037_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has 3 pipe tables, the manifest lists 1 with a CSV
- **When** Alice converts it
- **Then** table 1 reads `exact`, tables 2 and 3 read `approximate`
- **And** no mismatch line is added
- **Priority:** High

### SPEC-0103/E2E-038: a manifest table entry for a malformed table
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_038_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has a pipe table whose separator has a different number of cells than its header; the manifest gives it a CSV
- **When** Alice converts it
- **Then** the report keeps today's line `table-1: malformed table`
- **And** the CSV is not used, the table is not kept as today
- **Priority:** Medium

### SPEC-0103/E2E-039: a table entry that is not an object
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_039_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest `tables` list holds `1` as its first entry and an entry with `csv` for the second table
- **When** Alice converts it
- **Then** table 1 stays approximate, table 2 reads exact
- **And** the report holds `tables entry 1: not an object`
- **Priority:** Medium

### SPEC-0103/E2E-040: the table page is kept
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_040_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); table entries with `page` 2; `version` 2
- **When** Alice converts it
- **Then** the conversion succeeds, the report is `0 images, 2 tables` then the metadata line and no `tables entry` line
- **And** with a `.docx` source (`/doc.docx` converted the same way) the report is the same
- **Priority:** Medium

### SPEC-0103/E2E-041: a table page that is invalid
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_041_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the first table entry has `page` 0, the second `page` `two`
- **When** Alice converts it
- **Then** the report holds `tables entry 1: invalid page` and `tables entry 2: invalid page`
- **And** both tables keep their other data (CSV exact if given)
- **Priority:** Medium

### SPEC-0103/E2E-042: exactly as many entries as tables
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_042_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has 2 pipe tables and the manifest lists 2
- **When** Alice converts it
- **Then** no mismatch line is added
- **And** each entry applies to its table in order
- **Priority:** Medium

### SPEC-0103/E2E-043: a table with no CSV and a table with CSV in reverse order
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_043_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); table 1 has no `csv`, table 2 has `tab/t2.csv`
- **When** Alice converts it
- **Then** `fs.list_tables` shows table 1 `approximate` and table 2 `exact`
- **And** `fs.get_table` of table 2 in CSV answers the exact file
- **Priority:** Medium

### SPEC-0103/E2E-082: a table CSV for a page range of a spreadsheet
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_082_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/sheet.xlsx` converted with 3 tables each with its CSV
- **When** Alice lists the tables
- **Then** the three read exact in document order
- **And** the report has no `tables entry` line
- **Priority:** Medium

### SPEC-0103/E2E-084: the caption of a table from the Markdown wins when the manifest has none
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_084_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a table caption line `Table 2: Costs` in the Markdown and an entry without `caption`
- **When** Alice converts it
- **Then** the caption is `Table 2: Costs`
- **And** with the manifest caption `Costs` it is `Costs`
- **Priority:** Medium

### SPEC-0103/E2E-096: a table field of the wrong type
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_096_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); table entries with `csv` 7 and `caption` 5, `version` 2
- **When** Alice converts it
- **Then** the report holds `tables entry 1: invalid csv` and `tables entry 2: invalid caption`
- **And** both tables read approximate
- **Priority:** Medium

### SPEC-0103/E2E-104: null fields are absent
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_104_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); picture 1 has `ocr_text` null, `caption` null and `page` null; table entry 1 has `csv` null; headings entry with `page` null
- **When** Alice converts it
- **Then** the report has no `invalid` line and the count of images with OCR text is 0
- **And** the headings entry with a null page is counted
- **Priority:** Medium

### SPEC-0103/E2E-077: table files are not left in the volume
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_077_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the bundle holds a CSV file `tab/t1.csv`
- **When** Alice converts it and lists the folder of the document
- **Then** the folder holds the document and its Markdown companion only, no CSV file
- **And** the CSV is available through `fs.get_table`
- **Priority:** Medium

### SPEC-0103/E2E-078: a bundle file with an unsafe name
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_078_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the HTTP form returns a file named `../tab.csv` referenced by a table entry
- **When** Alice converts it
- **Then** the table reads approximate with `table-1: CSV unavailable`
- **And** the conversion succeeds
- **Priority:** Medium

## Constraints
- Never open a file by a converter supplied name.
- No CSV file lands in the project.
- The table page is metadata only.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/tools.md` (exact CSV in `fs.get_table`).
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0293: tables with an exact CSV, a caption and a page` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0103` and the requirement ids.
