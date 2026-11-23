# The converter answer carries the full document metadata: functional specification

> Id: SPEC-0103
> Nature: FEAT
> Depth: L
> Depth evidence: FEAT / depth L: 4 areas (converter contract, artifact storage, conversion paths, report) plus new persisted concepts (OCR text, exact CSV, structure), 24 requirements
> Status: implemented
> Generated on: 2026-11-21
> Project: existing
> From backlog: BL-0166
> Depends on: none
> Security: internal finding
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
A document converter (a program or an HTTP service) hands the platform the Markdown of a document, its picture files and a manifest. Today the manifest carries only a caption, a failure flag and a page per picture; tables get an approximate CSV rebuilt from the Markdown, nothing keeps the text read inside a picture, and no structure of the document is kept. This increment defines version 2 of the manifest: per picture the OCR text, per table an exact CSV file, caption and page, and the heading structure. The platform reads and stores all of it as metadata of the document, whatever the form of the converter, and tells the caller in the conversion report what it kept and what it dropped. Version 1 manifests, and converters with no manifest, keep working exactly as today. The tools that read OCR text, structure and raw images are the next increment (BL-0167).

## 2. Current State (MANDATORY, every depth)
When a document is converted, the platform keeps the Markdown as a file next to it and, as metadata, its images (the picture file, a caption, a page) and its tables (the cells, a Markdown form and a CSV form derived from the Markdown, labelled `CSV quality: approximate`). The converter can give a manifest, but the platform reads only a caption, a failure flag and a page per picture; a missing field means empty. A manifest that is not valid is ignored without any trace for the caller. The conversion returns a report: `<N> images, <M> tables`, then one line per item that failed. No text read inside a picture is kept, no table CSV supplied by the converter is kept, and no structure of the document is stored (headings exist only as lines of the Markdown). The tools that list and read images and tables, and the document view, show captions, pages and tables only. Converted documents are the source of the text that the chunk sets cut.

### 2.1 What users can do today
- Convert a document and get its images with captions and pages, and its tables as Markdown and approximate CSV (14.1 rows 1 to 4).
- See which items failed in the conversion report (14.1 row 5).
- The real converter writes no manifest yet; the owner will test with new converters (14.1 row 8).

