# US-0292: OCR text of images

> Spec: SPEC-0103 (sdlc/specs/SPEC-0103-converter-answer-with-the-full-document-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0291
> min_tier: 1
> Files touched: 7
> Closes spec: no

## Objective
Read and keep the text read inside a picture: `ocr_text` is stored with the image, `ocr_failed` is reported as `image-<n>: OCR unavailable`, a text over 100 000 characters is refused with `image-<n>: OCR text too long`, and the `metadata:` line counts the images that kept a text. A picture that is not kept gets only its own existing line.

## Requirements covered
- SPEC-0103/FR-NEW-004: WHEN a picture of a version 2 manifest carries `ocr_text` THE Documents context SHALL keep that text with the image as metadata.
- SPEC-0103/FR-NEW-005: IF a picture of a version 2 manifest has `ocr_failed` true THEN THE Documents context SHALL add the report line `image-<n>: OCR unavailable`.
- SPEC-0103/FR-NEW-006: IF an `ocr_text` is longer than 100 000 characters THEN THE Documents context SHALL keep no OCR text for that image and add the report line `image-<n>: OCR text too long`.
- SPEC-0103/FR-NEW-023 (partial: the version 2 picture fields ocr_text and ocr_failed, a picture entry that is not an object, null fields and a picture that is not kept): IF a field of a `pictures`, `tables` or `headings` entry has a value of the wrong type THEN THE Documents context SHALL treat that field as absent, keep the rest of the entry and of the manifest, and add the report line for that item.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0103_02 e2e_0103_064 e2e_0103_087 e2e_0103_095 e2e_0103_097 e2e_0103_098 e2e_0103_105 e2e_0103_107`. Tests convert documents with OCR texts of 0, 100 000 and 100 001 characters (and 100 000 accented characters), failed OCR with and without text, wrong typed fields, and a missing picture file, and assert the exact report and that the existing captions and pages are untouched.

## Design excerpt
### Decisions
- DEC-002 (SPEC-0103): The manifest is read by hand from a `serde_json::Value` in `docs/manifest.rs`, field by field, returning `ManifestOutcome { manifest, notes }` and never an error; a whole number is a `Number` that is `u64` or `i64` (so `2.0` and `1e0` are refused); lengths count `chars()`. **Obliged by:** SPEC-0103/FR-NEW-001, FR-NEW-003, FR-NEW-016, FR-NEW-023. **Rationale:** serde derive rejects a whole manifest on one wrong field and cannot say which item was wrong. **Alternatives considered:** serde derive with defaults (today's silent discard), a schema validator crate. **Source:** existing code. **Evidence:** `docs/service.rs:746-754,786-790` (silent ignore today), `docs/service.rs:89-102`.
- DEC-004 (SPEC-0103): The parser returns notes grouped by item (manifest level lines, per picture target, per table entry number, heading lines); `PendingSet` maps a picture target to its image number once it exists, and `report()` writes the groups in the order of FR-NEW-017 (header, metadata, manifest, images by number, tables by number with `table-<i>` lines before `tables entry <i>` lines, headings). A picture that is not kept emits only its existing line (FR-NEW-023). **Obliged by:** SPEC-0103/FR-NEW-017, FR-NEW-023. **Rationale:** image numbers are known only after capture. **Alternatives considered:** number images in the parser (needs the Markdown parse). **Source:** existing code. **Evidence:** `capture.rs:93-135,149-151`.
- DEC-005 (SPEC-0103): The `metadata:` line is written when the manifest version is 2 or newer after a successful parse: `k` = images kept with non empty OCR text, `j` = tables labelled exact because of a manifest CSV, `h` = headings of the stored structure; a manifest that is ignored (including an invalid version) counts as no manifest. **Obliged by:** SPEC-0103/FR-NEW-017, FR-NEW-003, FR-MOD-001. **Rationale:** matches the spec's counts one to one. **Alternatives considered:** count from the stored rows after commit (adds a read). **Source:** spec. **Evidence:** n/a.
- DEC-006 (SPEC-0103): Storage is additive in the volume meta store: nullable `doc_images.ocr_text`, `doc_tables.csv_text`, `doc_tables.page`, and a table `doc_headings`, all written by the existing `commit_artifact_set` transaction and deleted with their set by `sweep_stale_artifact_sets`; `quality` is `exact` for a manifest CSV. **Obliged by:** SPEC-0103/FR-NEW-004, FR-NEW-008, FR-NEW-011, FR-NEW-012, FR-NEW-013, FR-NEW-019, FR-NEW-020. **Rationale:** metadata of a set is replaced with the set, and old rows stay valid with nulls. **Alternatives considered:** one JSON column per set (not queryable by BL-0167), a separate store. **Source:** existing code. **Evidence:** `storage/meta.rs:114-140,362-378,932-1000,1060-1125`.
- DEC-010 (SPEC-0103): The storage charge `PendingSet::bytes()` adds the OCR text bytes, the exact CSV bytes (instead of the derived CSV length for such a table) and the title bytes of every heading, so the existing admission and refusal texts (`ERR_PROJECT_QUOTA_EXCEEDED`, `session write quota of <n> bytes exceeded`) apply unchanged. **Obliged by:** SPEC-0103/FR-NEW-022. **Rationale:** one charge, as today. **Alternatives considered:** a second charge. **Source:** existing code. **Evidence:** `capture.rs:137-145`, `core/fs_ops.rs:799-875`.
### Data model
New nullable column `doc_images.ocr_text` Text (`ColumnMigration`), written by the image insert, deleted with the set; the insert column list and its placeholders change (`DOC_IMAGE_COLS`).
### Contracts
Picture fields of version 2: `ocr_text` text (null or empty is none; at most 100 000 `chars()`), `ocr_failed` bool; lines in the picture group order: `caption unavailable`, `invalid <field>` and `not an object`, `OCR text too long`, `OCR unavailable`; an invalid picture `page` gives `image-<n>: invalid page` whatever the source format; a picture that is not kept (`picture unavailable`, `unsupported picture format`) gets no OCR or `invalid` line and is not counted in `images with OCR text`; a text with both `ocr_text` and `ocr_failed` keeps the text and adds the line. The storage charge adds the OCR text bytes; OCR text is not added to the document text, view or search.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/manifest.rs` | modify: picture fields and notes |
| `crates/core/src/docs/artifacts/capture.rs` | modify: `SetImage.ocr_text`, counts, lines, charge, commit row |
| `crates/core/src/storage/meta.rs` | modify: column, `NewDocImage.ocr_text`, insert |
| `crates/core/src/storage/rel/schema.rs`, `crates/core/tests/fixtures/schema_*.sql` | modify / regenerate |
| `crates/core/src/docs/manifest_e2e.rs` | modify: these tests |
### Existing patterns
Image build `capture.rs:93-135,256-298`; failure line wording `capture.rs:105`; image row `NewDocImage` and `DOC_IMAGE_COLS` `storage/meta.rs:129,398,990`; the schema golden steps of US-0291.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0103/E2E-020: OCR text and caption together
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_020_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); one picture with caption `Sales`, page 2 and `ocr_text` `Total 42`, `version` 2; `/report.pdf`; the Markdown has 1 heading
- **When** Alice converts it
- **Then** the report holds `metadata: 1 images with OCR text, 0 tables with exact CSV, 1 headings`
- **And** `fs.list_images` still shows the caption `Sales` and page 2
- **Priority:** High

