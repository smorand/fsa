# US-0291: the document structure and the metadata line of the report

> Spec: SPEC-0103 (sdlc/specs/SPEC-0103-converter-answer-with-the-full-document-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0290
> min_tier: 1
> Files touched: 8
> Closes spec: no

## Objective
Store a structure for every converted document: the converter's `headings` when given (valid entries, at most 10 000), otherwise the headings derived from the Markdown, and report the `metadata:` line for version 2 manifests with the invalid or dropped heading lines. After this story a version 2 conversion tells its caller how many headings it kept.

## Requirements covered
- SPEC-0103/FR-NEW-012: WHEN a version 2 manifest carries a `headings` list THE Documents context SHALL keep that list, in its order, as the structure of the document.
- SPEC-0103/FR-NEW-013: The Documents context SHALL keep a structure for every converted document: the converter's `headings` when supplied, otherwise the headings found in the Markdown.
- SPEC-0103/FR-NEW-014: IF a `headings` entry fails one of the rules of FR-NEW-012 THEN THE Documents context SHALL drop that entry and add the report line `heading <i>: invalid`.
- SPEC-0103/FR-NEW-015: IF a `headings` list has more than 10 000 valid entries THEN THE Documents context SHALL keep the first 10 000 and add the report line `headings over 10000 dropped: <n>`.
- SPEC-0103/FR-NEW-017 (partial: the metadata line and the full report order; the OCR and CSV counts are 0 until their stories): WHEN a conversion with a version 2 manifest (or a newer one) completes THE Documents context SHALL add to its report, right after the line `<N> images, <M> tables`, the line `metadata: <k> images with OCR text, <j> tables with exact CSV, <h> headings`.
- SPEC-0103/FR-MOD-001: WHEN a conversion with a version 2 manifest completes THE Documents context SHALL return the report with the metadata line of FR-NEW-017.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0103_05 e2e_0103_06 e2e_0103_089 e2e_0103_09 e2e_0103_1 e2e_0103_mod docs::artifacts::headings`. Tests convert documents with 2, 3, 10 000, 10 002 and 20 000 headings, empty and invalid lists, a fenced code block holding `# not a heading`, `## Q1 ##` and `# C#`, and assert the exact report; a unit test checks `derive` on the same Markdown.

## Design excerpt
### Decisions
- DEC-005 (SPEC-0103): The `metadata:` line is written when the manifest version is 2 or newer after a successful parse: `k` = images kept with non empty OCR text, `j` = tables labelled exact because of a manifest CSV, `h` = headings of the stored structure; a manifest that is ignored (including an invalid version) counts as no manifest. **Obliged by:** SPEC-0103/FR-NEW-017, FR-NEW-003, FR-MOD-001. **Rationale:** matches the spec's counts one to one. **Alternatives considered:** count from the stored rows after commit (adds a read). **Source:** spec. **Evidence:** n/a.
- DEC-006 (SPEC-0103): Storage is additive in the volume meta store: nullable `doc_images.ocr_text`, `doc_tables.csv_text`, `doc_tables.page`, and a table `doc_headings`, all written by the existing `commit_artifact_set` transaction and deleted with their set by `sweep_stale_artifact_sets`; `quality` is `exact` for a manifest CSV. **Obliged by:** SPEC-0103/FR-NEW-004, FR-NEW-008, FR-NEW-011, FR-NEW-012, FR-NEW-013, FR-NEW-019, FR-NEW-020. **Rationale:** metadata of a set is replaced with the set, and old rows stay valid with nulls. **Alternatives considered:** one JSON column per set (not queryable by BL-0167), a separate store. **Source:** existing code. **Evidence:** `storage/meta.rs:114-140,362-378,932-1000,1060-1125`.
- DEC-007 (SPEC-0103): Derived headings come from `docs/artifacts/headings.rs`: one pass over the lines, fence tracking as `parse.rs` does (a line starting with three backticks toggles), the rule of `chunking::blocks::heading` for each other line (closing run of `#` removed, `C#` kept), titles over 1 000 characters dropped silently, at most 10 000 kept with `headings over 10000 dropped: <n>` only for a version 2 manifest. The built in extraction stores no structure (its behavior stays unchanged, FR-NEW-021). **Obliged by:** SPEC-0103/FR-NEW-013, FR-NEW-015, FR-NEW-021. **Rationale:** one heading rule for chunking and metadata; linear time. **Alternatives considered:** re-implement the rule, run a Markdown crate. **Source:** existing code. **Evidence:** `chunking/blocks.rs:61,184`, `docs/artifacts/parse.rs:39-57`.
- DEC-014 (SPEC-0103): Nothing converts at upgrade and no data migration runs: new columns read null and `doc_headings` is empty for older sets. **Obliged by:** SPEC-0103/FR-NEW-020. **Rationale:** additive schema. **Alternatives considered:** a background reconversion (not asked). **Source:** spec. **Evidence:** n/a.
### Data model
New table `doc_headings` (`volume_id`, `set_id`, `seq`, `level`, `title`, `page` null; primary key `(volume_id, set_id, seq)`), rows written in `commit_artifact_set`, deleted by `sweep_stale_artifact_sets`; no heading rows for older sets.
### Contracts
Rust: `docs::artifacts::headings::derive(markdown) -> Vec<Heading>`; `Manifest.headings: Option<Vec<Heading>>` (present even when empty means supplied: the valid entries are the structure and nothing is derived); entry rules (object, `level` whole 1 to 6, `title` 1 to 1 000 characters after trimming and stored trimmed, `page` whole 1 or more, null `page` absent, null `level` or `title` invalid) with `heading <i>: invalid`, at most 100 such lines then `heading lines over 100 dropped: <n>`; more than 10 000 entries keep the first 10 000 with `headings over 10000 dropped: <n>` (derived headings too, only when the manifest is version 2 or newer); report: after the header, `metadata: <k> images with OCR text, <j> tables with exact CSV, <h> headings` for version 2 and newer (k and j are 0 here), and heading lines last. A version 1, versionless or ignored manifest gets no metadata line and the structure is derived and stored. The built in extraction stores no structure.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/artifacts/headings.rs`, `artifacts/mod.rs` | create / modify |
| `crates/core/src/docs/artifacts/capture.rs` | modify: headings in `PendingSet`, metadata line, charge, commit |
| `crates/core/src/docs/manifest.rs` | modify: `headings` reading and notes |
| `crates/core/src/storage/meta.rs` | modify: `doc_headings` table, `NewDocHeading`, insert and sweep |
| `crates/core/src/storage/rel/schema.rs`, `crates/core/tests/fixtures/schema_sqlite.sql`, `schema_postgres.sql` | modify / regenerate: DDL fingerprints (`production_ddl_is_unchanged_on_every_engine`), `MCPFS_REWRITE_SCHEMA_GOLDEN=1 cargo test -p fsa-core --test schema_golden`; fix the `storage::meta` table list test (`volume_tables`) |
| `crates/core/src/docs/manifest_e2e.rs` | modify: these tests |
### Existing patterns
Fence tracking `docs/artifacts/parse.rs:39-57`; heading rule `chunking/blocks.rs:61,184`; set commit and sweep `storage/meta.rs:932-1000,1060-1125`; insert column lists `storage/meta.rs:115,129,970,990`; schema golden update as done in the earlier specs (regenerate fixtures, then fix DDL fingerprints).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0103/E2E-050: headings supplied by the converter
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_050_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists headings `{level 1, Sales}`, `{level 2, Q1, page 2}`, `{level 2, Q2, page 3}`
- **When** Alice converts it
- **Then** the report holds `metadata: 0 images with OCR text, 0 tables with exact CSV, 3 headings`
- **And** the headings are those of the manifest, not those of the Markdown: the Markdown has 2 headings and the count in the line is 3
- **Priority:** High

### SPEC-0103/E2E-051: headings derived from the Markdown
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_051_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2 with no `headings`; the Markdown has `# Sales`, `## Q1`, a fenced code block holding the line `# not a heading`, and `## Q2`
- **When** Alice converts it
- **Then** the report holds `3 headings`
- **And** the line inside the code block is not counted
- **Priority:** High

### SPEC-0103/E2E-052: derived headings without any manifest
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_052_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no manifest; the Markdown has 2 headings
- **When** Alice converts it
- **Then** the report is unchanged (`0 images, 0 tables`) and no `metadata:` line is added
- **And** the conversion succeeds with no failure line
- **Priority:** Medium

### SPEC-0103/E2E-053: an invalid heading entry
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_053_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the headings list holds `{level 0, X}`, `{level 7, Y}`, `{level 2, empty title}`, `{level 2, Z}`
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid`, `heading 2: invalid`, `heading 3: invalid` and `1 headings`
- **And** the entry `Z` is kept
- **Priority:** High

### SPEC-0103/E2E-054: limits of the heading level
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_054_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); headings of level 1 and level 6
- **When** Alice converts it
- **Then** both are counted
- **And** a heading of level 6 with a title of 500 characters is counted too
- **Priority:** Medium

### SPEC-0103/E2E-055: too many headings
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_055_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists 10 001 headings
- **When** Alice converts it
- **Then** the report holds `10000 headings` in the metadata line and the line `headings over 10000 dropped: 1`
- **And** a list of exactly 10 000 produces no such line
- **Priority:** High

### SPEC-0103/E2E-056: a heading title with markup and accents
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_056_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a heading `{level 1, "Résumé **final** <b>"}`
- **When** Alice converts it
- **Then** the heading is counted
- **And** the conversion report has no failure line
- **Priority:** Medium

### SPEC-0103/E2E-058: exactly 10 000 headings
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_058_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists exactly 10 000 headings
- **When** Alice converts it
- **Then** the metadata line says `10000 headings` and no dropped line is added
- **And** the conversion succeeds
- **Priority:** Medium

### SPEC-0103/E2E-059: 20 000 headings
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_059_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists 20 000 headings
- **When** Alice converts it
- **Then** the report holds `headings over 10000 dropped: 10000`
- **And** 10 000 headings are kept
- **Priority:** Medium

### SPEC-0103/E2E-060: a converter that sends only captions
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_060_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/report.pdf`; a version 2 manifest with captions only; the Markdown has 2 headings
- **When** Alice converts it
- **Then** the report holds `metadata: 0 images with OCR text, 0 tables with exact CSV, 2 headings` (the headings of the Markdown)
- **And** captions and pages are kept as today
- **Priority:** High

### SPEC-0103/E2E-061: a version 2 manifest with nothing in it
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_061_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest is `{"version": 2}` and the Markdown has 1 heading
- **When** Alice converts it
- **Then** the report holds `metadata: 0 images with OCR text, 0 tables with exact CSV, 1 headings`
- **And** no failure line
- **Priority:** Medium

### SPEC-0103/E2E-063: a version 2 conversion without pictures
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_063_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2 and the Markdown has one pipe table and no picture
- **When** Alice converts it
- **Then** the report is `0 images, 1 tables` then `metadata: 0 images with OCR text, 0 tables with exact CSV, 0 headings`
- **And** no failure line
- **Priority:** Medium

### SPEC-0103/E2E-089: a heading entry that is not an object
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_089_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the headings list holds the text `Intro`, then `{level 1, Real}`
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid` and `1 headings`
- **And** the entry `Real` is kept
- **Priority:** Medium

### SPEC-0103/E2E-090: a heading level that is not a whole number
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_090_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); headings with `level` 1.5, `level` `1` (text) and `level` 3
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid` and `heading 2: invalid` and `1 headings`
- **And** the entry of level 3 is kept
- **Priority:** Medium

### SPEC-0103/E2E-091: a heading title of 1 001 characters
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_091_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a heading with a title of 1 001 characters and another of exactly 1 000
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid` and `1 headings`
- **And** the title of 1 000 characters is kept
- **Priority:** Medium

### SPEC-0103/E2E-099: an empty headings list
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_099_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `headings` is the empty list; the Markdown has 3 headings
- **When** Alice converts it
- **Then** the report holds `metadata: 0 images with OCR text, 0 tables with exact CSV, 0 headings`
- **And** no Markdown heading is derived
- **Priority:** Medium

### SPEC-0103/E2E-100: 150 invalid heading entries
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_100_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists 150 headings with level 0
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid` to `heading 100: invalid`, then `heading lines over 100 dropped: 50`
- **And** the metadata line says `0 headings`
- **Priority:** Medium

### SPEC-0103/E2E-101: a derived heading with a closing run
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_101_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no `headings` in the manifest; the Markdown has `## Q1 ##`, `# C#` and a heading whose title has 1 001 characters
- **When** Alice converts it
- **Then** a conversion with a `version` 2 manifest holding no `headings`: the metadata line counts 2 headings (the long one is dropped without a line)
- **And** the derived titles are `Q1` and `C#`
- **Priority:** Medium

### SPEC-0103/E2E-106: more than 10 000 derived headings
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_106_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no `headings` in the manifest; the Markdown has 10 002 headings
- **When** Alice converts it
- **Then** the metadata line counts 10000 headings and the report holds `headings over 10000 dropped: 2`
- **And** with no manifest at all the same document gives no such line
- **Priority:** Medium

### SPEC-0103/E2E-MOD-001: converting with a manifest (was SPEC-0019/E2E-001)
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_mod001_*`)
- **Validated before:** `report.pdf` converted with a manifest of captions and pages: the result reports `2 images, 2 tables` and no missing item; listing images shows `image-1` `Figure 1: Revenue 2024` page 1 and `image-2` `Schéma réseau 🌐` page 3.
- **Validates now:** the same, and when the manifest says `version` 2 the report holds a second line `metadata: 0 images with OCR text, 0 tables with exact CSV, 1 headings` (the Markdown of that fixture has 1 heading); a manifest without `version` gives the exact report as before.
- **Given** the SPEC-0019 data of that test, once with no `version` and once with `version` 2
- **When** Alice converts it
- **Then** the first report is `2 images, 2 tables`; the second is `2 images, 2 tables` then the metadata line
- **And** the captions and pages listed are identical in both
- **Priority:** High

## Constraints
- Do not read OCR or table entries yet.
- Heading lines and the metadata line keep the order of FR-NEW-017.
- The schema change is additive only.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/architecture.md` (structure stored per set).
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0291: the document structure and the metadata line of the report` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0103` and the requirement ids.
