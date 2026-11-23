# Read tools for the document metadata: functional specification

> Id: SPEC-0104
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 1 bounded context (Documents), 12 requirements, no persisted change. Override with --depth.
> Status: implemented
> Generated on: 2026-11-23
> Project: existing
> From backlog: BL-0167
> Depends on: SPEC-0103
> Security: n/a
> CVSS: n/a
> Affected: n/a
> Fixed in: n/a

## 1. Summary
The previous increment lets a document converter hand over the text read inside each picture, the exact table files and the heading structure, and the platform stores them. Nobody can read them yet. This increment lets a project member read the OCR text next to each image, download the picture file itself, read the heading structure of a document, and read a document as text only or in a full view that carries the OCR text and the structure. The same answers are available through the web interface of the file plane.

## 2. Current State (MANDATORY, every depth)
A member of a project can list the images kept from a document's last conversion with their caption and page, get one image as an encoded picture with its caption and page, list and get the tables as Markdown or CSV (an exact CSV when the converter supplied one), and read the document's whole text with an image line at each image and the tables in a chosen mode. Each of these also exists in the web interface of the file plane with the same answer. None of them shows the OCR text. No read gives the heading structure. A picture can only be had encoded inside an answer, never as a file. A document converted before the previous increment, or by the platform's own reading, has no OCR text and no structure.

### 2.1 What users can do today
- List and get images (caption, page, encoded picture), list and get tables, read the view: see 14.1 rows 1 to 4.
- Nothing reads OCR text, the structure or a raw picture (14.1 rows 5 and 6).

### 2.2 Specs governing this area
- SPEC-0019 (document artifacts): FR-NEW-003 (image list), FR-NEW-004 (get image), FR-NEW-007 (view). This increment adds a field to the first two and a view name to the third; that spec is not edited.
- SPEC-0103 (converter answer with the full document metadata): stores the OCR text and the structure that are read here; its read side was left to this entry.

### 2.3 What is verified today
Each of the five existing reads has end to end tests; none exercises OCR text or structure reads because none exists (14.1 row 7).

## 3. Scope
### 3.1 In scope
- OCR text in the image list and in the image answer.
- The picture file download.
- The structure read and its web twin.
- The view names `standard`, `text` and `full` and their web twin.