### 2.2 Specs governing this area
- SPEC-0019 (document artifacts): FR-NEW-030 (the conversion report), FR-NEW-033 (what the converter's output yields), FR-NEW-002 (CSV quality label), FR-NEW-015 (malformed tables). This increment extends FR-NEW-030 and FR-NEW-033 (FR-MOD-001); SPEC-0019 is not edited.
- SPEC-0006 and SPEC-0089: the converter's forms, limits and failures; unchanged.
- SPEC-0101: chunk sets read the text of a converted document; unchanged.

### 2.3 What is verified today
Tests exercise the converter forms, the manifest with captions and pages, the capture of images and tables, the report and the artifact tools against stubs (14.1 row 9). No test exercises a version, OCR text, a converter supplied CSV or a structure.

## 3. Scope
### 3.1 In scope
- Manifest version 2: version rule, OCR text per picture, per table CSV file, caption and page, heading structure.
- Reading it identically from the program form and the HTTP form.
- Storing OCR text, exact CSV, table pages and structure as metadata of the document, replaced with the rest of the set.
- The report: a metadata line, and a line for every dropped or refused field.
- Version 1 and no manifest unchanged; documents converted before untouched.
- Size and quota rules applied to the new data.

### 3.2 Out of scope, and why
- Tools and routes that read OCR text, raw images, structure and a full view: BL-0167 (so OCR and structure content is asserted there).
- Configuring converters in the database: BL-0165.
- Using OCR text or structure in search or in chunk sets: later specs; OCR text is not added to the document text here.
- Making artifacts follow a document through file operations: BL-0038.
- Converting at upgrade, or a command that converts every document: not asked.
- A converter that produces the manifest: the owner is testing new converters; this increment only defines and reads the contract.

### 3.3 Constraints imposed by the context
- The same contract for the program form and the HTTP form (imposed by the owner).
- Version the manifest so an old converter keeps working (imposed by the owner).
- A converter's answer is untrusted input: it must never make the platform read outside the converter's output (security rule of SPEC-0019).

## 4. Actors
| Actor | Role |
|---|---|
| Member | converts documents and reads their tables and images |
| Converter | the program or the HTTP service that produces the answer; not a person |
| Operator | runs converters and reads the conversion report |

## 4.5 Bounded contexts (MANDATORY at M and L)
| Context | What it owns | Key concepts |
|---|---|---|
| Documents | converted documents and their metadata | manifest, version, image, OCR text, table, exact CSV, heading, structure, conversion report |
| Projects | storage limits | quota |

## 5. Usage scenarios
### SC-001: converting with a full version 2 answer
**Actor:** Member
**Preconditions:** project `proj` with Alice (`alice@example.com`) as member; a converter that returns Markdown, two pictures with captions and OCR text, two tables (the first with a CSV file) and four headings.
**Flow:**
1. Alice converts `/report.pdf` (or uploads it with the conversion option) → the conversion succeeds.
2. The report reads `2 images, 2 tables` then `metadata: 2 images with OCR text, 1 tables with exact CSV, 4 headings`.
3. She lists the tables → the first reads `CSV quality: exact`, the second `approximate`; she reads the first as CSV → the converter's exact file.
**Postconditions:** the document holds its images, OCR text, tables, exact CSV and structure as one set.
**Exceptions:** a non member is refused (E2E-081).

### SC-002: the program form and the HTTP form are the same
**Actor:** Member
**Flow:** the same content arrives as an output folder or as one JSON answer with base64 files; both give the same report and the same stored data.
**Exceptions:** a base64 error fails the conversion with the existing text; unknown keys are ignored.

### SC-003: OCR text
**Actor:** Member
**Flow:** a picture can carry `ocr_text` and `ocr_failed`; the text is kept, a failure is reported as `image-<n>: OCR unavailable`, a text over 100 000 characters is refused as `image-<n>: OCR text too long`.

### SC-004: tables with exact CSV
**Actor:** Member
**Flow:** table entries apply in order to the document's tables; a `csv` file makes a table exact; an unusable file keeps the table approximate and is reported; extra entries are reported.
**Exceptions:** a CSV reference outside the bundle is never read.

### SC-005: the structure of the document
**Actor:** Member
**Flow:** headings supplied by the converter are kept; otherwise they are derived from the Markdown; invalid entries are dropped with a line; more than 10 000 are cut.

### SC-006: versions and malformed manifests
**Actor:** Operator
**Flow:** no version or version 1 reads as today; version 2 reads the new fields; a newer version reads as 2 with a note; an invalid version or a malformed manifest is ignored with a line and the conversion still succeeds.

### SC-007: a converter that sends less
**Actor:** Member
**Flow:** missing OCR means no OCR text, missing CSV means approximate, missing headings mean derived headings; nothing fails.

### SC-008: replacing, upgrading, other paths
**Actor:** Member, Operator
**Flow:** converting again replaces the whole set; a failed conversion keeps the previous set; documents converted before are untouched; the built in extraction is unchanged; a quota overflow keeps nothing.

### SC-009: limits
**Actor:** Operator
**Flow:** the converter output limit counts the CSV files; no CSV file lands in the project's files; unsafe file names are refused.

## 6. Functional requirements
### 6.1 New
#### FR-NEW-001 [EARS-E]: the manifest version
> WHEN the converter's answer carries a manifest THE Documents context SHALL read it by its `version`: absent or 1 as the manifest of today, 2 with the fields of this increment.

- **Business rules:** `version` is a whole number of 1 or more; a whole number is a JSON number written without a fraction or an exponent (`2.0` and `1e0` are not whole numbers). An absent `version` and `1` read exactly as before this increment: only `pictures` with `caption`, `caption_failed` and `page` are read, the fields of version 2 are ignored whatever their type, and the report is unchanged except for the lines of FR-NEW-016 and FR-NEW-023.
- **Business rules:** A manifest of version 2 can carry, besides `pictures`, the lists `tables` and `headings` and, per picture, `ocr_text` and `ocr_failed`. Unknown fields are ignored at every level.
- **Business rules:** A manifest is read the same way whether it comes from the program form or from the HTTP form (FR-NEW-018).
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-002 [EARS-O]: a version newer than supported
> IF the manifest `version` is above 2 THEN THE Documents context SHALL read the manifest with the rules of version 2 and add the report line `manifest version <n> is newer than supported 2`.

- **Business rules:** `<n>` is the version given. The line appears once per conversion.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-003 [EARS-O]: a version that is not valid
> IF the manifest `version` is not a whole number of 1 or more THEN THE Documents context SHALL ignore the whole manifest and add the report line `manifest ignored: version must be a whole number of 1 or more`.

- **Business rules:** Text, a boolean, null, 0, a negative number, a fraction and a number written with an exponent are not valid.
- **Business rules:** A manifest that is ignored is treated as no manifest: the conversion succeeds, nothing of the manifest is kept, and the report has no `metadata:` line.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-004 [EARS-E]: OCR text of an image
> WHEN a picture of a version 2 manifest carries `ocr_text` THE Documents context SHALL keep that text with the image as metadata.

- **Business rules:** `ocr_text` is text of 0 to 100 000 characters (characters, not bytes). An empty text is no OCR text.
- **Business rules:** The caption, the page and the image file are kept as today; OCR text does not change them.
- **Business rules:** OCR text is not added to the document text, to the document view or to search in this increment.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-005 [EARS-O]: OCR that failed
> IF a picture of a version 2 manifest has `ocr_failed` true THEN THE Documents context SHALL add the report line `image-<n>: OCR unavailable`.

- **Business rules:** `ocr_failed` defaults to false. A picture with both `ocr_text` and `ocr_failed` true keeps its text and gets the line.
- **Business rules:** When the same picture also has `caption_failed` true, the caption line comes first, then the OCR line.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-006 [EARS-O]: OCR text that is too long
> IF an `ocr_text` is longer than 100 000 characters THEN THE Documents context SHALL keep no OCR text for that image and add the report line `image-<n>: OCR text too long`.

- **Business rules:** A text of exactly 100 000 characters is kept. The other fields of the image are kept.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-007 [EARS-E]: table entries apply in document order
> WHEN a version 2 manifest carries a `tables` list THE Documents context SHALL apply its entry number `i` to the `i`th table of the document, counting every pipe table of the Markdown in reading order.

- **Business rules:** An entry is an object with the optional fields `csv`, `caption` and `page`. A table with no entry is kept as today.
- **Business rules:** A table that is malformed (SPEC-0019/FR-NEW-015) is not kept: its entry is not used and its report line is the one of today.
- **Business rules:** An entry that is not an object is ignored with the report line `tables entry <i>: not an object`.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-008 [EARS-E]: exact CSV of a table
> WHEN a table entry has `csv` naming a file of the answer's bundle THE Documents context SHALL keep that file's content as the table's CSV and label the table `CSV quality: exact`.

- **Business rules:** The Markdown form of the table is still built from the pipe table; the CSV form is the file's content, character for character.
- **Business rules:** A bundle file is named as pictures are named: a relative path inside the bundle; the file must be valid UTF-8 text of at most 5 000 000 characters.
- **Business rules:** A table without `csv` keeps the label `CSV quality: approximate`.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-009 [EARS-O]: CSV that cannot be used
> IF the file named by a table entry's `csv` is missing, is not valid UTF-8, is longer than 5 000 000 characters or names a place outside the bundle THEN THE Documents context SHALL keep the table as approximate and add the report line `table-<i>: CSV unavailable`.

- **Business rules:** A place outside the bundle is a URL, an absolute path, a name with a drive or colon, or a relative path that climbs above the bundle (`../`). Nothing outside the bundle is ever read.
- **Business rules:** A file of exactly 5 000 000 characters is kept.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-010 [EARS-O]: more entries than tables
> IF the `tables` list has more entries than the document has tables THEN THE Documents context SHALL apply the entries that match, ignore the others and add the report line `manifest lists <n> tables, the document has <m>`.

- **Business rules:** `<n>` is the number of entries, `<m>` the number of pipe tables. With fewer entries than tables, or as many, no line is added.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-011 [EARS-E]: caption and page of a table
> WHEN a table entry carries a `caption` or a `page` THE Documents context SHALL use that caption for the table when it is not empty, and keep that page with the table.

- **Business rules:** Without a manifest caption the caption is the Markdown caption of today (`Table <n>: <text>`) or empty.
- **Business rules:** `page` is a whole number of 1 or more; any other value is dropped with the report line `tables entry <i>: invalid page` and the other fields of the entry are kept.
- **Business rules:** The page of a table is kept for any source format of the document (a picture's page is kept only for paged formats, as today); it is metadata only in this increment and is read by BL-0167.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-012 [EARS-E]: headings supplied by the converter
> WHEN a version 2 manifest carries a `headings` list THE Documents context SHALL keep that list, in its order, as the structure of the document.

- **Business rules:** An entry is an object with `level` (a whole number from 1 to 6), `title` (1 to 1 000 characters after trimming) and optionally `page` (a whole number of 1 or more).
- **Business rules:** The stored title is the trimmed title, otherwise as written (accents, markup).
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-013 [EARS-U]: the structure is always kept
> The Documents context SHALL keep a structure for every converted document: the converter's `headings` when supplied, otherwise the headings found in the Markdown.

- **Business rules:** A Markdown heading is a line of 1 to 6 `#`, then a space or a tab, then a non empty title; lines inside a fenced code block are not headings.
- **Business rules:** The structure of a document without a manifest, with a version 1 manifest or with a version 2 manifest without `headings` is derived from the Markdown.
- **Business rules:** A `headings` list counts as supplied when it is present, even when it is empty or when every entry is invalid: the structure is then the valid entries of that list and no Markdown heading is derived.
- **Business rules:** A closing run of `#` separated from the title by a space is not part of a derived title (`C#` keeps its mark). Derived headings obey the limits of FR-NEW-014 and FR-NEW-015: a derived title of more than 1 000 characters is dropped without a line, and more than 10 000 derived headings keep the first 10 000 with the line `headings over 10000 dropped: <n>` added only when the manifest is of version 2 or newer.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-014 [EARS-O]: a heading entry that is not valid
> IF a `headings` entry fails one of the rules of FR-NEW-012 THEN THE Documents context SHALL drop that entry and add the report line `heading <i>: invalid`.

- **Business rules:** The rules of FR-NEW-012: (a) the entry is an object; (b) `level` is a whole number from 1 to 6; (c) `title` is 1 to 1 000 characters after trimming; (d) `page`, when present, is a whole number of 1 or more.
- **Business rules:** `<i>` is the position of the entry in the list, from 1. The other entries are kept.
- **Business rules:** At most 100 `heading <i>: invalid` lines are written; further invalid entries give one line `heading lines over 100 dropped: <n>`.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-015 [EARS-O]: too many headings
> IF a `headings` list has more than 10 000 valid entries THEN THE Documents context SHALL keep the first 10 000 and add the report line `headings over 10000 dropped: <n>`.

- **Business rules:** `<n>` is the number of entries dropped. Exactly 10 000 adds no line.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-016 [EARS-O]: a manifest that is malformed
> IF the manifest is not valid JSON, is not an object, or has `pictures` that is not an object, `tables` that is not a list or `headings` that is not a list THEN THE Documents context SHALL ignore the whole manifest and add the report line `manifest ignored: <reason>`.

- **Business rules:** Reasons, verbatim: `not valid JSON`, `not a JSON object`, `pictures must be an object`, `tables must be a list`, `headings must be a list`.
- **Business rules:** The `manifest ignored: <reason>` line is added for any version, absent included. `pictures must be an object` applies to every version; `tables must be a list` and `headings must be a list` apply to version 2 or newer only, a version 1 or versionless manifest ignoring those fields whatever their type.
- **Business rules:** The conversion succeeds; as with no manifest, images and tables are kept without captions, pages, OCR text or CSV, and the structure is derived.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-017 [EARS-E]: the metadata line of the report
> WHEN a conversion with a version 2 manifest (or a newer one) completes THE Documents context SHALL add to its report, right after the line `<N> images, <M> tables`, the line `metadata: <k> images with OCR text, <j> tables with exact CSV, <h> headings`.

- **Business rules:** `<k>` counts images that kept a non empty OCR text, `<j>` tables labelled exact because of a manifest CSV, `<h>` the headings of the stored structure (supplied or derived).
- **Business rules:** Report order: (1) `<N> images, <M> tables`; (2) `metadata:`; (3) `manifest ignored: <reason>`, `manifest version <n> is newer than supported 2`, `manifest lists <n> tables, the document has <m>`, in that order; (4) per image by number: `caption unavailable`, `invalid <field>` and `not an object` lines, `OCR text too long`, `OCR unavailable`; (5) per table by number: `malformed table`, `CSV unavailable`, then the `tables entry <i>` lines of that number; (6) `heading <i>: invalid` lines by position, `heading lines over 100 dropped: <n>`, then `headings over 10000 dropped: <n>`.
- **Business rules:** With no manifest, a version 1 manifest or an ignored manifest the report has no `metadata:` line and is exactly as before this increment, except for the lines of FR-NEW-016 and FR-NEW-023.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-018 [EARS-U]: one contract for both forms
> The Documents context SHALL read the same fields, apply the same rules and produce the same stored data whether the converter answers as a program (an output folder with the manifest) or as an HTTP service (one JSON answer).

- **Business rules:** Program form: `document.md`, the manifest file and every regular file below the output folder; HTTP form: the Markdown, the manifest and the files (base64) of one JSON answer.
- **Business rules:** Files not referenced by the manifest or the Markdown are kept nowhere. A file of the HTTP form with invalid base64 fails the conversion with the existing text.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-019 [EARS-E]: a conversion replaces the whole set
> WHEN a document is converted again THE Documents context SHALL replace the images, tables, OCR text, CSV and structure of the document as one set.

- **Business rules:** A document whose file changes loses its set as today (`No artifacts: document not extracted`).
- **Business rules:** Converting twice with the same answer gives the same stored data.
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-020 [EARS-U]: nothing is converted again at upgrade
> The Documents context SHALL keep the images and tables of documents converted before this increment as they are, and SHALL NOT convert any document because of the upgrade.

- **Business rules:** Such a document has no OCR text and no exact CSV; its structure is not stored until it is converted again.
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-021 [EARS-U]: the built in extraction is unchanged
> The Documents context SHALL keep the behavior and the report of the built in text extraction unchanged.

- **Business rules:** The built in extraction keeps no OCR text, labels its tables exact and adds no `metadata:` line.
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-022 [EARS-U]: size and storage rules
> The Documents context SHALL count the files of the answer and the manifest in the converter output limit, count the OCR text, the CSV and the structure in the storage charged for the document's metadata, and SHALL leave no CSV file in the project's files.

- **Business rules:** The converter output limit counts, in bytes, the Markdown, every file of the bundle and the manifest, in both forms, so both forms give the same total for the same content; in the HTTP form the Markdown and the manifest are counted as the text received. OCR text and structure count as part of the manifest's bytes.
- **Business rules:** Over the converter output limit the conversion fails with the existing text `document service output is over the doc_service.max_input_bytes limit of <limit>`; over the project's storage it is refused with the existing quota error `ERR_PROJECT_QUOTA_EXCEEDED`; over the session write allowance with `session write quota of <n> bytes exceeded`; in every case nothing is kept.
- **Business rules:** A bundle exactly at the limit is accepted. A heading is charged the bytes of its title.
- **Scenario:** SC-009
- **Priority:** Must

#### FR-NEW-023 [EARS-O]: a field of the wrong type
> IF a field of a `pictures`, `tables` or `headings` entry has a value of the wrong type THEN THE Documents context SHALL treat that field as absent, keep the rest of the entry and of the manifest, and add the report line for that item.

- **Business rules:** Lines: `image-<n>: invalid <field>` (field one of `caption`, `caption_failed`, `page`, `ocr_text`, `ocr_failed`), `tables entry <i>: invalid <field>` (field one of `csv`, `caption`; the page line is `tables entry <i>: invalid page` of FR-NEW-011).
- **Business rules:** A `pictures` entry that is not an object is treated as an empty entry and gives `image-<n>: not an object`. A `csv` that is null or the empty text is no `csv`, with no line.
- **Business rules:** A picture `page` that is not a whole number of 1 or more is dropped with `image-<n>: invalid page`, whatever the source format (a valid page is kept only for paged formats, as today).
- **Business rules:** A field whose value is `null` is treated as absent, with no line, for every field of a `pictures`, `tables` or `headings` entry; for `level` and `title` of a heading `null` makes the entry invalid (FR-NEW-014).
- **Business rules:** A picture that is not kept (the existing lines `image-<n>: picture unavailable` or `image-<n>: unsupported picture format <ext>`) gets only that line: none of the OCR, `invalid <field>` or `not an object` lines of its manifest entry is added, and it is not counted in `images with OCR text`.
- **Business rules:** For a version 1 or versionless manifest only `caption`, `caption_failed` and `page` of a picture are read, and so only they can give these lines; a wrong type there no longer discards the manifest.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-024 [EARS-O]: a failed conversion changes nothing
> IF a conversion fails THEN THE Documents context SHALL keep the previous set of the document as it was.

- **Business rules:** A document never converted stays without a set (`No artifacts: document not extracted`).
- **Scenario:** SC-008
- **Priority:** Must

### 6.2 Modified
#### FR-MOD-001 [EARS-E]: the conversion report (modifies SPEC-0019/FR-NEW-030)
> WHEN a conversion with a version 2 manifest completes THE Documents context SHALL return the report with the metadata line of FR-NEW-017.

- **Before:** SPEC-0019/FR-NEW-030: the report is `<N> images, <M> tables` then one line per failed item.
- **After:** the report of a version 2 conversion adds the `metadata:` line after the first line and the lines of this increment; a version 1 or absent manifest gives the same report as before except for the lines of FR-NEW-016 and FR-NEW-023.
- **Reason:** the converter's metadata (OCR, CSV, structure) must be observable.
- **Scenario:** SC-001, SC-006

#### FR-MOD-002 [EARS-E]: manifest reading by version (modifies SPEC-0019/FR-NEW-033)
> WHEN the converter gives a manifest THE Documents context SHALL read it by its version (FR-NEW-001).

- **Before:** SPEC-0019/FR-NEW-033: the manifest holds `pictures` with a caption, a failure flag and a page; a manifest that does not parse is ignored without a line.
- **After:** the manifest has a version; a manifest that does not parse is ignored with a report line (FR-NEW-016); a wrong typed field discards that field only (FR-NEW-023).
- **Reason:** the contract must be versioned so that an old converter keeps working.
- **Scenario:** SC-006

### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
- A conversion with 1 000 pictures, 500 tables and 10 000 headings is stored in under 10 seconds beyond the converter's own time.
### 7.2 Security and access
- Only members of the project convert its documents (E2E-081); a CSV or picture reference never reads outside the converter's output (FR-NEW-009).
### 7.5 Reliability
- A conversion stores its whole set or nothing (FR-NEW-019, E2E-071, E2E-085).
### 7.6 Operability
- An operator reading the conversion report can tell, for every dropped field, which item and why (FR-NEW-002 to 016).

## 8. Edge cases
| Category | Case | Expected behavior | Test |
|---|---|---|---|
| Empty states | no manifest; manifest with nothing; no pictures | as today or zeros in the metadata line | E2E-005, E2E-061, E2E-063 |
| Boundary values | OCR 100 000 and 100 001; CSV 5 000 000 and 5 000 001; headings 10 000 and 10 001; level 1 and 6; title 1 000 and 1 001; output cap exactly and plus one | exact bounds | E2E-026, E2E-028, E2E-034, E2E-055, E2E-058, E2E-054, E2E-091, E2E-065, E2E-076 |
| Concurrent actions | two members read the same set | same data | E2E-083 |
| Access states | non member | refused | E2E-081 |
| Data variations | accents, emoji, markup in titles and OCR | stored as written | E2E-027, E2E-056 |
| Partial operations | a failed re-conversion; a quota overflow | nothing changes | E2E-071, E2E-075, E2E-085 |
| Dependency failures | converter timeout, base64 error | existing failures, nothing kept | E2E-085, E2E-079 |
| Ordering and timing | version read first; entries in order | stated order | E2E-010, E2E-037, E2E-043 |
| Navigation | re-listing after restart | unchanged | E2E-069 |
| Exhaustion | quota and output cap | refused, nothing kept | E2E-075, E2E-076 |
| Slow connectivity | not applicable: no interactive flow |  |  |

## 9. Acceptance tests (the contract)
> Behavioral and technology agnostic. /sdlc-design maps each test to a driver. An implementing agent uses this as its definition of done.
> Convention: unless a test says otherwise, the manifest of a Given says `version` 2. "converts" means the document is converted by the configured converter, by the conversion option of an upload or by the conversion tool; the report is the conversion report the caller receives. "A manifest says" describes the manifest the converter returns. In the program form the manifest, the Markdown and the files are in the output folder; in the HTTP form they are in one JSON answer. The words "OCR text", "headings" and "CSV" in a Then are read through the report line `metadata:` and through the existing table tools, because the tools that read OCR text and structure are BL-0167.

### 9.1 Summary
| Test | Action | Category | Scenario | Requirements | Priority |
|---|---|---|---|---|---|
| E2E-001 | New | journey | SC-001 | FR-NEW-001, FR-NEW-004, FR-NEW-008, FR-NEW-012, FR-NEW-017, FR-NEW-018 | Critical |
| E2E-002 | New | journey | SC-002 | FR-NEW-018, FR-NEW-001, FR-NEW-004, FR-NEW-008, FR-NEW-012, FR-NEW-017 | Critical |
| E2E-003 | New | journey | SC-006 | FR-NEW-001, FR-NEW-017 | High |
| E2E-004 | New | edge | SC-006 | FR-NEW-001, FR-NEW-017 | Medium |
| E2E-005 | New | edge | SC-007 | FR-NEW-001, FR-NEW-017, FR-NEW-013 | Medium |
| E2E-010 | New | edge | SC-006 | FR-NEW-002 | High |
| E2E-011 | New | failure | SC-006 | FR-NEW-003 | High |
| E2E-012 | New | failure | SC-006 | FR-NEW-016 | High |
| E2E-013 | New | failure | SC-006 | FR-NEW-016 | Medium |
| E2E-014 | New | edge | SC-006 | FR-NEW-016 | Medium |
| E2E-015 | New | edge | SC-006 | FR-NEW-002 | Medium |
| E2E-016 | New | edge | SC-006 | FR-NEW-002 | Medium |
| E2E-017 | New | failure | SC-006 | FR-NEW-003 | High |
| E2E-018 | New | edge | SC-006 | FR-NEW-001, FR-NEW-003 | Medium |
| E2E-020 | New | journey | SC-003 | FR-NEW-004 | High |
| E2E-021 | New | edge | SC-003 | FR-NEW-004 | Medium |
| E2E-022 | New | edge | SC-003 | FR-NEW-004 | Medium |
| E2E-023 | New | failure | SC-003 | FR-NEW-005 | High |
| E2E-024 | New | edge | SC-003 | FR-NEW-005 | Medium |
| E2E-025 | New | edge | SC-003 | FR-NEW-005, FR-NEW-004 | Medium |
| E2E-026 | New | failure | SC-003 | FR-NEW-006 | High |
| E2E-027 | New | edge | SC-003 | FR-NEW-004 | Medium |
| E2E-028 | New | edge | SC-003 | FR-NEW-006 | Medium |
| E2E-029 | New | failure | SC-003 | FR-NEW-006 | High |
| E2E-030 | New | journey | SC-004 | FR-NEW-007, FR-NEW-008 | Critical |
| E2E-031 | New | edge | SC-004 | FR-NEW-011 | Medium |
| E2E-032 | New | failure | SC-004 | FR-NEW-009 | High |
| E2E-033 | New | failure | SC-004 | FR-NEW-009 | High |
| E2E-034 | New | failure | SC-004 | FR-NEW-009 | High |
| E2E-035 | New | failure | SC-004 | FR-NEW-009 | High |
| E2E-036 | New | failure | SC-004 | FR-NEW-010 | High |
| E2E-037 | New | edge | SC-004 | FR-NEW-007, FR-NEW-010 | High |
| E2E-038 | New | edge | SC-004 | FR-NEW-007, FR-NEW-009 | Medium |
| E2E-039 | New | failure | SC-004 | FR-NEW-007, FR-NEW-010 | Medium |
| E2E-040 | New | edge | SC-004 | FR-NEW-011 | Medium |
| E2E-041 | New | failure | SC-004 | FR-NEW-011 | Medium |
| E2E-042 | New | edge | SC-004 | FR-NEW-010 | Medium |
| E2E-043 | New | edge | SC-004 | FR-NEW-007, FR-NEW-008 | Medium |
| E2E-050 | New | journey | SC-005 | FR-NEW-012 | High |
| E2E-051 | New | journey | SC-005 | FR-NEW-013 | High |
| E2E-052 | New | edge | SC-005 | FR-NEW-013 | Medium |
| E2E-053 | New | failure | SC-005 | FR-NEW-014 | High |
| E2E-054 | New | edge | SC-005 | FR-NEW-012, FR-NEW-014 | Medium |
| E2E-055 | New | failure | SC-005 | FR-NEW-015 | High |
| E2E-056 | New | edge | SC-005 | FR-NEW-012 | Medium |
| E2E-057 | New | failure | SC-005 | FR-NEW-016 | Medium |
| E2E-058 | New | edge | SC-005 | FR-NEW-015 | Medium |
| E2E-059 | New | failure | SC-005 | FR-NEW-015 | Medium |
| E2E-060 | New | journey | SC-007 | FR-NEW-001, FR-NEW-004, FR-NEW-008, FR-NEW-012 | High |
| E2E-061 | New | edge | SC-007 | FR-NEW-001, FR-NEW-017 | Medium |
| E2E-062 | New | edge | SC-007 | FR-NEW-019 | Medium |
| E2E-063 | New | edge | SC-001 | FR-NEW-001, FR-NEW-017 | Medium |
| E2E-064 | New | failure | SC-007 | FR-NEW-004 | Medium |
| E2E-065 | New | journey | SC-009 | FR-NEW-022, FR-NEW-008 | Medium |
| E2E-066 | New | edge | SC-002 | FR-NEW-018, FR-NEW-002 | Medium |
| E2E-067 | New | edge | SC-002 | FR-NEW-018 | Medium |
| E2E-068 | New | edge | SC-008 | FR-NEW-021 | Medium |
| E2E-069 | New | edge | SC-008 | FR-NEW-020 | Medium |
| E2E-070 | New | journey | SC-008 | FR-NEW-019 | High |
| E2E-071 | New | failure | SC-008 | FR-NEW-024 | High |
| E2E-072 | New | edge | SC-008 | FR-NEW-020 | High |
| E2E-073 | New | edge | SC-008 | FR-NEW-021 | Medium |
| E2E-074 | New | edge | SC-008 | FR-NEW-019 | Medium |
| E2E-075 | New | failure | SC-008 | FR-NEW-022 | High |
| E2E-076 | New | failure | SC-009 | FR-NEW-022 | High |
| E2E-077 | New | edge | SC-009 | FR-NEW-008, FR-NEW-022 | Medium |
| E2E-078 | New | failure | SC-009 | FR-NEW-009 | Medium |
| E2E-079 | New | failure | SC-002 | FR-NEW-018 | High |
| E2E-080 | New | journey | SC-001 | FR-NEW-018, FR-NEW-019 | High |
| E2E-081 | New | failure | SC-001 | FR-NEW-001 | High |
| E2E-082 | New | edge | SC-004 | FR-NEW-008 | Medium |
| E2E-083 | New | journey | SC-001 | FR-NEW-019 | Medium |
| E2E-084 | New | edge | SC-004 | FR-NEW-011 | Medium |
| E2E-085 | New | failure | SC-008 | FR-NEW-024 | Medium |
| E2E-086 | New | edge | SC-008 | FR-NEW-020 | Medium |
| E2E-087 | New | edge | SC-003 | FR-NEW-004 | Medium |
| E2E-088 | New | journey | SC-008 | FR-NEW-021 | Medium |
| E2E-089 | New | failure | SC-005 | FR-NEW-014 | Medium |
| E2E-090 | New | failure | SC-005 | FR-NEW-014 | Medium |
| E2E-091 | New | failure | SC-005 | FR-NEW-014 | Medium |
| E2E-092 | New | failure | SC-009 | FR-NEW-022 | Medium |
| E2E-093 | New | failure | SC-009 | FR-NEW-022, FR-NEW-018 | Medium |
| E2E-094 | New | edge | SC-009 | FR-NEW-022 | Medium |
| E2E-095 | New | failure | SC-006 | FR-NEW-023 | High |
| E2E-096 | New | failure | SC-006 | FR-NEW-023 | Medium |
| E2E-097 | New | failure | SC-006 | FR-NEW-023 | Medium |
| E2E-098 | New | edge | SC-006 | FR-NEW-023, FR-NEW-001 | Medium |
| E2E-099 | New | edge | SC-005 | FR-NEW-013 | Medium |
| E2E-100 | New | failure | SC-005 | FR-NEW-014 | Medium |
| E2E-101 | New | edge | SC-005 | FR-NEW-013 | Medium |
| E2E-102 | New | failure | SC-008 | FR-NEW-024 | Medium |
| E2E-103 | New | edge | SC-006 | FR-NEW-016, FR-NEW-001 | Medium |
| E2E-104 | New | edge | SC-006 | FR-NEW-023 | Medium |
| E2E-105 | New | failure | SC-006 | FR-NEW-023 | Medium |
| E2E-106 | New | edge | SC-005 | FR-NEW-013, FR-NEW-015 | Medium |
| E2E-107 | New | failure | SC-003 | FR-NEW-023, FR-NEW-005 | Medium |
| E2E-MOD-001 | Modified | journey | SC-001 | FR-MOD-001, FR-MOD-002 | High |

Coverage: happy 13 (journey 13), failure 38, side effect 0, edge 45, state transition 0, security 0 as a category (the unsafe file names are the failure tests E2E-035 and E2E-078). 96 new tests and 1 modified. Happy to failure ratio 1:2.9.

### 9.2 New tests
#### E2E-001: a full version 2 answer from the program form
- **Category:** journey. **Scenario:** SC-001. **Requirements:** FR-NEW-001, FR-NEW-004, FR-NEW-008, FR-NEW-012, FR-NEW-017, FR-NEW-018
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2, two pictures (`fig/a.png` with caption `Sales` and OCR text `Total 42`, `fig/b.png` with caption `Costs` and OCR text `Net 7`), two tables (the first with CSV file `tab/t1.csv`, the second with none) and four headings; `/report.pdf` of 3 pages
- **When** Alice converts it with the program form
- **Then** the conversion succeeds and its report is `2 images, 2 tables`, then `metadata: 2 images with OCR text, 1 tables with exact CSV, 4 headings`
- **And** `fs.list_tables` shows the first table `CSV quality: exact` and the second `CSV quality: approximate`; `fs.get_table` of the first in CSV answers the exact content of `tab/t1.csv`
- **Priority:** Critical

#### E2E-002: the same answer from the HTTP form
- **Category:** journey. **Scenario:** SC-002. **Requirements:** FR-NEW-018, FR-NEW-001, FR-NEW-004, FR-NEW-008, FR-NEW-012, FR-NEW-017
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP (the manifest and the files travel in one JSON answer, files in base64); same content as E2E-001
- **When** Alice converts `/report.pdf` with the HTTP form
- **Then** the report and the tables read exactly as in E2E-001
- **And** the two forms give the same `fs.list_images` and `fs.list_tables` answers
- **Priority:** Critical

#### E2E-003: a version 1 answer reads as today
- **Category:** journey. **Scenario:** SC-006. **Requirements:** FR-NEW-001, FR-NEW-017
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest has no `version`, one picture with caption `Sales` and page 2; `/report.pdf` of 3 pages
- **When** Alice converts it
- **Then** the report is exactly `1 images, 0 tables`; the caption and page are as today
- **And** no `metadata:` line appears
- **Priority:** High

#### E2E-004: a version 1 stated explicitly
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-001, FR-NEW-017
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 1 with one picture and a caption
- **When** Alice converts it
- **Then** the report is `1 images, 0 tables`
- **And** an `ocr_text` field in that picture is ignored, no `metadata:` line
- **Priority:** Medium

#### E2E-005: no manifest at all
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-001, FR-NEW-017, FR-NEW-013
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter returns the Markdown with two `#` headings and two pipe tables and no manifest
- **When** Alice converts it
- **Then** the report is `0 images, 2 tables`
- **And** both tables read `CSV quality: approximate`; no `metadata:` line
- **Priority:** Medium

#### E2E-010: a newer version is read as version 2
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-002
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 3, a picture with `ocr_text` `Total 42` and an unknown field `layout`; the Markdown has 2 headings
- **When** Alice converts it
- **Then** the conversion succeeds; the report holds `metadata: 1 images with OCR text, 0 tables with exact CSV, 2 headings` and the line `manifest version 3 is newer than supported 2`
- **And** the unknown field changes nothing
- **Priority:** High

#### E2E-011: a version that is not a whole number
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-003
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` `two`, then `version` 0, then `version` 2.5
- **When** Alice converts it three times
- **Then** each conversion succeeds and each report holds `manifest ignored: version must be a whole number of 1 or more`
- **And** no caption, page, OCR text or CSV of that manifest is kept
- **Priority:** High

#### E2E-012: a manifest that is not JSON
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-016
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest file holds `{not json`
- **When** Alice converts it
- **Then** the conversion succeeds; the report holds `manifest ignored: not valid JSON`
- **And** the images are kept without caption or page
- **Priority:** High

#### E2E-013: a manifest that is not an object
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-016
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest holds the list `[1, 2]`
- **When** Alice converts it
- **Then** the report holds `manifest ignored: not a JSON object`
- **And** the conversion succeeded
- **Priority:** Medium

#### E2E-014: a manifest whose pictures field has the wrong type
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-016
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2 and `pictures` is the text `none`
- **When** Alice converts it
- **Then** the report holds `manifest ignored: pictures must be an object`
- **And** tables and headings fields of the same manifest are not read either
- **Priority:** Medium

#### E2E-015: a far newer version
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-002
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 99 with a picture OCR text and one headings entry
- **When** Alice converts it
- **Then** the report holds the metadata line and `manifest version 99 is newer than supported 2`
- **And** the version 2 fields are read
- **Priority:** Medium

#### E2E-016: a newer version note appears once
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-002
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a version 3 manifest with 3 pictures
- **When** Alice converts it
- **Then** the report holds the line `manifest version 3 is newer than supported 2` exactly once
- **And** the report lines about pictures are unchanged
- **Priority:** Medium

#### E2E-017: a version given as text or as a boolean
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-003
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` "2" (text), then `true`, then `null`, then -1, then 2.0, then 1e0
- **When** Alice converts it six times
- **Then** each report holds `manifest ignored: version must be a whole number of 1 or more`
- **And** no field of the manifest is kept
- **Priority:** High

#### E2E-018: a version of exactly 1 with version 2 fields
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-001, FR-NEW-003
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `version` 1 with `tables` and `headings` fields
- **When** Alice converts it
- **Then** the new fields are ignored and the report has no metadata line
- **And** the report is exactly as for E2E-004
- **Priority:** Medium

#### E2E-020: OCR text and caption together
- **Category:** journey. **Scenario:** SC-003. **Requirements:** FR-NEW-004
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); one picture with caption `Sales`, page 2 and `ocr_text` `Total 42`, `version` 2; `/report.pdf`; the Markdown has 1 heading
- **When** Alice converts it
- **Then** the report holds `metadata: 1 images with OCR text, 0 tables with exact CSV, 1 headings`
- **And** `fs.list_images` still shows the caption `Sales` and page 2
- **Priority:** High

#### E2E-021: a picture without OCR text
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-004
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); two pictures, only the first with `ocr_text`, `version` 2
- **When** Alice converts it
- **Then** the report holds `metadata: 1 images with OCR text`
- **And** the second image is kept as it is today
- **Priority:** Medium

#### E2E-022: OCR text of an empty string
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-004
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with `ocr_text` empty, `version` 2
- **When** Alice converts it
- **Then** the count of images with OCR text is 0
- **And** no failure line
- **Priority:** Medium

#### E2E-023: the converter says the OCR failed
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-005
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); two pictures, the second with `ocr_failed` true and no `ocr_text`, `version` 2
- **When** Alice converts it
- **Then** the report holds the line `image-2: OCR unavailable`
- **And** the conversion succeeds and the first image is intact
- **Priority:** High