### SPEC-0103/E2E-021: a picture without OCR text
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_021_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); two pictures, only the first with `ocr_text`, `version` 2
- **When** Alice converts it
- **Then** the report holds `metadata: 1 images with OCR text`
- **And** the second image is kept as it is today
- **Priority:** Medium

### SPEC-0103/E2E-022: OCR text of an empty string
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_022_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with `ocr_text` empty, `version` 2
- **When** Alice converts it
- **Then** the count of images with OCR text is 0
- **And** no failure line
- **Priority:** Medium

### SPEC-0103/E2E-023: the converter says the OCR failed
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_023_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); two pictures, the second with `ocr_failed` true and no `ocr_text`, `version` 2
- **When** Alice converts it
- **Then** the report holds the line `image-2: OCR unavailable`
- **And** the conversion succeeds and the first image is intact
- **Priority:** High

### SPEC-0103/E2E-024: OCR failure and caption failure on one picture
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_024_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); picture 1 with `caption_failed` true and `ocr_failed` true, `version` 2
- **When** Alice converts it
- **Then** the report lists `image-1: caption unavailable` then `image-1: OCR unavailable`
- **And** both lines appear once
- **Priority:** Medium

### SPEC-0103/E2E-025: OCR text and OCR failure together
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_025_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with `ocr_text` `Total 42` and `ocr_failed` true, `version` 2
- **When** Alice converts it
- **Then** the text is kept, counted in `images with OCR text`, and the line `image-1: OCR unavailable` is added
- **And** the failure flag does not discard the text
- **Priority:** Medium