### 3.2 Out of scope, and why
- Searching OCR text or headings: a later entry of the roadmap, search is not changed here.
- Tables: the table reads already give Markdown and exact CSV; nothing changes.
- A tool that returns the raw picture bytes: the tool interface carries text; the encoded form stays there (the entry's own note).
- Changing what is stored or how a converter answers: the previous increment.

### 3.3 Constraints imposed by the context
- The answer field names (`ocr_text`, `headings`, `level`, `title`, `page`, `marker`, `structure`) and the view name parameter are part of the public contract read by agents, so they are fixed here; everything else about how they are produced is design. Imposed by the existing tools' contract.
- Public names, fixed here because consumers type them: the structure read is the tool `fs.get_document_structure` with the parameters `mount_id`, `path`, `marker`, and the web route `GET /api/fs/{mount_id}/structure` with the query `path` and `marker`; the view gets the optional parameter `view` (`standard`, `text`, `full`) on the tool `fs.get_document_view` and on the web route `GET /api/fs/{mount_id}/document-view`; the picture download is the web route `GET /api/fs/{mount_id}/image-file` with the query `path` and `id`, its body the picture file and its `Content-Type` the media type. Imposed by the existing tool and route naming.
- Platform administrators have no implicit access to project files (existing rule).

## 4. Actors
- **Project member**: reads the metadata of documents of their project.
- **Non member, including a platform administrator**: refused.

## 4.5 Bounded contexts
| Context | What it owns | Key concepts |
|---|---|---|
| Documents | the conversion of a file and what was kept from it | image, OCR text, table, heading, structure, view |

## 5. Usage scenarios
### SC-001: Read what was read inside the pictures
**Actor:** a member
**Preconditions:** a converted document with images
**Flow:**
1. Alice lists the images → each entry shows its caption, page and OCR text or `null`.
2. She gets one image → the picture and its OCR text.
**Postconditions:** the OCR text is available next to each caption
**Exceptions:**
- EXC-001a: the document is not a file at step 1 → `not a file: <path>`.
- EXC-001b: it was never converted → `No artifacts: document not extracted`.
- EXC-001c: an unknown image at step 2 → `Not found: <id>`.

### SC-002: Download a picture file
**Actor:** a member
**Preconditions:** a converted document
**Flow:**
1. Alice downloads `image-1` → she receives the picture file as the converter supplied it, with its media type.
**Postconditions:** nothing in the project changed
**Exceptions:**
- EXC-002a: unknown image → `Not found: <id>`.
- EXC-002b: not a file or never converted → the refusals of SC-001.

### SC-003: Read the structure
**Actor:** a member
**Preconditions:** a converted document
**Flow:**
1. Alice reads the structure → headings with level, title, page, 100 per answer.
2. She follows the marker → the next answer.
**Postconditions:** she knows the outline of the document
**Exceptions:**
- EXC-003a: invalid marker → refused.
- EXC-003b: never converted or not a file → refused.

### SC-004: Read the document as text only or in full
**Actor:** a member
**Preconditions:** a converted document
**Flow:**
1. Alice asks the view `text` → the converter's text unchanged.
2. She asks `full` → the standard view with OCR lines and the structure.
3. She asks nothing → the view of before.
**Postconditions:** three ways to read the same document
**Exceptions:**
- EXC-004a: unknown view or table mode → refused with the texts of FR-NEW-004.

### SC-005: A document converted before
**Actor:** a member
**Preconditions:** a document converted before SPEC-0103 or by the built in reading
**Flow:**
1. Alice lists the images, reads the structure and the full view → OCR `null`, empty structure, no refusal.
**Postconditions:** nothing was converted again
**Exceptions:**
- none

### SC-006: The same answers through the web interface
**Actor:** a member
**Preconditions:** a converted document
**Flow:**
1. Alice calls the web interface for the images, the structure and the view → the answers equal the tool answers.
**Postconditions:** one payload on both ways in
**Exceptions:**
- EXC-006a: a non member → the same refusal as the tools.

## 6. Functional requirements
### 6.1 New
#### FR-NEW-001 [EARS-E]: Download a picture
> WHEN a member downloads an image of a converted document THE Documents context SHALL deliver the picture file exactly as the converter supplied it, labelled with the media type of its format.

- **Business rules:** Media types: `png` is `image/png`, `jpeg` is `image/jpeg`, `gif` is `image/gif`, `webp` is `image/webp`, `tiff` is `image/tiff`, `bmp` is `image/bmp`. The image is named by its id (`image-1`); an id that is not exactly `image-` followed by a whole number of 1 or more with no leading zero is unknown. The refusals come in this order: not a file, no conversion, unknown image. The download is available through the web interface only; the tool interface keeps the encoded form of `get image`. The download response carries `Content-Disposition: attachment; filename="<id>.<ext>"` where `<ext>` is `png`, `jpg`, `gif`, `webp`, `tiff` or `bmp` for the six formats, `X-Content-Type-Options: nosniff` and a `Content-Length` equal to the byte count; no other body is sent on success.
- **User visible text:** `ERR_NOT_FOUND: not a file: <path>`, `ERR_NOT_FOUND: No artifacts: document not extracted`, `ERR_NOT_FOUND: Not found: <id as typed>`
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-002 [EARS-E]: Read the structure
> WHEN a member reads the structure of a converted document THE Documents context SHALL return its headings in document order, each with its level (1 to 6), its title and its page, 100 headings per answer with a marker for the next answer.

- **Business rules:** A heading without a page shows page `null`. The answer is `{"headings": [{"level": 2, "title": "Costs", "page": 2}], "marker": null}`; the marker is `null` on the last answer. A marker is valid only for the same project, the same kind of read and the same document: a marker given by the image list or the table list is invalid here. A marker is a position, not a conversion version: after a new conversion it is read against the new one, and a position past the end gives an empty list with marker `null`. An invalid marker is refused with `ERR_INVALID_ARGUMENT: Invalid continuation marker: <raw>`.
- **User visible text:** `ERR_INVALID_ARGUMENT: Invalid continuation marker: <raw>`
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-003 [EARS-O]: Structure refusals and empty structure
> IF the document is not a file, or was never converted, THEN THE Documents context SHALL refuse the structure read with the existing refusal for that case, AND IF the document was converted but has no structure THEN THE Documents context SHALL answer an empty list.

- **Business rules:** Refusal order: not a file, invalid marker, no conversion (the order of the image list). An empty structure is an empty list with marker `null`, never a refusal.
- **User visible text:** `ERR_NOT_FOUND: not a file: <path>`, `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-004 [EARS-E]: Choose the view
> WHEN a member reads a converted document's view THE Documents context SHALL accept the view `standard`, `text` or `full`, and `standard` when none is given.

- **Business rules:** The name is matched with its exact letter case. Any other value is refused before the document is read, in the order: not a file, unsupported view, unsupported table mode, no conversion. The table mode is checked whatever the view.
- **User visible text:** `ERR_INVALID_ARGUMENT: Unsupported view: <value> (use standard, text or full)`
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-005 [EARS-S]: Text only view
> WHILE the view is `text` THE Documents context SHALL return the conversion text exactly as the converter returned it, with no image line, no OCR line and no table rendering added.

- **Business rules:** The table mode and the captions choice have no effect on the `text` view. The `standard` and `text` answers are exactly `{path, text}`, with no `structure` field; the `full` answer is `{path, text, structure}`.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-006 [EARS-S]: Full view
> WHILE the view is `full` THE Documents context SHALL return the standard view with, right after the image line of each image that has OCR text, the line `[OCR image-<n>: <text>]`, and with the document's structure.

- **Business rules:** The OCR text is written as stored; a line break inside it stays a line break and the closing `]` follows the last character. An image with no OCR text gets no OCR line. The OCR line does not depend on the captions choice. The structure is a list `structure` of `{level, title, page}` in document order, all headings with no limit, even above 100, empty when the document has none. Tables follow the chosen table mode as in the standard view. Placement: the OCR line is written immediately after the image line with no blank line between them, with the same line terminator as the image line; the blank line that follows an image line in the standard view follows the OCR line instead; if the image line is the last of the text and has no terminator, the OCR line is appended after one line break and carries none; with two adjacent images, the shared blank line comes after the OCR line of the first (if any) and before the image line of the second.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-007 [EARS-UB]: Documents converted before
> A document converted before the metadata existed, or by the platform's own reading, SHALL NOT be refused or converted again by any of these reads.

- **Business rules:** Its images show OCR text `null`, its structure is an empty list, its `full` view equals its `standard` view plus an empty `structure`.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-008 [EARS-U]: The same answers through the web interface
> The Documents context SHALL give through the web interface of the file plane the same answers, with the same refusals and texts, as through the tools for the image list, the image, the structure and the view.

- **Business rules:** Equality is on the success payload, byte for byte. A refusal through the web interface keeps the existing web error body (the status of the error code, and a JSON body with `error` holding the code and `detail` holding `<code>: <text>`); the tool refusal text and the web `detail` text are the same. The download refuses with that JSON body and delivers no picture byte; on success its body is the file bytes and its `Content-Type` is the media type. The download exists in the web interface only.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-009 [EARS-UB]: Access and no side effect
> The reads of this increment SHALL be open to the members of the project only, AND SHALL NOT change any file, any access time, the storage used or the document's conversion.

- **Business rules:** A non member is refused with `ERR_FORBIDDEN: '<person>' is not a member of '<project>'`; an unknown project with `ERR_PROJECT_NOT_FOUND: project '<id>' not found`. Platform administrators have no implicit access.
- **User visible text:** `ERR_FORBIDDEN: '<person>' is not a member of '<project>'`
- **Scenario:** SC-002
- **Priority:** Must

### 6.2 Modified
#### FR-MOD-001 [EARS-E]: The image list carries the OCR text (modifies SPEC-0019/FR-NEW-003)
> WHEN a member lists the images of a converted document THE Documents context SHALL give with each image its OCR text, `null` when none was kept.

- **Before and after:** Before: `{id, caption, page}`. After: `{id, caption, page, ocr_text}`. Only a zero length text is `null` (never an empty string); a text of only spaces or line breaks is given as stored. The text is given in full, so an answer of 100 images can be large (up to 100 000 characters each).
- **Reason:** make the stored metadata readable.
- **Scenario:** SC-001

#### FR-MOD-002 [EARS-E]: The image carries the OCR text (modifies SPEC-0019/FR-NEW-004)
> WHEN a member gets one image of a converted document THE Documents context SHALL give its OCR text with the existing answer, `null` when none was kept.

- **Before and after:** Before: `format`, `caption`, `page`, `base64`. After: the same plus `ocr_text`.
- **Reason:** make the stored metadata readable.
- **Scenario:** SC-001

#### FR-MOD-003 [EARS-E]: The view takes a view name (modifies SPEC-0019/FR-NEW-007)
> WHEN a member reads a converted document's view without naming a view THE Documents context SHALL return exactly the answer it returned before this increment.

- **Before and after:** Before: image lines with captions and tables in the chosen mode, no OCR, no structure. After: unchanged by default; `text` and `full` are the additions.
- **Reason:** make the stored metadata readable.
- **Scenario:** SC-004

### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
An image list, a structure read or a view of a document with 10 000 headings or 100 images completes within 5 seconds for the member. Not changed for the existing reads.
### 7.2 Security and access
Members only, as FR-NEW-009. A picture is delivered as a download labelled with its media type and never interpreted as a page by the member's browser (the media type is the picture's own).
### 7.3 Privacy and compliance
OCR text and pictures are project content: they are shown to members only and to nobody else.
### 7.4 Usability and accessibility
Not applicable: no screen is added.
### 7.5 Reliability
Not changed: a conversion replaces the whole set at once (SPEC-0103), so a read sees one conversion; no new requirement.
### 7.6 Operability
Not changed: the reads follow the existing request records.

## 8. Edge cases
| Category | Case | Expected behavior | Test |
|---|---|---|---|
| Empty states | no OCR text; no structure; never converted | `null`; empty list; refused | E2E-025, E2E-029, E2E-015 |
| Boundary values | 100 and 101 images; 150 headings; 5 MB picture | paged; paged; whole | E2E-031, E2E-007, E2E-032 |
| Concurrent actions | a new conversion replaces the old | the new one is read | E2E-030 |
| Slow or lost connectivity | not applicable: single request reads | | |
| Access states | non member; unknown project | refused | E2E-011, E2E-023 |
| Data variations | accents, emoji, line break in OCR text | as stored | E2E-027 |
| Navigation | marker not valid; image removed by a re-conversion | refused | E2E-017, E2E-024 |
| Partial operations | not applicable: reads | | |
| Dependency failures | not applicable: nothing external is called | | |
| Ordering and timing | headings in document order; images in reading order | stated order | E2E-006, E2E-002 |
| Exhaustion | not applicable | | |

## 9. Acceptance tests (the contract)
> Behavioral and technology agnostic. Convention: **document D** is `/report.pdf` in project `proj`, converted by a converter that returned this Markdown: `# Report`, a picture, `## Costs`, a table captioned `Costs` with an exact CSV, `## Outlook`, a second picture; two pictures: `image-1` (PNG, caption `Figure 1: Revenue 2024`, page 1, OCR text `Revenue 2024: 4.2 M EUR`) and `image-2` (JPEG, caption `Schéma réseau 🌐`, page 3, no OCR text); and the headings level 1 `Report` page 1, level 2 `Costs` page 2, level 2 `Outlook` page 3. "The standard view" is the answer the view gave before this increment. "The web interface" is the second way in, with answers identical to the tools'.