#### E2E-024: OCR failure and caption failure on one picture
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-005
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); picture 1 with `caption_failed` true and `ocr_failed` true, `version` 2
- **When** Alice converts it
- **Then** the report lists `image-1: caption unavailable` then `image-1: OCR unavailable`
- **And** both lines appear once
- **Priority:** Medium

#### E2E-025: OCR text and OCR failure together
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-005, FR-NEW-004
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with `ocr_text` `Total 42` and `ocr_failed` true, `version` 2
- **When** Alice converts it
- **Then** the text is kept, counted in `images with OCR text`, and the line `image-1: OCR unavailable` is added
- **And** the failure flag does not discard the text
- **Priority:** Medium

#### E2E-026: OCR text too long
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with an `ocr_text` of 100 001 characters, `version` 2
- **When** Alice converts it
- **Then** the report holds `image-1: OCR text too long`; the count of images with OCR text is 0
- **And** a text of exactly 100 000 characters is kept (counted)
- **Priority:** High

#### E2E-027: OCR text with accents and emoji
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-004
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with `ocr_text` `Résumé 42 ✓ 日本語`, `version` 2
- **When** Alice converts it
- **Then** the count of images with OCR text is 1
- **And** the conversion report has no failure line
- **Priority:** Medium

#### E2E-028: exactly 100 000 characters of OCR text
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); an `ocr_text` of exactly 100 000 characters
- **When** Alice converts it
- **Then** the count of images with OCR text is 1 and no failure line
- **And** a count of characters, not bytes: 100 000 accented characters are kept
- **Priority:** Medium

