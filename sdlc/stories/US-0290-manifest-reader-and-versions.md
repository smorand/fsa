# US-0290: the manifest reader, versions and manifest level report lines

> Spec: SPEC-0103 (sdlc/specs/SPEC-0103-converter-answer-with-the-full-document-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 1
> Files touched: 6
> Closes spec: no

## Objective
Replace the silent, all or nothing reading of the converter manifest by a tolerant versioned reader: version 1 reads as today, version 2 and newer are recognised, an invalid version or a malformed manifest is ignored with a `manifest ignored: <reason>` report line, and a wrong typed field of a version 1 picture drops that field with a line instead of discarding the manifest. The conversion always succeeds. It also creates the end to end test module and its converter stand ins for the program and HTTP forms.

## Requirements covered
- SPEC-0103/FR-NEW-001: WHEN the converter's answer carries a manifest THE Documents context SHALL read it by its `version`: absent or 1 as the manifest of today, 2 with the fields of this increment.
- SPEC-0103/FR-NEW-002: IF the manifest `version` is above 2 THEN THE Documents context SHALL read the manifest with the rules of version 2 and add the report line `manifest version <n> is newer than supported 2`.
- SPEC-0103/FR-NEW-003: IF the manifest `version` is not a whole number of 1 or more THEN THE Documents context SHALL ignore the whole manifest and add the report line `manifest ignored: version must be a whole number of 1 or more`.
- SPEC-0103/FR-NEW-016: IF the manifest is not valid JSON, is not an object, or has `pictures` that is not an object, `tables` that is not a list or `headings` that is not a list THEN THE Documents context SHALL ignore the whole manifest and add the report line `manifest ignored: <reason>`.
- SPEC-0103/FR-MOD-002: WHEN the converter gives a manifest THE Documents context SHALL read it by its version (FR-NEW-001).
- SPEC-0103/FR-NEW-023 (partial: the version 1 fields only: caption, caption_failed, page of a picture; the other lines come with their stories): IF a field of a `pictures`, `tables` or `headings` entry has a value of the wrong type THEN THE Documents context SHALL treat that field as absent, keep the rest of the entry and of the manifest, and add the report line for that item.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0103_ docs::`. The tests convert `/report.pdf` through stand in converters (shell script in the program form, axum stub in the HTTP form) with manifests: none, version 1, version 2, 3 and 99, `two`, 0, 2.5, `"2"`, `true`, `null`, -1, 2.0, 1e0, invalid JSON, a list, and a text `pictures`, and assert the exact report text and the captions and pages listed by `fs.list_images`.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0103): No new dependency; the stack is the repository's. **Obliged by:** n/a (no code impact): the spec names no technology. **Rationale:** `serde_json` and the relational layer cover every need. **Alternatives considered:** a JSON schema crate (cannot produce per item report lines). **Source:** existing code. **Evidence:** `Cargo.toml`.
- DEC-002 (SPEC-0103): The manifest is read by hand from a `serde_json::Value` in `docs/manifest.rs`, field by field, returning `ManifestOutcome { manifest, notes }` and never an error; a whole number is a `Number` that is `u64` or `i64` (so `2.0` and `1e0` are refused); lengths count `chars()`. **Obliged by:** SPEC-0103/FR-NEW-001, FR-NEW-003, FR-NEW-016, FR-NEW-023. **Rationale:** serde derive rejects a whole manifest on one wrong field and cannot say which item was wrong. **Alternatives considered:** serde derive with defaults (today's silent discard), a schema validator crate. **Source:** existing code. **Evidence:** `docs/service.rs:746-754,786-790` (silent ignore today), `docs/service.rs:89-102`.
- DEC-003 (SPEC-0103): `Conversion.manifest` keeps the raw `Value` (program form: parsed from `artifacts.json` text, a non JSON text becomes the note `not valid JSON`; HTTP form: the `manifest` value as received) and the parse happens in `PendingSet::converter`, so both forms share one reader. **Obliged by:** SPEC-0103/FR-NEW-018. **Rationale:** one contract, one code path. **Alternatives considered:** parse in each form. **Source:** existing code. **Evidence:** `docs/service.rs:73-77,746,786`, `capture.rs:93`.
- DEC-004 (SPEC-0103): The parser returns notes grouped by item (manifest level lines, per picture target, per table entry number, heading lines); `PendingSet` maps a picture target to its image number once it exists, and `report()` writes the groups in the order of FR-NEW-017 (header, metadata, manifest, images by number, tables by number with `table-<i>` lines before `tables entry <i>` lines, headings). A picture that is not kept emits only its existing line (FR-NEW-023). **Obliged by:** SPEC-0103/FR-NEW-017, FR-NEW-023. **Rationale:** image numbers are known only after capture. **Alternatives considered:** number images in the parser (needs the Markdown parse). **Source:** existing code. **Evidence:** `capture.rs:93-135,149-151`.
### Data model
No schema change.
### Contracts
Rust: `docs::manifest::parse(&Value) -> ManifestOutcome { manifest: Option<Manifest>, notes: Notes }` (never fails); `Manifest.version: u32` (1 for absent or 1, 2 for 2 and newer, with the note `manifest version <n> is newer than supported 2` for n above 2); an invalid version or a not JSON object, a `pictures` that is not an object (any version), `tables` or `headings` that are not lists (version 2 or newer only; ignored without a line for version 1 or none) give `manifest ignored: <reason>` with the reasons `not valid JSON`, `not a JSON object`, `version must be a whole number of 1 or more`, `pictures must be an object`, `tables must be a list`, `headings must be a list`; for a picture entry of any version a wrong typed `caption`, `caption_failed` or `page` is treated as absent with `image-<n>: invalid <field>` and a page that is not a whole number of 1 or more gives `image-<n>: invalid page`; a null is absent with no line. `Conversion.manifest` becomes `Option<serde_json::Value>`: the program form wraps non JSON text into a marker the parser turns into `not valid JSON`; the HTTP form passes the `manifest` value. Report group order: header, (metadata line comes with US-0291), manifest lines in the order ignored, newer, lists, then image lines by number.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/manifest.rs`, `docs/mod.rs` | create / modify: reader, notes, unit tests |
| `crates/core/src/docs/service.rs` | modify: `Conversion.manifest` raw value, both forms |
| `crates/core/src/docs/artifacts/capture.rs` | modify: parse in `PendingSet::converter`, apply version 1 fields, grouped notes in `report()` |
| `crates/core/src/docs/manifest_e2e.rs`, `crates/core/src/lib.rs` | create / modify: test module, program and HTTP stand ins, helpers |
| existing manifest tests in `docs/service.rs` and `docs/artifacts/capture.rs` | modify only the assertion of a silently ignored malformed manifest |
### Existing patterns
Today's reader `manifest_from` and the `Manifest`/`PictureMeta` structs `docs/service.rs:89-102,746-754,786-790`; picture lookup `capture.rs:262-265`; failure line order `capture.rs:93-135,149-151`; stand ins: shell script converters `tools/converter_bound_e2e.rs:88,501`, axum stub `docs/service.rs:1290,1377`, fixtures `tools/artifacts.rs:1054,1470-1482`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0103/E2E-003: a version 1 answer reads as today
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_003_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest has no `version`, one picture with caption `Sales` and page 2; `/report.pdf` of 3 pages
- **When** Alice converts it
- **Then** the report is exactly `1 images, 0 tables`; the caption and page are as today
- **And** no `metadata:` line appears
- **Priority:** High

### SPEC-0103/E2E-004: a version 1 stated explicitly
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_004_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 1 with one picture and a caption
- **When** Alice converts it
- **Then** the report is `1 images, 0 tables`
- **And** an `ocr_text` field in that picture is ignored, no `metadata:` line
- **Priority:** Medium

### SPEC-0103/E2E-005: no manifest at all
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_005_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter returns the Markdown with two `#` headings and two pipe tables and no manifest
- **When** Alice converts it
- **Then** the report is `0 images, 2 tables`
- **And** both tables read `CSV quality: approximate`; no `metadata:` line
- **Priority:** Medium

### SPEC-0103/E2E-010: a newer version is read as version 2
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_010_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 3, a picture with `ocr_text` `Total 42` and an unknown field `layout`; the Markdown has 2 headings
- **When** Alice converts it
- **Then** the conversion succeeds; the report holds `metadata: 1 images with OCR text, 0 tables with exact CSV, 2 headings` and the line `manifest version 3 is newer than supported 2`
- **And** the unknown field changes nothing
- **Priority:** High

### SPEC-0103/E2E-011: a version that is not a whole number
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_011_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` `two`, then `version` 0, then `version` 2.5
- **When** Alice converts it three times
- **Then** each conversion succeeds and each report holds `manifest ignored: version must be a whole number of 1 or more`
- **And** no caption, page, OCR text or CSV of that manifest is kept
- **Priority:** High

### SPEC-0103/E2E-012: a manifest that is not JSON
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_012_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest file holds `{not json`
- **When** Alice converts it
- **Then** the conversion succeeds; the report holds `manifest ignored: not valid JSON`
- **And** the images are kept without caption or page
- **Priority:** High

### SPEC-0103/E2E-013: a manifest that is not an object
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_013_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest holds the list `[1, 2]`
- **When** Alice converts it
- **Then** the report holds `manifest ignored: not a JSON object`
- **And** the conversion succeeded
- **Priority:** Medium

### SPEC-0103/E2E-014: a manifest whose pictures field has the wrong type
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_014_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2 and `pictures` is the text `none`
- **When** Alice converts it
- **Then** the report holds `manifest ignored: pictures must be an object`
- **And** tables and headings fields of the same manifest are not read either
- **Priority:** Medium

### SPEC-0103/E2E-015: a far newer version
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_015_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 99 with a picture OCR text and one headings entry
- **When** Alice converts it
- **Then** the report holds the metadata line and `manifest version 99 is newer than supported 2`
- **And** the version 2 fields are read
- **Priority:** Medium

### SPEC-0103/E2E-016: a newer version note appears once
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_016_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a version 3 manifest with 3 pictures
- **When** Alice converts it
- **Then** the report holds the line `manifest version 3 is newer than supported 2` exactly once
- **And** the report lines about pictures are unchanged
- **Priority:** Medium

### SPEC-0103/E2E-017: a version given as text or as a boolean
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_017_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` "2" (text), then `true`, then `null`, then -1, then 2.0, then 1e0
- **When** Alice converts it six times
- **Then** each report holds `manifest ignored: version must be a whole number of 1 or more`
- **And** no field of the manifest is kept
- **Priority:** High

### SPEC-0103/E2E-018: a version of exactly 1 with version 2 fields
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_018_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `version` 1 with `tables` and `headings` fields
- **When** Alice converts it
- **Then** the new fields are ignored and the report has no metadata line
- **And** the report is exactly as for E2E-004
- **Priority:** Medium

### SPEC-0103/E2E-057: headings of the wrong type
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_057_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2 and `headings` is the text `none`
- **When** Alice converts it
- **Then** the report holds `manifest ignored: headings must be a list`
- **And** the other fields of the manifest are not read
- **Priority:** Medium

### SPEC-0103/E2E-103: headings that is not a list in a version 1 manifest
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_103_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no `version`; `headings` is the text `none`, one picture with a caption
- **When** Alice converts it
- **Then** the report is exactly `1 images, 0 tables`
- **And** the caption is kept
- **Priority:** Medium

## Constraints
- Tables, headings and OCR are not read yet (US-0291 to US-0293); fields of version 2 are ignored whatever their type.
- Never log a converter supplied value.
- No new dependency.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/architecture.md` a short paragraph on manifest versions (full docs at US-0295).
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0290: the manifest reader, versions and manifest level report lines` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0103` and the requirement ids.