### 9.1 Summary
| Test | Action | Category | Scenario | Requirements | Priority |
|---|---|---|---|---|---|
| E2E-001 | New | journey | SC-001 | FR-MOD-001, FR-MOD-002, FR-NEW-001, FR-NEW-002, FR-NEW-006, FR-NEW-007 | Critical |
| E2E-002 | New | requirement | SC-001 | FR-MOD-001 | High |
| E2E-003 | New | requirement | SC-001 | FR-MOD-002 | High |
| E2E-004 | New | requirement | SC-002 | FR-NEW-001 | High |
| E2E-005 | New | requirement | SC-002 | FR-NEW-001 | High |
| E2E-006 | New | requirement | SC-003 | FR-NEW-002 | High |
| E2E-007 | New | requirement | SC-003 | FR-NEW-002 | High |
| E2E-008 | New | requirement | SC-004 | FR-NEW-004, FR-NEW-005 | High |
| E2E-009 | New | requirement | SC-004 | FR-NEW-004, FR-NEW-006 | High |
| E2E-010 | New | requirement | SC-004 | FR-MOD-003, FR-NEW-004 | High |
| E2E-011 | New | failure | SC-001 | FR-NEW-009, FR-MOD-001, FR-NEW-001, FR-NEW-002, FR-NEW-004 | High |
| E2E-012 | New | failure | SC-002 | FR-NEW-001 | High |
| E2E-013 | New | failure | SC-002 | FR-NEW-001 | High |
| E2E-014 | New | failure | SC-002 | FR-NEW-001 | High |
| E2E-015 | New | failure | SC-003 | FR-NEW-003 | High |
| E2E-016 | New | failure | SC-003 | FR-NEW-003 | High |
| E2E-017 | New | failure | SC-003 | FR-NEW-002, FR-NEW-003 | Medium |
| E2E-018 | New | failure | SC-004 | FR-NEW-004 | High |
| E2E-019 | New | failure | SC-004 | FR-NEW-004, FR-MOD-003 | Medium |
| E2E-020 | New | failure | SC-004 | FR-NEW-004 | Medium |
| E2E-021 | New | failure | SC-001 | FR-MOD-002 | Medium |
| E2E-022 | New | failure | SC-002 | FR-NEW-001 | Medium |
| E2E-023 | New | failure | SC-002 | FR-NEW-009 | Medium |
| E2E-024 | New | failure | SC-002 | FR-NEW-001 | Medium |
| E2E-025 | New | edge | SC-001 | FR-MOD-001, FR-MOD-002 | High |
| E2E-026 | New | requirement | SC-005 | FR-NEW-007 | High |
| E2E-027 | New | edge | SC-001 | FR-NEW-006, FR-MOD-001 | Medium |
| E2E-028 | New | edge | SC-004 | FR-NEW-006 | Medium |
| E2E-029 | New | edge | SC-004 | FR-NEW-006, FR-NEW-003 | Medium |
| E2E-030 | New | edge | SC-001 | FR-MOD-001, FR-NEW-002, FR-NEW-006 | High |
| E2E-031 | New | edge | SC-001 | FR-MOD-001 | Medium |
| E2E-032 | New | edge | SC-002 | FR-NEW-001 | Medium |
| E2E-033 | New | edge | SC-004 | FR-NEW-005 | Medium |
| E2E-034 | New | edge | SC-003 | FR-NEW-002 | Medium |
| E2E-035 | New | edge | SC-005 | FR-NEW-007, FR-NEW-003 | Medium |
| E2E-036 | New | side effect | SC-006 | FR-NEW-008 | High |
| E2E-037 | New | side effect | SC-006 | FR-NEW-008 | High |
| E2E-038 | New | side effect | SC-006 | FR-NEW-008, FR-MOD-001 | High |
| E2E-039 | New | side effect | SC-002 | FR-NEW-009 | High |
| E2E-040 | New | side effect | SC-002 | FR-NEW-001, FR-NEW-009 | Medium |
| E2E-041 | New | edge | SC-002 | FR-NEW-001 | Medium |
| E2E-042 | New | edge | SC-004 | FR-NEW-006, FR-MOD-003 | Medium |
| E2E-043 | New | failure | SC-004 | FR-NEW-004, FR-NEW-005 | Medium |
| E2E-044 | New | failure | SC-006 | FR-NEW-008, FR-NEW-009 | High |
| E2E-045 | New | failure | SC-003 | FR-NEW-002, FR-NEW-003 | Medium |
| E2E-046 | New | edge | SC-003 | FR-NEW-002 | Medium |
| E2E-047 | New | failure | SC-006 | FR-NEW-001, FR-NEW-008 | High |
| E2E-048 | New | edge | SC-001 | FR-MOD-001 | Medium |
| E2E-049 | New | edge | SC-004 | FR-NEW-006 | Medium |
| E2E-050 | New | edge | SC-004 | FR-NEW-006 | Medium |
| E2E-051 | New | requirement | SC-002 | FR-NEW-001 | High |