#### E2E-029: two pictures with too long OCR text
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); pictures 1 and 3 have an `ocr_text` of 100 001 characters, picture 2 a valid one
- **When** Alice converts it
- **Then** the report holds `image-1: OCR text too long` then `image-3: OCR text too long`; the count of images with OCR text is 1
- **And** picture 2 is intact
- **Priority:** High

#### E2E-030: exact CSV for a table
- **Category:** journey. **Scenario:** SC-004. **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has one pipe table, the manifest lists it with `csv` `tab/t1.csv` whose content is `a,b\n1,"x,y"\n`; `version` 2
- **When** Alice converts it and asks `fs.get_table` of `table-1` in CSV
- **Then** the answer is exactly `a,b\n1,"x,y"\n`
- **And** `fs.list_tables` shows `CSV quality: exact`
- **Priority:** Critical

#### E2E-031: a table entry with a caption
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-011
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has a pipe table with a caption line `Table 1: Sales` before it; the manifest entry says `caption` `Sales per region`; `version` 2
- **When** Alice converts it
- **Then** `fs.list_tables` shows the caption `Sales per region`
- **And** with an empty manifest caption it shows `Table 1: Sales`
- **Priority:** Medium

#### E2E-032: the CSV file is missing from the bundle
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-009
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the table entry says `csv` `tab/missing.csv`, no such file; `version` 2
- **When** Alice converts it
- **Then** the report holds `table-1: CSV unavailable`
- **And** the table is kept and reads `CSV quality: approximate`
- **Priority:** High