### SPEC-0103/E2E-026: OCR text too long
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_026_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with an `ocr_text` of 100 001 characters, `version` 2
- **When** Alice converts it
- **Then** the report holds `image-1: OCR text too long`; the count of images with OCR text is 0
- **And** a text of exactly 100 000 characters is kept (counted)
- **Priority:** High

### SPEC-0103/E2E-027: OCR text with accents and emoji
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_027_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with `ocr_text` `Résumé 42 ✓ 日本語`, `version` 2
- **When** Alice converts it
- **Then** the count of images with OCR text is 1
- **And** the conversion report has no failure line
- **Priority:** Medium

### SPEC-0103/E2E-028: exactly 100 000 characters of OCR text
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_028_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); an `ocr_text` of exactly 100 000 characters
- **When** Alice converts it
- **Then** the count of images with OCR text is 1 and no failure line
- **And** a count of characters, not bytes: 100 000 accented characters are kept
- **Priority:** Medium

### SPEC-0103/E2E-029: two pictures with too long OCR text
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_029_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); pictures 1 and 3 have an `ocr_text` of 100 001 characters, picture 2 a valid one
- **When** Alice converts it
- **Then** the report holds `image-1: OCR text too long` then `image-3: OCR text too long`; the count of images with OCR text is 1
- **And** picture 2 is intact
- **Priority:** High

### SPEC-0103/E2E-087: OCR text is not searched or chunked as a document line
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_087_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with `ocr_text` `UniqueOcrWord`
- **When** Alice converts it and searches `UniqueOcrWord`
- **Then** search finds nothing
- **And** the document view is unchanged (BL-0167 adds OCR to the view)
- **Priority:** Medium

### SPEC-0103/E2E-095: a picture field of the wrong type
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_095_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); picture 1 has `ocr_text` 42 and picture 2 has `ocr_failed` the text `yes`, `version` 2
- **When** Alice converts it
- **Then** the report holds `image-1: invalid ocr_text` and `image-2: invalid ocr_failed`; the count of images with OCR text is 0
- **And** the captions and pages are kept
- **Priority:** High

### SPEC-0103/E2E-097: a picture entry that is not an object, and a bad picture page
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_097_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `pictures` holds the text `x` for the first picture and a page 0 for the second, `version` 2
- **When** Alice converts it
- **Then** the report holds `image-1: not an object` and `image-2: invalid page`
- **And** both images are kept
- **Priority:** Medium

### SPEC-0103/E2E-098: a wrong type in a version 1 manifest
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_098_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no `version`; picture 1 has `caption` 42 and a valid page 2; the manifest also has `ocr_text` 7
- **When** Alice converts it
- **Then** the report holds `image-1: invalid caption` and no `metadata:` line
- **And** the page 2 is kept and the manifest is not discarded
- **Priority:** Medium

### SPEC-0103/E2E-105: an invalid picture page on a source that is not paged
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_105_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/doc.docx`; picture 1 has `page` 0
- **When** Alice converts it
- **Then** the report holds `image-1: invalid page`
- **And** a valid page 2 on the same source gives no line and no page is kept
- **Priority:** Medium

### SPEC-0103/E2E-107: a picture that is not kept
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_107_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); picture 1 refers to a file missing from the bundle and has `ocr_failed` true, `ocr_text` 42 and `caption` 5
- **When** Alice converts it
- **Then** the report holds `image-1: picture unavailable` and no OCR or `invalid` line for that picture
- **And** the count of images with OCR text is 0
- **Priority:** Medium

### SPEC-0103/E2E-064: pictures whose targets do not match the manifest
- Level / Driver / Location: functional e2e / MCP handlers on the real conversion path with a stand in converter (program form or HTTP form) / `crates/core/src/docs/manifest_e2e.rs` (`e2e_0103_064_*`)
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest describes `fig/x.png` with OCR text, the Markdown refers to `fig/a.png`
- **When** Alice converts it
- **Then** the picture `fig/a.png` is kept without caption or OCR text and the count of images with OCR text is 0
- **And** no line about `fig/x.png` is added
- **Priority:** Medium

## Constraints
- No read tool for OCR text (BL-0167).
- A wrong typed field is absent, never a manifest rejection.
- Keep the image file and caption rules untouched.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/architecture.md` (OCR text kept per image).
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0292: OCR text of images` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0103` and the requirement ids.