Coverage by category: journey 1, requirement 11, failure 18, edge 16, side effect 5. Happy (journey and requirement) 12, failure 18: happy to failure ratio 1:1.5 (beats 1:1).

### 9.2 New tests
#### E2E-001: a converted document read end to end
- **Category:** journey
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001, FR-MOD-002, FR-NEW-001, FR-NEW-002, FR-NEW-006, FR-NEW-007
- **Given** project `proj`, Alice (`alice@example.com`) a member, document D converted
- **When** Alice lists the images, gets `image-1`, downloads `image-1`, reads the structure, then reads the full view
- **Then** the list shows `image-1` with OCR text `Revenue 2024: 4.2 M EUR` and `image-2` with no text; the get answer for `image-1` carries the same text and its picture; the download is the picture file of `image-1`; the structure lists `Report`, `Costs`, `Outlook` in that order; the full view shows `[OCR image-1: Revenue 2024: 4.2 M EUR]` on the line after the image line of `image-1`
- **And** nothing in the project's files changed: the file list before and after is identical
- **Priority:** Critical

#### E2E-002: the image list carries the OCR text
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** document D converted
- **When** Alice lists the images of `/report.pdf`
- **Then** two entries in reading order: `image-1` (caption `Figure 1: Revenue 2024`, page 1, OCR text `Revenue 2024: 4.2 M EUR`) and `image-2` (caption `Schéma réseau 🌐`, page 3, OCR text `null`), and no next page marker
- **Priority:** High