#### E2E-033: the CSV file is not text
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-009
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the CSV file holds the bytes `FF FE 00`
- **When** Alice converts it
- **Then** the report holds `table-1: CSV unavailable`
- **And** the table reads `CSV quality: approximate`
- **Priority:** High

#### E2E-034: the CSV file is too large
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-009
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the CSV file holds 5 000 001 characters
- **When** Alice converts it
- **Then** the report holds `table-1: CSV unavailable`
- **And** a file of exactly 5 000 000 characters is kept as exact CSV
- **Priority:** High

#### E2E-035: a CSV reference that escapes the bundle
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-009
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the table entries say `csv` `../../etc/passwd`, `/etc/passwd` and `http://example.com/t.csv`
- **When** Alice converts it
- **Then** each table reports `table-N: CSV unavailable` and reads `CSV quality: approximate`
- **And** no file outside the bundle was read (the content of `/etc/passwd` appears nowhere)
- **Priority:** High

#### E2E-036: more entries than tables
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-010
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has 1 pipe table, the manifest lists 3 tables
- **When** Alice converts it
- **Then** the report holds `manifest lists 3 tables, the document has 1`
- **And** the first entry is applied to the table, the two others ignored
- **Priority:** High

#### E2E-037: fewer entries than tables
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-007, FR-NEW-010
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has 3 pipe tables, the manifest lists 1 with a CSV
- **When** Alice converts it
- **Then** table 1 reads `exact`, tables 2 and 3 read `approximate`
- **And** no mismatch line is added
- **Priority:** High

#### E2E-038: a manifest table entry for a malformed table
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-007, FR-NEW-009
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has a pipe table whose separator has a different number of cells than its header; the manifest gives it a CSV
- **When** Alice converts it
- **Then** the report keeps today's line `table-1: malformed table`
- **And** the CSV is not used, the table is not kept as today
- **Priority:** Medium

#### E2E-039: a table entry that is not an object
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-007, FR-NEW-010
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest `tables` list holds `1` as its first entry and an entry with `csv` for the second table
- **When** Alice converts it
- **Then** table 1 stays approximate, table 2 reads exact
- **And** the report holds `tables entry 1: not an object`
- **Priority:** Medium

#### E2E-040: the table page is kept
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-011
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); table entries with `page` 2; `version` 2
- **When** Alice converts it
- **Then** the conversion succeeds, the report is `0 images, 2 tables` then the metadata line and no `tables entry` line
- **And** with a `.docx` source (`/doc.docx` converted the same way) the report is the same
- **Priority:** Medium

#### E2E-041: a table page that is invalid
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-011
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the first table entry has `page` 0, the second `page` `two`
- **When** Alice converts it
- **Then** the report holds `tables entry 1: invalid page` and `tables entry 2: invalid page`
- **And** both tables keep their other data (CSV exact if given)
- **Priority:** Medium

#### E2E-042: exactly as many entries as tables
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-010
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the Markdown has 2 pipe tables and the manifest lists 2
- **When** Alice converts it
- **Then** no mismatch line is added
- **And** each entry applies to its table in order
- **Priority:** Medium

#### E2E-043: a table with no CSV and a table with CSV in reverse order
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); table 1 has no `csv`, table 2 has `tab/t2.csv`
- **When** Alice converts it
- **Then** `fs.list_tables` shows table 1 `approximate` and table 2 `exact`
- **And** `fs.get_table` of table 2 in CSV answers the exact file
- **Priority:** Medium

#### E2E-050: headings supplied by the converter
- **Category:** journey. **Scenario:** SC-005. **Requirements:** FR-NEW-012
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists headings `{level 1, Sales}`, `{level 2, Q1, page 2}`, `{level 2, Q2, page 3}`
- **When** Alice converts it
- **Then** the report holds `metadata: 0 images with OCR text, 0 tables with exact CSV, 3 headings`
- **And** the headings are those of the manifest, not those of the Markdown: the Markdown has 2 headings and the count in the line is 3
- **Priority:** High

#### E2E-051: headings derived from the Markdown
- **Category:** journey. **Scenario:** SC-005. **Requirements:** FR-NEW-013
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2 with no `headings`; the Markdown has `# Sales`, `## Q1`, a fenced code block holding the line `# not a heading`, and `## Q2`
- **When** Alice converts it
- **Then** the report holds `3 headings`
- **And** the line inside the code block is not counted
- **Priority:** High

#### E2E-052: derived headings without any manifest
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-013
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no manifest; the Markdown has 2 headings
- **When** Alice converts it
- **Then** the report is unchanged (`0 images, 0 tables`) and no `metadata:` line is added
- **And** the conversion succeeds with no failure line
- **Priority:** Medium

#### E2E-053: an invalid heading entry
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-014
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the headings list holds `{level 0, X}`, `{level 7, Y}`, `{level 2, empty title}`, `{level 2, Z}`
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid`, `heading 2: invalid`, `heading 3: invalid` and `1 headings`
- **And** the entry `Z` is kept
- **Priority:** High

#### E2E-054: limits of the heading level
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-012, FR-NEW-014
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); headings of level 1 and level 6
- **When** Alice converts it
- **Then** both are counted
- **And** a heading of level 6 with a title of 500 characters is counted too
- **Priority:** Medium

#### E2E-055: too many headings
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-015
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists 10 001 headings
- **When** Alice converts it
- **Then** the report holds `10000 headings` in the metadata line and the line `headings over 10000 dropped: 1`
- **And** a list of exactly 10 000 produces no such line
- **Priority:** High

#### E2E-056: a heading title with markup and accents
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-012
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a heading `{level 1, "Résumé **final** <b>"}`
- **When** Alice converts it
- **Then** the heading is counted
- **And** the conversion report has no failure line
- **Priority:** Medium

#### E2E-057: headings of the wrong type
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-016
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2 and `headings` is the text `none`
- **When** Alice converts it
- **Then** the report holds `manifest ignored: headings must be a list`
- **And** the other fields of the manifest are not read
- **Priority:** Medium

#### E2E-058: exactly 10 000 headings
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-015
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists exactly 10 000 headings
- **When** Alice converts it
- **Then** the metadata line says `10000 headings` and no dropped line is added
- **And** the conversion succeeds
- **Priority:** Medium

#### E2E-059: 20 000 headings
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-015
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists 20 000 headings
- **When** Alice converts it
- **Then** the report holds `headings over 10000 dropped: 10000`
- **And** 10 000 headings are kept
- **Priority:** Medium

#### E2E-060: a converter that sends only captions
- **Category:** journey. **Scenario:** SC-007. **Requirements:** FR-NEW-001, FR-NEW-004, FR-NEW-008, FR-NEW-012
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/report.pdf`; a version 2 manifest with captions only; the Markdown has 2 headings
- **When** Alice converts it
- **Then** the report holds `metadata: 0 images with OCR text, 0 tables with exact CSV, 2 headings` (the headings of the Markdown)
- **And** captions and pages are kept as today
- **Priority:** High

#### E2E-061: a version 2 manifest with nothing in it
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-001, FR-NEW-017
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest is `{"version": 2}` and the Markdown has 1 heading
- **When** Alice converts it
- **Then** the report holds `metadata: 0 images with OCR text, 0 tables with exact CSV, 1 headings`
- **And** no failure line
- **Priority:** Medium

#### E2E-062: the same manifest twice
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-019
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the same document converted twice with the same manifest
- **When** Alice converts it twice
- **Then** both reports are equal
- **And** the lists are the same, with no duplicate image or table
- **Priority:** Medium

#### E2E-063: a version 2 conversion without pictures
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-001, FR-NEW-017
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `version` 2 and the Markdown has one pipe table and no picture
- **When** Alice converts it
- **Then** the report is `0 images, 1 tables` then `metadata: 0 images with OCR text, 0 tables with exact CSV, 0 headings`
- **And** no failure line
- **Priority:** Medium

#### E2E-064: pictures whose targets do not match the manifest
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-004
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest describes `fig/x.png` with OCR text, the Markdown refers to `fig/a.png`
- **When** Alice converts it
- **Then** the picture `fig/a.png` is kept without caption or OCR text and the count of images with OCR text is 0
- **And** no line about `fig/x.png` is added
- **Priority:** Medium

#### E2E-065: a bundle exactly at the converter output limit
- **Category:** journey. **Scenario:** SC-009. **Requirements:** FR-NEW-022, FR-NEW-008
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter output limit is 1 000 000 bytes; the Markdown, pictures, CSV files and the manifest add up to exactly 1 000 000 bytes
- **When** Alice converts it
- **Then** the conversion succeeds
- **And** the tables read exact
- **Priority:** Medium

#### E2E-066: HTTP form with unknown top level keys
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-018, FR-NEW-002
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP; the answer holds the keys `markdown`, `files`, `manifest` and `extra`
- **When** Alice converts it
- **Then** the conversion succeeds and `extra` is ignored
- **And** the report is as for the same answer without `extra`
- **Priority:** Medium

#### E2E-067: folder form with the manifest next to other files
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-018
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the output folder holds `document.md`, `artifacts.json`, `fig/a.png`, `tab/t1.csv` and `notes.txt` (not referenced)
- **When** Alice converts it
- **Then** the report counts only referenced pictures and tables
- **And** `notes.txt` is not kept anywhere
- **Priority:** Medium

#### E2E-068: an extraction option does not change a converted document
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-021
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/notes.docx` converted with a converter, then the built in extraction run on it with the refresh option
- **When** Alice runs the built in extraction
- **Then** the built in extraction replaces the set, tables read exact, no metadata line
- **And** no OCR text remains
- **Priority:** Medium

#### E2E-069: a document converted before is read after a restart
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-020
- **Given** a document converted by the previous version
- **When** the operator restarts the platform and Alice lists its tables
- **Then** the lists are unchanged
- **And** the converter stub received no call
- **Priority:** Medium

#### E2E-070: a re-conversion replaces the whole set
- **Category:** journey. **Scenario:** SC-008. **Requirements:** FR-NEW-019
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/report.pdf` converted with OCR text on 2 pictures and an exact CSV; then the converter is changed to send version 1 answers
- **When** Alice converts the document again
- **Then** the report is `2 images, 1 tables` with no `metadata:` line; the table reads `CSV quality: approximate`
- **And** `fs.get_table` of that table in CSV answers the CSV derived from the Markdown, not the first conversion's exact file
- **Priority:** High

#### E2E-071: a failed re-conversion keeps the previous set
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-024
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/report.pdf` converted with a full version 2 answer; the converter now fails
- **When** Alice converts it again
- **Then** the conversion fails with the existing converter error text
- **And** the images, tables and exact CSV of the first conversion are intact
- **Priority:** High

#### E2E-072: documents converted before keep their data
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-020
- **Given** a document converted by the previous version, with captions and approximate tables
- **When** the platform is upgraded and Alice lists its images and tables
- **Then** the lists are unchanged and nothing is converted again
- **And** a new conversion produces version 2 data
- **Priority:** High

#### E2E-073: the built in extraction is unchanged
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-021
- **Given** project `proj`, Alice a member; no converter service is configured; `/notes.docx`
- **When** Alice extracts its text
- **Then** tables read `CSV quality: exact` as today and the report has no `metadata:` line
- **And** the report is exactly as before this increment
- **Priority:** Medium

#### E2E-074: a file edited after conversion
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-019
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/report.pdf` converted with full data; the file is then replaced
- **When** Alice lists its images
- **Then** the answer is `No artifacts: document not extracted` as today
- **And** listing its tables gives the same answer
- **Priority:** Medium

#### E2E-075: storage quota counts the new metadata
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-022
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the project has 1 000 bytes of storage left; the answer carries OCR text and CSV files of 4 000 bytes in total
- **When** Alice converts the document
- **Then** the conversion is refused with the existing quota error `ERR_PROJECT_QUOTA_EXCEEDED`
- **And** no image, table, OCR text or CSV is kept
- **Priority:** High

#### E2E-076: the bundle cap counts the CSV files
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-022
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter output limit is 1 000 000 bytes; the Markdown, the picture files, the CSV files and the manifest add up to 1 000 001 bytes
- **When** Alice converts it
- **Then** the conversion fails with the existing text `document service output is over the doc_service.max_input_bytes limit of 1000000`
- **And** nothing is kept
- **Priority:** High

#### E2E-077: table files are not left in the volume
- **Category:** edge. **Scenario:** SC-009. **Requirements:** FR-NEW-008, FR-NEW-022
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the bundle holds a CSV file `tab/t1.csv`
- **When** Alice converts it and lists the folder of the document
- **Then** the folder holds the document and its Markdown companion only, no CSV file
- **And** the CSV is available through `fs.get_table`
- **Priority:** Medium

#### E2E-078: a bundle file with an unsafe name
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-009
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the HTTP form returns a file named `../tab.csv` referenced by a table entry
- **When** Alice converts it
- **Then** the table reads approximate with `table-1: CSV unavailable`
- **And** the conversion succeeds
- **Priority:** Medium

#### E2E-079: the HTTP form with a base64 error
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-018
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP; a CSV file with invalid base64
- **When** Alice converts it
- **Then** the conversion fails with the existing text `document service bundle file 'tab/t1.csv' is not base64:` followed by the cause
- **And** nothing is kept
- **Priority:** High

#### E2E-080: conversion by upload carries the same data
- **Category:** journey. **Scenario:** SC-001. **Requirements:** FR-NEW-018, FR-NEW-019
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); Alice uploads `/report.pdf` with the conversion option set
- **When** the upload completes
- **Then** the answer carries the report of E2E-001
- **And** the images and tables are those of E2E-001
- **Priority:** High

#### E2E-081: a non member cannot convert
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); Dave (`dave@example.com`) is not a member
- **When** Dave converts `/report.pdf`
- **Then** the answer is `ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'`
- **And** nothing is stored
- **Priority:** High

#### E2E-082: a table CSV for a page range of a spreadsheet
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-008
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/sheet.xlsx` converted with 3 tables each with its CSV
- **When** Alice lists the tables
- **Then** the three read exact in document order
- **And** the report has no `tables entry` line
- **Priority:** Medium

#### E2E-083: a second member sees the same data
- **Category:** journey. **Scenario:** SC-001. **Requirements:** FR-NEW-019
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); Carol (`carol@example.com`) is a member too; `/report.pdf` converted by Alice with a full answer
- **When** Carol lists the tables
- **Then** she sees the exact CSV tables
- **And** the converter stub received one call in total
- **Priority:** Medium

#### E2E-084: the caption of a table from the Markdown wins when the manifest has none
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-011
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a table caption line `Table 2: Costs` in the Markdown and an entry without `caption`
- **When** Alice converts it
- **Then** the caption is `Table 2: Costs`
- **And** with the manifest caption `Costs` it is `Costs`
- **Priority:** Medium

#### E2E-085: a failed conversion changes nothing
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-024
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter times out on `/report.pdf`
- **When** Alice converts it
- **Then** the conversion fails with the existing timeout text
- **And** listing its images and tables answers `No artifacts: document not extracted` (the document was never converted before)
- **Priority:** Medium

#### E2E-086: an old document and a new one in the same folder
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-020
- **Given** `/old.pdf` converted before the upgrade and `/new.pdf` converted after, in the same project
- **When** Alice lists the tables of both
- **Then** `/old.pdf` tables read approximate, `/new.pdf` tables read exact
- **And** the two sets are independent
- **Priority:** Medium

#### E2E-087: OCR text is not searched or chunked as a document line
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-004
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a picture with `ocr_text` `UniqueOcrWord`
- **When** Alice converts it and searches `UniqueOcrWord`
- **Then** search finds nothing
- **And** the document view is unchanged (BL-0167 adds OCR to the view)
- **Priority:** Medium

#### E2E-088: the built in extraction of a spreadsheet
- **Category:** journey. **Scenario:** SC-008. **Requirements:** FR-NEW-021
- **Given** no converter service configured; `/budget.xlsx` with 2 sheets of one table each
- **When** Alice extracts its text
- **Then** the report is `0 images, 2 tables`, both `CSV quality: exact`, as before this increment
- **And** no `metadata:` line
- **Priority:** Medium

#### E2E-089: a heading entry that is not an object
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-014
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the headings list holds the text `Intro`, then `{level 1, Real}`
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid` and `1 headings`
- **And** the entry `Real` is kept
- **Priority:** Medium