#### E2E-003: one image carries the OCR text
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-MOD-002
- **Given** document D converted
- **When** Alice gets `image-1`
- **Then** the answer holds the format `png`, the caption, the page 1, the OCR text `Revenue 2024: 4.2 M EUR` and the encoded picture, as before plus the text
- **Priority:** High

#### E2E-004: download a PNG picture
- **Category:** requirement
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted; the converter supplied `image-1` as a PNG file of 2 048 bytes
- **When** Alice downloads `image-1`
- **Then** she receives 2 048 bytes identical to the file the converter supplied, labelled `image/png`
- **Priority:** High

#### E2E-005: download a JPEG picture
- **Category:** requirement
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted; `image-2` is a JPEG file of 1 500 bytes
- **When** Alice downloads `image-2`
- **Then** she receives 1 500 bytes identical to the supplied file, labelled `image/jpeg`
- **Priority:** High

#### E2E-006: the structure in reading order
- **Category:** requirement
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002
- **Given** document D converted
- **When** Alice reads the structure of `/report.pdf`
- **Then** three headings in this order: level 1 `Report` page 1, level 2 `Costs` page 2, level 2 `Outlook` page 3, and no next page marker
- **Priority:** High

#### E2E-007: the structure is read 100 headings at a time
- **Category:** requirement
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002
- **Given** a document converted with 150 headings titled `Section 1` to `Section 150`
- **When** Alice reads the structure, then reads again with the next page marker
- **Then** the first answer holds `Section 1` to `Section 100` and a marker; the second holds `Section 101` to `Section 150` and no marker
- **Priority:** High

#### E2E-008: the text only view
- **Category:** requirement
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-NEW-005
- **Given** document D converted
- **When** Alice reads the view of `/report.pdf` as `text`
- **Then** she receives exactly the Markdown text the converter returned, with no image line and no table rendering added
- **And** the answer holds no `[Image` line and no `[OCR` line
- **Priority:** High

#### E2E-009: the full view
- **Category:** requirement
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-NEW-006
- **Given** document D converted
- **When** Alice reads the view as `full`
- **Then** the text is the standard view with the line `[OCR image-1: Revenue 2024: 4.2 M EUR]` right after the image line of `image-1`, and no OCR line for `image-2`; the answer also carries the structure: `Report`, `Costs`, `Outlook` with their levels and pages
- **Priority:** High

#### E2E-010: the default view is unchanged
- **Category:** requirement
- **Scenario:** SC-004
- **Requirements:** FR-MOD-003, FR-NEW-004
- **Given** document D converted
- **When** Alice reads the view without choosing a view, then as `standard`
- **Then** both answers are identical to the view the previous release gave for D (image lines with captions, tables in Markdown), with no OCR line and no structure
- **Priority:** High

#### E2E-011: a non member reads nothing
- **Category:** failure
- **Scenario:** SC-001
- **Requirements:** FR-NEW-009, FR-MOD-001, FR-NEW-001, FR-NEW-002, FR-NEW-004
- **Given** document D converted; Eve (`eve@example.com`) is not a member of `proj`
- **When** Eve lists the images, downloads `image-1`, reads the structure and reads the view
- **Then** each is refused with `ERR_FORBIDDEN: 'eve@example.com' is not a member of 'proj'`
- **And** no picture bytes, OCR text or heading appears in any answer
- **Priority:** High

#### E2E-012: download an unknown image
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted (two images)
- **When** Alice downloads `image-9`
- **Then** she is refused with `ERR_NOT_FOUND: Not found: image-9`
- **And** no bytes are delivered
- **Priority:** High

#### E2E-013: download from a path that is not a file
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** project `proj` with a folder `/docs`
- **When** Alice downloads `image-1` of `/docs`
- **Then** she is refused with `ERR_NOT_FOUND: not a file: /docs`
- **Priority:** High

#### E2E-014: download from a document never converted
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** a file `/plain.pdf` that was never converted
- **When** Alice downloads `image-1` of `/plain.pdf`
- **Then** she is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Priority:** High

#### E2E-015: structure of a document never converted
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** a file `/plain.pdf` that was never converted
- **When** Alice reads its structure
- **Then** she is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Priority:** High

#### E2E-016: structure of a path that is not a file
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** project `proj` with a folder `/docs`
- **When** Alice reads the structure of `/docs`
- **Then** she is refused with `ERR_NOT_FOUND: not a file: /docs`
- **Priority:** High

#### E2E-017: structure with a marker that is not valid
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002, FR-NEW-003
- **Given** document D converted
- **When** Alice reads the structure with the marker `garbage`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Invalid continuation marker: garbage`
- **And** no heading is returned
- **Priority:** Medium

#### E2E-018: an unknown view
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004
- **Given** document D converted
- **When** Alice reads the view as `pretty`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Unsupported view: pretty (use standard, text or full)`
- **Priority:** High

#### E2E-019: an unknown table mode with the full view
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-MOD-003
- **Given** document D converted
- **When** Alice reads the view as `full` with the table mode `grid`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Unsupported table mode: grid (use markdown, csv-reference or both)`
- **Priority:** Medium

#### E2E-020: the view name is case sensitive
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004
- **Given** document D converted
- **When** Alice reads the view as `Full`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Unsupported view: Full (use standard, text or full)`
- **Priority:** Medium

#### E2E-021: get an unknown image
- **Category:** failure
- **Scenario:** SC-001
- **Requirements:** FR-MOD-002
- **Given** document D converted
- **When** Alice gets `image-9`
- **Then** she is refused with `ERR_NOT_FOUND: Not found: image-9`
- **Priority:** Medium