#### E2E-090: a heading level that is not a whole number
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-014
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); headings with `level` 1.5, `level` `1` (text) and `level` 3
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid` and `heading 2: invalid` and `1 headings`
- **And** the entry of level 3 is kept
- **Priority:** Medium

#### E2E-091: a heading title of 1 001 characters
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-014
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); a heading with a title of 1 001 characters and another of exactly 1 000
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid` and `1 headings`
- **And** the title of 1 000 characters is kept
- **Priority:** Medium

#### E2E-092: the manifest bytes count in the output limit, program form
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-022
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the converter output limit is 1 000 000 bytes; the Markdown, the files and the manifest (mostly OCR text) add up to 1 000 000 bytes, then to 1 000 001
- **When** Alice converts each
- **Then** the first conversion succeeds; the second fails with `document service output is over the doc_service.max_input_bytes limit of 1000000`
- **And** nothing is kept for the second
- **Priority:** Medium

#### E2E-093: the same total in the HTTP form
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-022, FR-NEW-018
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP; the same content as E2E-092 (limit 1 000 000 bytes; the Markdown, the files and the manifest text add up to 1 000 000 and then 1 000 001 bytes)
- **When** Alice converts each
- **Then** the first succeeds, the second fails with the same text as the program form
- **And** both forms give the same totals
- **Priority:** Medium

#### E2E-094: a large answer is stored in time
- **Category:** edge. **Scenario:** SC-009. **Requirements:** FR-NEW-022
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the answer holds 1 000 pictures, 500 tables and 10 000 headings
- **When** Alice converts it
- **Then** the conversion completes within 10 seconds more than the converter itself took
- **And** the report counts 1 000 images, 500 tables and 10 000 headings
- **Priority:** Medium

#### E2E-095: a picture field of the wrong type
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-023
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); picture 1 has `ocr_text` 42 and picture 2 has `ocr_failed` the text `yes`, `version` 2
- **When** Alice converts it
- **Then** the report holds `image-1: invalid ocr_text` and `image-2: invalid ocr_failed`; the count of images with OCR text is 0
- **And** the captions and pages are kept
- **Priority:** High

#### E2E-096: a table field of the wrong type
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-023
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); table entries with `csv` 7 and `caption` 5, `version` 2
- **When** Alice converts it
- **Then** the report holds `tables entry 1: invalid csv` and `tables entry 2: invalid caption`
- **And** both tables read approximate
- **Priority:** Medium

#### E2E-097: a picture entry that is not an object, and a bad picture page
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-023
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `pictures` holds the text `x` for the first picture and a page 0 for the second, `version` 2
- **When** Alice converts it
- **Then** the report holds `image-1: not an object` and `image-2: invalid page`
- **And** both images are kept
- **Priority:** Medium

#### E2E-098: a wrong type in a version 1 manifest
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-023, FR-NEW-001
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no `version`; picture 1 has `caption` 42 and a valid page 2; the manifest also has `ocr_text` 7
- **When** Alice converts it
- **Then** the report holds `image-1: invalid caption` and no `metadata:` line
- **And** the page 2 is kept and the manifest is not discarded
- **Priority:** Medium

#### E2E-099: an empty headings list
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-013
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest says `headings` is the empty list; the Markdown has 3 headings
- **When** Alice converts it
- **Then** the report holds `metadata: 0 images with OCR text, 0 tables with exact CSV, 0 headings`
- **And** no Markdown heading is derived
- **Priority:** Medium

#### E2E-100: 150 invalid heading entries
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-014
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); the manifest lists 150 headings with level 0
- **When** Alice converts it
- **Then** the report holds `heading 1: invalid` to `heading 100: invalid`, then `heading lines over 100 dropped: 50`
- **And** the metadata line says `0 headings`
- **Priority:** Medium

#### E2E-101: a derived heading with a closing run
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-013
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no `headings` in the manifest; the Markdown has `## Q1 ##`, `# C#` and a heading whose title has 1 001 characters
- **When** Alice converts it
- **Then** a conversion with a `version` 2 manifest holding no `headings`: the metadata line counts 2 headings (the long one is dropped without a line)
- **And** the derived titles are `Q1` and `C#`
- **Priority:** Medium

#### E2E-102: the HTTP converter fails
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-024
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest) over HTTP; the service answers an error after a first successful conversion
- **When** Alice converts it again
- **Then** the conversion fails with the existing error text
- **And** the previous images, tables and exact CSV are intact
- **Priority:** Medium

#### E2E-103: headings that is not a list in a version 1 manifest
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-016, FR-NEW-001
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no `version`; `headings` is the text `none`, one picture with a caption
- **When** Alice converts it
- **Then** the report is exactly `1 images, 0 tables`
- **And** the caption is kept
- **Priority:** Medium

#### E2E-104: null fields are absent
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-023
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); picture 1 has `ocr_text` null, `caption` null and `page` null; table entry 1 has `csv` null; headings entry with `page` null
- **When** Alice converts it
- **Then** the report has no `invalid` line and the count of images with OCR text is 0
- **And** the headings entry with a null page is counted
- **Priority:** Medium

#### E2E-105: an invalid picture page on a source that is not paged
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-023
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); `/doc.docx`; picture 1 has `page` 0
- **When** Alice converts it
- **Then** the report holds `image-1: invalid page`
- **And** a valid page 2 on the same source gives no line and no page is kept
- **Priority:** Medium

#### E2E-106: more than 10 000 derived headings
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-013, FR-NEW-015
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); no `headings` in the manifest; the Markdown has 10 002 headings
- **When** Alice converts it
- **Then** the metadata line counts 10000 headings and the report holds `headings over 10000 dropped: 2`
- **And** with no manifest at all the same document gives no such line
- **Priority:** Medium

#### E2E-107: a picture that is not kept
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-023, FR-NEW-005
- **Given** project `proj`, Alice (`alice@example.com`) a member, a converter that returns a bundle (the Markdown, picture files, table files and a manifest); picture 1 refers to a file missing from the bundle and has `ocr_failed` true, `ocr_text` 42 and `caption` 5
- **When** Alice converts it
- **Then** the report holds `image-1: picture unavailable` and no OCR or `invalid` line for that picture
- **And** the count of images with OCR text is 0
- **Priority:** Medium

### 9.4 Modified tests
#### E2E-MOD-001: converting with a manifest (was SPEC-0019/E2E-001)
- **Category:** journey. **Scenario:** SC-001. **Requirements:** FR-MOD-001, FR-MOD-002
- **Validated before:** `report.pdf` converted with a manifest of captions and pages: the result reports `2 images, 2 tables` and no missing item; listing images shows `image-1` `Figure 1: Revenue 2024` page 1 and `image-2` `Schéma réseau 🌐` page 3.
- **Validates now:** the same, and when the manifest says `version` 2 the report holds a second line `metadata: 0 images with OCR text, 0 tables with exact CSV, 1 headings` (the Markdown of that fixture has 1 heading); a manifest without `version` gives the exact report as before.
- **Given** the SPEC-0019 data of that test, once with no `version` and once with `version` 2
- **When** Alice converts it
- **Then** the first report is `2 images, 2 tables`; the second is `2 images, 2 tables` then the metadata line
- **And** the captions and pages listed are identical in both
- **Priority:** High

### 9.5 Removed tests
None.

## 10. Traceability
| Scenario | Requirements | Happy | Failure | Edge |
|---|---|---|---|---|
| SC-001 | FR-NEW-001, FR-NEW-004, FR-NEW-008, FR-NEW-012, FR-NEW-017, FR-NEW-018, FR-NEW-019 | E2E-001, E2E-080, E2E-083 | E2E-081 | E2E-063 |
| SC-002 | FR-NEW-001, FR-NEW-002, FR-NEW-004, FR-NEW-008, FR-NEW-012, FR-NEW-017, FR-NEW-018 | E2E-002 | E2E-079 | E2E-066, E2E-067 |
| SC-003 | FR-NEW-004, FR-NEW-005, FR-NEW-006, FR-NEW-023 | E2E-020 | E2E-023, E2E-026, E2E-029, E2E-107 | E2E-021, E2E-022, E2E-024, E2E-025, E2E-027, E2E-028, E2E-087 |
| SC-004 | FR-NEW-007, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-011 | E2E-030 | E2E-032, E2E-033, E2E-034, E2E-035, E2E-036, E2E-039, E2E-041 | E2E-031, E2E-037, E2E-038, E2E-040, E2E-042, E2E-043, E2E-082, E2E-084 |
| SC-005 | FR-NEW-012, FR-NEW-013, FR-NEW-014, FR-NEW-015, FR-NEW-016 | E2E-050, E2E-051 | E2E-053, E2E-055, E2E-057, E2E-059, E2E-089, E2E-090, E2E-091, E2E-100 | E2E-052, E2E-054, E2E-056, E2E-058, E2E-099, E2E-101, E2E-106 |
| SC-006 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-016, FR-NEW-017, FR-NEW-023 | E2E-003 | E2E-011, E2E-012, E2E-013, E2E-017, E2E-095, E2E-096, E2E-097, E2E-105 | E2E-004, E2E-010, E2E-014, E2E-015, E2E-016, E2E-018, E2E-098, E2E-103, E2E-104 |
| SC-007 | FR-NEW-001, FR-NEW-004, FR-NEW-008, FR-NEW-012, FR-NEW-013, FR-NEW-017, FR-NEW-019 | E2E-060 | E2E-064 | E2E-005, E2E-061, E2E-062 |
| SC-008 | FR-NEW-019, FR-NEW-020, FR-NEW-021, FR-NEW-022, FR-NEW-024 | E2E-070, E2E-088 | E2E-071, E2E-075, E2E-085, E2E-102 | E2E-068, E2E-069, E2E-072, E2E-073, E2E-074, E2E-086 |
| SC-009 | FR-NEW-008, FR-NEW-009, FR-NEW-018, FR-NEW-022 | E2E-065 | E2E-076, E2E-078, E2E-092, E2E-093 | E2E-077, E2E-094 |

Tests per requirement (at least 3 each):
- FR-NEW-001: E2E-001, E2E-002, E2E-003, E2E-004, E2E-005, E2E-018, E2E-060, E2E-061, E2E-063, E2E-081, E2E-098, E2E-103 (12 tests)
- FR-NEW-002: E2E-010, E2E-015, E2E-016, E2E-066 (4 tests)
- FR-NEW-003: E2E-011, E2E-017, E2E-018 (3 tests)
- FR-NEW-004: E2E-001, E2E-002, E2E-020, E2E-021, E2E-022, E2E-025, E2E-027, E2E-060, E2E-064, E2E-087 (10 tests)
- FR-NEW-005: E2E-023, E2E-024, E2E-025, E2E-107 (4 tests)
- FR-NEW-006: E2E-026, E2E-028, E2E-029 (3 tests)
- FR-NEW-007: E2E-030, E2E-037, E2E-038, E2E-039, E2E-043 (5 tests)
- FR-NEW-008: E2E-001, E2E-002, E2E-030, E2E-043, E2E-060, E2E-065, E2E-077, E2E-082 (8 tests)
- FR-NEW-009: E2E-032, E2E-033, E2E-034, E2E-035, E2E-038, E2E-078 (6 tests)
- FR-NEW-010: E2E-036, E2E-037, E2E-039, E2E-042 (4 tests)
- FR-NEW-011: E2E-031, E2E-040, E2E-041, E2E-084 (4 tests)
- FR-NEW-012: E2E-001, E2E-002, E2E-050, E2E-054, E2E-056, E2E-060 (6 tests)
- FR-NEW-013: E2E-005, E2E-051, E2E-052, E2E-099, E2E-101, E2E-106 (6 tests)
- FR-NEW-014: E2E-053, E2E-054, E2E-089, E2E-090, E2E-091, E2E-100 (6 tests)
- FR-NEW-015: E2E-055, E2E-058, E2E-059, E2E-106 (4 tests)
- FR-NEW-016: E2E-012, E2E-013, E2E-014, E2E-057, E2E-103 (5 tests)
- FR-NEW-017: E2E-001, E2E-002, E2E-003, E2E-004, E2E-005, E2E-061, E2E-063 (7 tests)
- FR-NEW-018: E2E-001, E2E-002, E2E-066, E2E-067, E2E-079, E2E-080, E2E-093 (7 tests)
- FR-NEW-019: E2E-062, E2E-070, E2E-074, E2E-080, E2E-083 (5 tests)
- FR-NEW-020: E2E-069, E2E-072, E2E-086 (3 tests)
- FR-NEW-021: E2E-068, E2E-073, E2E-088 (3 tests)
- FR-NEW-022: E2E-065, E2E-075, E2E-076, E2E-077, E2E-092, E2E-093, E2E-094 (7 tests)
- FR-NEW-023: E2E-095, E2E-096, E2E-097, E2E-098, E2E-104, E2E-105, E2E-107 (7 tests)
- FR-NEW-024: E2E-071, E2E-085, E2E-102 (3 tests)

## 11. Open questions
None that change observable behavior.

## 12. Glossary (MANDATORY)
| Term | Definition | Context |
|---|---|---|
| Converter | a program or an HTTP service that turns a document into Markdown, files and a manifest | Documents |
| Program form, HTTP form | the converter answers in an output folder, or in one JSON answer with base64 files | Documents |
| Bundle | the Markdown, the files and the manifest of one answer | Documents |
| Manifest | the description of the answer: pictures, tables, headings and a version | Documents |
| Version | the whole number saying which fields of the manifest are read | Documents |
| OCR text | the text read inside a picture | Documents |
| Exact CSV | a table's CSV given by the converter, kept character for character | Documents |
| Structure | the list of the headings of a document, each with a level, a title and optionally a page | Documents |
| Conversion report | the lines returned after a conversion: counts, the metadata line and one line per dropped item | Documents |
| Set | the images, tables, OCR text, CSV and structure of a document, replaced as a whole | Documents |
| Quota | the storage limit of a project | Projects |

## 13. Implementability gate (MANDATORY)
| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 13 | 4 | NOT-IMPLEMENTABLE (tests without version 2, malformed manifest on version 1, wrong typed fields, supplied headings, output limit units, report order, whole number, heading rules, unobservable Thens, EARS shapes) |
| 2 | 4 | 0 | NOT-IMPLEMENTABLE (what the output limit counts, null values, invalid picture page on any source, derived headings above 10 000, an unobservable And) |
| 3 | 1 | 0 | IMPLEMENTABLE (one low gap about a picture that is not kept, amended) |

**Amendments applied:** every finding of rounds 1 to 3 is merged; FR-NEW-023 and FR-NEW-024, FR-MOD-002 and tests E2E-092 to E2E-107 added; the checks of Phase 10 were run on the final file
**No implementation detail outside Section 14:** PASS

## 14. Evidence appendix (for /sdlc-design and the auditor, not for the product owner)
### 14.1 Current State evidence
| # | Claim (Section 2) | Evidence |
|---|---|---|
| 1 | the manifest holds only `pictures` with `caption`, `caption_failed`, `page`; missing fields read empty | `crates/core/src/docs/service.rs:89-102` |
| 2 | images are kept as files (content addressed) with caption and page; tables as cells with Markdown and derived CSV, quality `approximate` for converter tables | `crates/core/src/docs/artifacts/capture.rs:95-175`, `crates/core/src/docs/artifacts/mod.rs:29-48`, `crates/core/src/storage/meta.rs:114-140` |
| 3 | a picture target resolves only among the bundle files; URLs, absolute paths and `..` name nothing | `crates/core/src/docs/artifacts/capture.rs:262-298` |
| 4 | pipe table blocks and headings are found by parsing the Markdown; no structure stored | `crates/core/src/docs/artifacts/parse.rs:46-72`, `crates/core/src/chunking/blocks.rs:61,184` |
| 5 | the report is `<N> images, <M> tables` then one line per failure | `crates/core/src/docs/artifacts/capture.rs:146-151,105,125` |
| 6 | a malformed manifest is ignored with a log line only | `crates/core/src/docs/service.rs:746-754,786-790` |
| 7 | the converter output is read as a bundle, capped by the converter output limit, symlinks skipped; base64 failure text | `crates/core/src/docs/service.rs:690-741,770-773`, `:292` |
| 8 | the real converter writes no manifest yet | `README.md:259`, `.agent_docs/architecture.md:169` |
| 9 | converter and artifact tests | `crates/core/src/docs/service.rs` (23 tests), `docs/artifacts/capture.rs` (8), `docs/artifacts/parse.rs` (9), `tools/artifacts.rs` (180 in `mod e2e`), `tools/converter_bound_e2e.rs` (23) |
| 10 | the conversion paths call the converter and commit the set | `crates/core/src/core/fs_ops.rs:696,799-823`, `crates/core/src/tools/document.rs:99,118`, `crates/core/src/api/dataplane.rs:665,1584` |
| 11 | table labels, list and get tools | `crates/core/src/tools/artifacts.rs:238-306`, `crates/core/src/docs/artifacts/mod.rs:40-45` |
Test command: `make test` (`./test.sh`); artifacts only: `cargo test -p fsa-core --lib -- docs:: tools::artifacts tools::converter_bound`.

### 14.2 Class sweep
**Class:** names and references written by an untrusted converter reaching reads of files or stored content outside the converter's own output (CWE-22).
**Searches run:**
- `rg -n '\.files\b' crates/core/src/docs crates/core/src/core/fs_ops.rs` (idiom: access to the bundle's file map): the only read of the map is `capture.rs:258`, behind `resolve_target` (`capture.rs:274-298`), which refuses a URL, an absolute path, a colon and a climb above the bundle; the folder form skips symlinks (`service.rs:690-741`).
- `rg -n 'resolve_target|fn read_bundle|read_dir|symlink_metadata' crates/core/src/docs` (idiom: path resolution and folder walks): `resolve_target` (`capture.rs:274`), `read_bundle_blocking` (`service.rs:688`).
Idioms not searched: path joins in the git and archive code, which never take converter output.

| # | Occurrence | Verdict | Disposition |
|---|---|---|---|
| 1 | picture target read `capture.rs:258` | safe: `resolve_target` at `capture.rs:274-298` rejects URLs, absolute paths, colons, climbs; folder form: the map holds only regular files below the output (`service.rs:704-731`); HTTP form: keys are arbitrary strings that are never opened as files (`service.rs:773-779`), only looked up in the map | kept |
| 3 | HTTP file keys `service.rs:773-779` | safe: keys stay map keys, the only resolver is `resolve_target`; covered by E2E-078 | kept |
| 2 | the new table `csv` reference | new surface: must use the same control | FR-NEW-009, tests E2E-035, E2E-078 |

Exploit style tests (literal payloads, other channel: the content of the host file must appear nowhere): E2E-035 (`../../etc/passwd`, `/etc/passwd`, `http://example.com/t.csv`) and E2E-078 (`../tab.csv`).

### 14.3 Design inputs
- Manifest structs gain `version`, `tables`, `headings` and per picture fields; today's parsing uses `serde(default)` and drops errors silently, the new report lines need parse errors surfaced (`docs/service.rs:746-754`). Signal: E2E-011 to E2E-017.
- New stored data: OCR text per image, CSV text and page per table, a structure per set. These change the stored tables of the volume meta store (`storage/meta.rs:114-140`) and the schema golden. Signal: E2E-001, E2E-070.
- Derived headings must use the same rule as the chunking (`chunking/blocks.rs`), code blocks excluded. Signal: E2E-051.
- The metadata line and the new failure lines extend `PendingSet::report` (`capture.rs:149`); line order is stated in FR-NEW-017. Signal: E2E-001, E2E-024.
- The bundle limit and the storage charge must include the new data (`capture.rs:140-145`, `service.rs:690-741`, `core/fs_ops.rs:791`). Signal: E2E-075, E2E-076.
- The built in extraction path builds its own set (`capture.rs:63`); it must keep deriving the structure but add no report line. Signal: E2E-073, E2E-088.
- A CSV file referenced by a table is not a regular file of the project: it lives only as metadata. Signal: E2E-077.