#### E2E-022: download with an image id that is not canonical
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted
- **When** Alice downloads `image-01`, then `image-0`, then `picture-1`
- **Then** each is refused with `ERR_NOT_FOUND: Not found: <the id typed>`
- **Priority:** Medium

#### E2E-023: an unknown project
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-009
- **Given** no project `ghost`
- **When** Alice reads the structure of `/report.pdf` in `ghost` and downloads `image-1` from it
- **Then** each is refused with `ERR_PROJECT_NOT_FOUND: project 'ghost' not found`
- **Priority:** Medium

#### E2E-024: an image that a re-conversion removed
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted with two images, then converted again by a converter that returns one image
- **When** Alice downloads `image-2`
- **Then** she is refused with `ERR_NOT_FOUND: Not found: image-2`
- **And** `image-1` still downloads and is the new picture
- **Priority:** Medium

#### E2E-025: an image with no OCR text
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001, FR-MOD-002
- **Given** document D converted
- **When** Alice gets `image-2`
- **Then** the OCR text is `null`, the caption `Schéma réseau 🌐` and page 3 are as converted
- **And** a picture whose OCR text was empty or absent is also `null`, never an empty string
- **Priority:** High

#### E2E-026: a document converted before the metadata existed
- **Category:** requirement
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** `/old.pdf` converted by a release that stored no OCR text and no structure, one image captioned `Old figure`
- **When** Alice lists the images, reads the structure, reads the view as `full`
- **Then** the image has OCR text `null`; the structure is an empty list with no marker, not a refusal; the full view equals the standard view and carries an empty structure
- **And** nothing is converted again: the document's conversion is untouched
- **Priority:** High

#### E2E-027: accents, emoji and line breaks in OCR text
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-006, FR-MOD-001
- **Given** a document whose `image-1` has the OCR text `Café 🌐
ligne 2` (a line break between)
- **When** Alice lists the images and reads the full view
- **Then** the list shows the text exactly as stored; the full view shows `[OCR image-1: Café 🌐` then `ligne 2]` on the next line
- **Priority:** Medium

#### E2E-028: captions hidden, OCR text still shown
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** document D converted
- **When** Alice reads the full view with captions off
- **Then** the image line of `image-1` reads `[Image image-1]` with no caption and the line `[OCR image-1: Revenue 2024: 4.2 M EUR]` still follows it
- **Priority:** Medium

#### E2E-029: the full view of a document with no headings
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-003
- **Given** a document converted whose text has no heading and whose converter gave an empty structure
- **When** Alice reads the full view and the structure
- **Then** the full view carries an empty structure; the structure read is an empty list with no marker, not a refusal
- **Priority:** Medium

#### E2E-030: a new conversion replaces what is read
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001, FR-NEW-002, FR-NEW-006
- **Given** document D converted, then converted again by a converter returning one image with OCR text `Q3 chart` and one heading `Summary`
- **When** Alice lists the images, reads the structure and the full view
- **Then** one image with OCR text `Q3 chart`; one heading `Summary`; the full view shows `[OCR image-1: Q3 chart]` and nothing of the first conversion
- **Priority:** High

#### E2E-031: exactly 100 and 101 images
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** a document converted with 100 images, and another with 101, each with OCR text `t<n>`
- **When** Alice lists the images of each
- **Then** 100 images and no marker for the first; 100 images and a marker for the second, and the next page holds `image-101` with `t101`
- **Priority:** Medium

#### E2E-032: a large picture downloads whole
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** a document converted whose `image-1` is a PNG file of 5 000 000 bytes
- **When** Alice downloads `image-1`
- **Then** she receives 5 000 000 bytes identical to the supplied file
- **Priority:** Medium

#### E2E-033: a table mode is ignored by the text view
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-005
- **Given** document D converted
- **When** Alice reads the view as `text` with the table mode `csv-reference`
- **Then** the answer equals the `text` view without a table mode
- **Priority:** Medium

#### E2E-034: a heading without a page
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002
- **Given** a document converted whose converter gave the heading `Appendix` level 2 with no page
- **When** Alice reads the structure
- **Then** the heading shows level 2, title `Appendix` and page `null`
- **Priority:** Medium

#### E2E-035: a document converted by the built in reading
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-003
- **Given** `/doc.docx` converted by the platform's own reading, with no converter
- **When** Alice reads its structure and its view as `full`
- **Then** the structure is an empty list; the full view equals the standard view plus an empty structure
- **Priority:** Medium

#### E2E-036: the structure through the web interface
- **Category:** side effect
- **Scenario:** SC-006
- **Requirements:** FR-NEW-008
- **Given** document D converted
- **When** Alice reads the structure through the web interface and through the tool
- **Then** both answers are identical, byte for byte
- **Priority:** High

#### E2E-037: every view through the web interface
- **Category:** side effect
- **Scenario:** SC-006
- **Requirements:** FR-NEW-008
- **Given** document D converted
- **When** Alice reads the view as `standard`, `text` and `full` through the web interface and through the tool
- **Then** each pair of answers is identical, and a bad view is refused with the same code and text
- **Priority:** High

#### E2E-038: the image list through the web interface
- **Category:** side effect
- **Scenario:** SC-006
- **Requirements:** FR-NEW-008, FR-MOD-001
- **Given** document D converted
- **When** Alice lists the images through the web interface and through the tool
- **Then** both answers are identical and carry the OCR text
- **Priority:** High

#### E2E-039: a download changes nothing
- **Category:** side effect
- **Scenario:** SC-002
- **Requirements:** FR-NEW-009
- **Given** document D converted; the file list, its access times and the project's used storage noted
- **When** Alice downloads `image-1` and reads the structure
- **Then** the file list, the access time of `/report.pdf` and the used storage are unchanged
- **And** the conversion of `/report.pdf` is still the same one
- **Priority:** High

#### E2E-040: downloading twice gives the same bytes
- **Category:** side effect
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001, FR-NEW-009
- **Given** document D converted
- **When** Alice downloads `image-1` twice
- **Then** both downloads are identical, and equal the supplied file
- **Priority:** Medium

#### E2E-041: a TIFF picture is labelled as TIFF
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** a document converted whose `image-1` is a TIFF file
- **When** Alice downloads `image-1`
- **Then** she receives the file as supplied, labelled `image/tiff`
- **And** the same holds for GIF (`image/gif`), WebP (`image/webp`) and BMP (`image/bmp`)
- **Priority:** Medium

#### E2E-042: tables in the full view follow the chosen mode
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-MOD-003
- **Given** document D converted (its table has an exact CSV)
- **When** Alice reads the full view with the table mode `csv-reference`
- **Then** the table shows as the reference line the standard view gives for that mode, and the OCR line and structure are present
- **Priority:** Medium

#### E2E-043: a table mode is checked whatever the view
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-NEW-005
- **Given** document D converted
- **When** Alice reads the view as `text` with the table mode `grid`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Unsupported table mode: grid (use markdown, csv-reference or both)`
- **Priority:** Medium

#### E2E-044: a non member through the web interface
- **Category:** failure
- **Scenario:** SC-006
- **Requirements:** FR-NEW-008, FR-NEW-009
- **Given** document D converted; Eve (`eve@example.com`) is not a member of `proj`
- **When** Eve lists the images, reads the structure and downloads `image-1` through the web interface
- **Then** each is refused with the status of `ERR_FORBIDDEN` and the body `error` = `ERR_FORBIDDEN`, `detail` = `ERR_FORBIDDEN: 'eve@example.com' is not a member of 'proj'`
- **And** no picture byte is delivered
- **Priority:** High

#### E2E-045: a marker from another read is refused
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002, FR-NEW-003
- **Given** a document (not D) converted with 150 images; Alice holds the marker the image list gave her
- **When** Alice reads the structure of `/report.pdf` with that marker
- **Then** she is refused with `ERR_INVALID_ARGUMENT: Invalid continuation marker: <that marker>`
- **And** no heading is returned
- **Priority:** Medium

#### E2E-046: 10 000 headings read page by page
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-002
- **Given** a document converted with 10 000 headings titled `H1` to `H10000`
- **When** Alice reads the structure following every marker
- **Then** 100 answers of 100 headings, in order `H1` to `H10000`, the last with marker `null`; the full view of the same document carries all 10 000 headings
- **Priority:** Medium

#### E2E-047: a refused download through the web interface
- **Category:** failure
- **Scenario:** SC-006
- **Requirements:** FR-NEW-001, FR-NEW-008
- **Given** document D converted
- **When** Alice downloads `image-9` through the web interface
- **Then** the status is that of `ERR_NOT_FOUND` and the body is `error` = `ERR_NOT_FOUND`, `detail` = `ERR_NOT_FOUND: Not found: image-9`
- **And** no picture byte is delivered
- **Priority:** High

#### E2E-048: OCR text made only of spaces
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** a document whose `image-1` has the OCR text `   ` (three spaces) and `image-2` an empty OCR text
- **When** Alice lists the images
- **Then** `image-1` shows the OCR text `   ` as stored and `image-2` shows `null`
- **Priority:** Medium

#### E2E-049: the OCR line when the image line ends the text
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** a document whose text ends with its image line `[Image image-1]` with no final line break, `image-1` having OCR text `fin`
- **When** Alice reads the full view
- **Then** the text ends with the image line of `image-1`, one line break, then `[OCR image-1: fin]` and no final line break
- **Priority:** Medium

#### E2E-050: two adjacent images
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** a document whose text has two pictures one after the other, `image-1` with OCR text `a` and `image-2` with OCR text `b`
- **When** Alice reads the full view
- **Then** the lines come in this order: the image line of `image-1`, `[OCR image-1: a]`, one blank line, the image line of `image-2`, `[OCR image-2: b]`
- **Priority:** Medium

#### E2E-051: download headers
- **Category:** requirement
- **Scenario:** SC-002
- **Requirements:** FR-NEW-001
- **Given** document D converted; `image-1` is 2 048 bytes and `image-2` 1 500 bytes
- **When** Alice downloads `image-1` and `image-2` through the web interface
- **Then** the first answer carries `Content-Disposition: attachment; filename="image-1.png"`, `X-Content-Type-Options: nosniff` and `Content-Length: 2048`; the second carries `filename="image-2.jpg"` and `Content-Length: 1500`
- **Priority:** High

### 9.3 Modified tests
None: the existing tests of the image list, the image and the view keep their expectations, except that a list or image answer now also holds the OCR text field (`null` for every existing fixture); each such existing test is listed by `/sdlc-design`.

### 9.4 Removed tests
None.

## 10. Traceability
| Scenario | Requirements | Happy | Failure | Edge |
|---|---|---|---|---|
| SC-001 | FR-MOD-001, FR-MOD-002, FR-NEW-001, FR-NEW-002, FR-NEW-004, FR-NEW-006, FR-NEW-007, FR-NEW-009 | E2E-001, E2E-002, E2E-003 | E2E-011, E2E-021 | E2E-025, E2E-027, E2E-030, E2E-031, E2E-048 |
| SC-002 | FR-NEW-001, FR-NEW-009 | E2E-004, E2E-005, E2E-051 | E2E-012, E2E-013, E2E-014, E2E-022, E2E-023, E2E-024 | E2E-032, E2E-039, E2E-040, E2E-041 |
| SC-003 | FR-NEW-002, FR-NEW-003 | E2E-006, E2E-007 | E2E-015, E2E-016, E2E-017, E2E-045 | E2E-034, E2E-046 |
| SC-004 | FR-MOD-003, FR-NEW-003, FR-NEW-004, FR-NEW-005, FR-NEW-006 | E2E-008, E2E-009, E2E-010 | E2E-018, E2E-019, E2E-020, E2E-043 | E2E-028, E2E-029, E2E-033, E2E-042, E2E-049, E2E-050 |
| SC-005 | FR-NEW-003, FR-NEW-007 | E2E-026 |  | E2E-035 |
| SC-006 | FR-MOD-001, FR-NEW-001, FR-NEW-008, FR-NEW-009 |  | E2E-044, E2E-047 | E2E-036, E2E-037, E2E-038 |

## 11. Open questions
None.

## 12. Glossary (MANDATORY)
| Term | Definition | Context |
|---|---|---|
| Document D | the reference converted document of the tests | tests |
| OCR text | the text read inside a picture, stored with the image | Documents |
| Structure | the headings of a document in order, each with level, title, page | Documents |
| View | a document's text read as `standard`, `text` or `full` | Documents |
| Standard view | the view of before this increment | Documents |
| Media type | the label telling what kind of file a download is | Documents |
| Web interface | the second way in to the same reads, beside the tools | Documents |
| Marker | the token that asks for the next page of an answer | Documents |

## 13. Implementability gate (MANDATORY)
| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 5 | 5 | NOT-IMPLEMENTABLE |
| 2 | 4 | 5 | NOT-IMPLEMENTABLE |
| 3 | 0 | 0 | IMPLEMENTABLE |

**Amendments applied:** public names and headers; refusal shape of the web interface; answer shapes per view; marker validity; OCR line placement; whitespace OCR text; tests E2E-043 to E2E-051; 14.3 inputs. **No implementation detail outside Section 14 (except the fixed public names):** PASS

## 14. Evidence appendix (for /sdlc-design and the auditor, not for the product owner)
### 14.1 Current State evidence
| Claim (Section 2) | Evidence |
|---|---|
| Rows 1 to 4: images list `{id, caption, page}`, get image encoded, tables, view with image line | `crates/core/src/tools/artifacts.rs:135-186,238-311,313-350`; `TOOL_CONTRACT.txt:236-246,306-311,794-798` (opened, line numbers verified) |
| Each exists in the web interface | `crates/core/src/api/dataplane.rs:73-77,170-174` |
| Row 5: OCR text, structure are stored (SPEC-0103) | `crates/core/src/docs/artifacts/capture.rs:26-36`; `crates/core/src/storage/meta.rs:274,301` |
| Row 6: no raw download, no structure read | `rg 'doc_headings' crates/core/src/tools` returns nothing; raw bytes read at `artifacts.rs:182` only to encode |
| Row 7: end to end tests of the existing reads | `crates/core/src/tools/artifacts.rs` module `e2e` (from line 360) |
| Picture formats | `crates/core/src/docs/artifacts/capture.rs:25` (`png`, `jpeg`, `gif`, `webp`, `tiff`, `bmp`) |
| Refusal texts reused | `artifacts.rs:174,228`; `docs/artifacts/render.rs:24,62` |
Test command: `cargo test -p fsa-core --lib -- tools::artifacts api::dataplane`

### 14.3 Design inputs
- No store read returns OCR text, headings or `doc_tables.page`; new readers needed (`storage/meta.rs`, headings only through a test helper at `meta.rs:2614`).
- The view reader `artifact_view_images` (`meta.rs:1450`) does not select OCR text; the full view needs it.
- A raw download is not a JSON answer: the web interface needs a non JSON response; its route name and media type table are design.
- The structure tool raises the tool count (135 in `AGENTS.md`) and changes `TOOL_CONTRACT.txt`, `tool-contract-golden.json` and the route list (`dataplane.rs:73`); the golden contract test fails until regenerated (the detection signal). A new marker kind is needed in `docs/artifacts/mod.rs:52` (`enum ListKind`, `as_str`) and `decode_marker`.
- SPEC-0103 says artifacts are session weight only for the storage charge; reads here are unaffected.
- The list answer can reach about 10 MB for 100 images with 100 000 character texts; the full text is required (FR-MOD-001), so no bound is applied.
- The store readers behind the image list and the image (`list_artifact_images`, `artifact_image`) must also return `ocr_text`; a new reader for headings is needed, unlimited for the `full` view and with a limit and offset for the structure read.
- Also update `crates/core/src/api/openapi.rs` (rows for `structure` and `image-file`, the latter with no tool, and the `view` query), the tool-name list at `tools/mod.rs:476`, the `fs.get_document_view` argument struct in `mcp/server` (new `view` field), and a new argument struct for `fs.get_document_structure`.
- `render::ViewImage` and `render::document_view` (`docs/artifacts/render.rs:68,~100`) must also carry and place the OCR text.
- A binary success response needs a handler variant of `guarded` (`api/dataplane.rs:~253`) that keeps the membership guard and `error_response` for refusals.
- Whether the read of artifacts touches the access time today: E2E-039 requires it does not; check the existing reads at `artifacts.rs` and `safety.rs`.
