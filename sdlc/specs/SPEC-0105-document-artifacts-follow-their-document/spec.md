# Document artifacts follow their document: functional specification

> Id: SPEC-0105
> Nature: FEAT
> Depth: L
> Depth evidence: FEAT / depth L: 4 areas (file operations, trash and restore, purge, session quota) and 15 requirements. Override with --depth.
> Status: implemented
> Generated on: 2026-11-23
> Project: existing
> From backlog: BL-0038
> Depends on: none
> Security: n/a
> CVSS: n/a
> Affected: n/a
> Fixed in: n/a

## 1. Summary
A converted document has images, tables, headings and a conversion text kept with it. Today any move, rename, copy or deletion to the trash makes them go away, so a member must convert again after tidying a folder or restoring a file. This increment makes them follow the document through move, rename, copy, trash, restore, overwrite and purge, with no new conversion.

## 2. Current State (MANDATORY, every depth)
The artifacts of a document are tied to the document's path and to the exact version of the file they were built from. Any move, rename, copy, deletion to the trash or restore gives the file a new version, so the artifacts are hidden at once and reclaimed after a delay. A member who moves a converted document finds its images, tables and structure refused at both the old and the new path until the document is converted again. A copy starts with none and the source keeps its own. A document restored from the trash has none. A project removed and restored within its grace period keeps them. A pull, checkout, merge or revert discards the artifacts of the documents whose content changed and keeps the others. The artifact bytes are charged to the session write allowance, never to the project storage. Two other kinds of metadata of a document, its chunk sets and its search entries, already follow a move and are removed with a deletion.

### 2.1 What users can do today
- Convert a document and read its artifacts; any later move, copy, deletion or restore loses them (14.1 rows 1 to 6).
- Move, copy, delete, restore and purge files and folders (14.1 rows 2 to 6).

### 2.2 Specs governing this area
- SPEC-0019 (document artifacts): FR-NEW-011 (discard on any change) and FR-NEW-012 (a copy starts with none) are modified by this increment (FR-MOD-001, FR-MOD-002); FR-NEW-013 (session charge) is kept. That spec is not edited.
- SPEC-0020 DEC-008: artifacts are session weight only; kept.
- SPEC-0101 FR-NEW-025 and SPEC-0100 FR-NEW-005 and FR-NEW-006: chunk sets and search jobs follow a move; this increment mirrors that for artifacts.

### 2.3 What is verified today
Each operation has a SPEC-0019 test asserting the discard (move, copy, copy over, trash and restore, folder move, archive, edit, pull, checkout, project restore). Seven groups of them change expectation here (14.1 row 9, 9.3).

## 3. Scope
### 3.1 In scope
- Move and rename of files and folders; copy of files and folders; overwrite by move or copy; trash and restore of files and folders; hard delete and project removal.
- The session charge of a copy and its refusal.

### 3.2 Out of scope, and why
- Version control operations: git has no notion of a move; a pull, checkout, merge or revert keeps the artifacts of unchanged documents and discards those of changed ones as before (SPEC-0019), which is correct.
- Project export: the archive holds files, and artifacts are not files.
- Edit, replace, upload and archive extraction over a document: the bytes change, so the artifacts are discarded as before.
- Charging artifacts to the project storage: unchanged (SPEC-0020 DEC-008).
- Copy across projects: does not exist today.

### 3.3 Constraints imposed by the context
- The session write allowance rule of SPEC-0019 FR-NEW-013 stays: artifact weight is charged to the session, not to the project.

## 4. Actors
- **Project member**: moves, copies, deletes and restores documents.
- **The platform**: trashes stale files by retention and removes projects past their grace period.
- **Non member**: refused.

## 4.5 Bounded contexts
| Context | What it owns | Key concepts |
|---|---|---|
| Documents | a converted document's artifacts and conversion text | image, table, heading, conversion text, artifacts |
| Files | paths, folders, trash, copies | move, copy, overwrite, trash, restore, purge |

## 5. Usage scenarios
### SC-001: Move or rename a converted document
**Actor:** a member
**Preconditions:** `/docs/report.pdf` converted
**Flow:**
1. Alice moves or renames it → the document is at the new path.
2. She reads its images, tables, structure or view there → the same artifacts, no conversion.
**Postconditions:** artifacts at the new path, none at the old path
**Exceptions:**
- EXC-001a: the source does not exist → refused.
- EXC-001b: the destination exists without overwrite → refused, nothing changes.

### SC-002: Move a folder of converted documents
**Actor:** a member
**Preconditions:** converted documents under a folder
**Flow:**
1. Alice moves or renames the folder → every document is at its new path.
**Postconditions:** every document keeps its artifacts
**Exceptions:**
- EXC-002a: a folder moved into itself → refused, nothing changes.

### SC-003: Copy a converted document or folder
**Actor:** a member
**Preconditions:** converted documents; a session write allowance
**Flow:**
1. Alice copies → each copy has its own artifacts, charged to the session allowance.
**Postconditions:** source and copy independent
**Exceptions:**
- EXC-003a: the allowance cannot hold the artifacts → the copy is refused whole.

### SC-004: Overwrite a converted document
**Actor:** a member
**Preconditions:** two converted documents
**Flow:**
1. Alice moves or copies one onto the other with overwrite → the destination has the incoming artifacts.
**Postconditions:** one set at the destination
**Exceptions:**
- EXC-004a: no overwrite requested → refused.

### SC-005: Delete and restore a converted document
**Actor:** a member
**Preconditions:** a converted document
**Flow:**
1. Alice deletes it → it goes to the trash with its artifacts.
2. She restores it → the artifacts are at the restored path, renamed if the path is taken.
**Postconditions:** artifacts back with the document
**Exceptions:**
- EXC-005a: restore of an unknown item → refused.
- EXC-005b: the restore cannot be made → the item stays in the trash.

### SC-006: Remove for good
**Actor:** a member or the platform
**Preconditions:** a converted document and a server that allows hard delete, or a removed project past its grace period
**Flow:**
1. Alice hard deletes it, or the platform removes the project → the document and its artifacts are removed.
**Postconditions:** nothing of it remains
**Exceptions:**
- EXC-006a: hard delete not allowed by the server → refused, nothing changes.

### SC-007: Operations that still discard
**Actor:** a member
**Preconditions:** a converted document
**Flow:**
1. Alice replaces its bytes or overwrites it with a plain file → its artifacts go away as before.
**Postconditions:** no artifacts
**Exceptions:**
- EXC-007a: a plain file moved or copied → no artifacts are created.

### SC-008: Refused operations
**Actor:** a member or a non member
**Preconditions:** converted documents
**Flow:**
1. An operation is refused (quota, rule, access) → every artifact is as before.
**Postconditions:** nothing changed
**Exceptions:**
- as listed in the tests

## 6. Functional requirements
### 6.1 New
#### FR-NEW-001 [EARS-E]: A moved or renamed document keeps its artifacts
> WHEN a member moves or renames a converted document THE Documents context SHALL keep its artifacts and its conversion text at the new path and none at the old path.

- **Business rules:** The artifacts are the images (with captions, pages and OCR text), the tables, the headings and the conversion text. The image ids, the table ids and every caption are unchanged. Reading the old path is refused as for any path that is not a file.
- **User visible text:** `ERR_NOT_FOUND: not a file: <old path>`
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-002 [EARS-E]: A moved or renamed folder keeps the artifacts of its documents
> WHEN a member moves or renames a folder THE Documents context SHALL keep the artifacts of every converted document in it, at any depth, at the new path of each.

- **Business rules:** Documents without artifacts stay without. An empty folder or a folder without converted documents moves as before.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-003 [EARS-E]: A copied document gets its own artifacts
> WHEN a member copies a converted document THE Documents context SHALL give the copy its own artifacts, equal to those of the source, AND SHALL leave the artifacts of the source unchanged.

- **Business rules:** After the copy the two sets are independent: editing, replacing or converting one again does not change the other. The weight of the copied artifacts is what the conversion of that document was charged (SPEC-0019 FR-NEW-013 as extended by SPEC-0103 FR-NEW-022): the stored pictures, the table Markdown and CSV text, the captions, the OCR text, the heading titles and the conversion text. It is charged to the session write allowance on top of the copied file's own size for a single file; the project storage is not charged for artifacts.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-004 [EARS-E]: A copied folder gives each copied document its own artifacts
> WHEN a member copies a folder THE Documents context SHALL give every copied converted document its own artifacts under FR-NEW-003, charging the weight of the copied artifacts to the session write allowance in one amount; the folder's files charge nothing to the session, as today.

- **Business rules:** The sources keep theirs.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-005 [EARS-O]: A copy that does not fit is refused whole
> IF the weight of the artifacts to copy, added to the files, exceeds what remains of the session write allowance THEN THE Documents context SHALL refuse the copy and keep nothing of it, not the files nor the artifacts.

- **Business rules:** For a single file the weight compared is the file's size plus its artifacts; for a folder it is the weight of the artifacts alone. A weight equal to what remains fits. The refusal is the existing one and the allowance is unchanged.
- **User visible text:** `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded`
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-006 [EARS-E]: An overwritten document takes the artifacts of what replaces it
> WHEN a member moves or copies a document onto an existing document with overwrite THE Documents context SHALL replace the destination's artifacts by those of the incoming document, AND the destination SHALL have no artifacts when the incoming document has none.

- **Business rules:** A folder copied onto an existing folder with overwrite replaces the artifacts of each document that has a counterpart in the source and leaves the artifacts of the destination's other documents unchanged. A copy of a document onto itself with overwrite leaves its artifacts unchanged and is charged as a copy; a folder copied into itself copies each document once. Without overwrite the existing refusal applies (`ERR_NO_CLOBBER: '<destination>' exists (pass overwrite=true)`) and both sets stay as they were.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-007 [EARS-E]: A trashed document keeps its artifacts
> WHEN a member deletes a converted document or a folder holding converted documents THE Documents context SHALL keep their artifacts with the item in the trash.

- **Business rules:** A document trashed by the automatic retention sweep keeps its artifacts like one deleted by a member. While the item is in the trash its original path reads as a path that is not a file, and its artifacts are readable only after the restore. A different file written at the original path has no artifacts of the trashed one.
- **User visible text:** `ERR_NOT_FOUND: not a file: <path>`, `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-008 [EARS-E]: A restored document gets its artifacts back
> WHEN a member restores a trashed document or folder THE Documents context SHALL give the restored documents their artifacts at the path where each is restored.

- **Business rules:** When the original path is taken the existing rename applies (`_restored`, then `_restored2` and so on) and the artifacts follow the new name.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-009 [EARS-E]: A purged document loses its artifacts
> WHEN a converted document is hard deleted, or its project is removed for good, THE Documents context SHALL remove its artifacts.

- **Business rules:** Hard delete exists only when the server allows it; without it the existing refusal applies and nothing changes. A trashed item is never removed automatically. After the removal a new file at the same path has no artifacts. A project removed and restored within its grace period keeps its artifacts (unchanged behavior).
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-010 [EARS-UB]: No partial result
> A refused or failed move, copy or restore SHALL NOT change the artifacts of any document, those of the source, of the destination and of the trash item.

- **Business rules:** This holds for every refusal of these operations, for example a missing source (`'<source>' does not exist`), a destination that exists (`ERR_NO_CLOBBER: '<destination>' exists (pass overwrite=true)`), a destination under a plain file (`ERR_NO_CLOBBER: '<parent>' already exists and is not a directory`), a move onto or into itself, a quota refusal, a non member, or a document changed during its conversion. Missing parent folders are created, as today.
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-011 [EARS-UB]: No artifacts out of nothing
> A move, copy, trash or restore of a file that has no artifacts SHALL NOT give it or its copy any artifacts.

- **Business rules:** Plain files and documents never converted behave as before.
- **Scenario:** SC-007
- **Priority:** Must

#### FR-NEW-012 [EARS-UB]: No new conversion, nothing altered
> A move, copy, trash or restore SHALL NOT call the document converter, AND SHALL NOT alter the content of the artifacts it keeps.

- **Business rules:** A move, trash or restore charges nothing to the session write allowance. Chunk sets and search entries behave as before this increment; the audit log keeps its entries as before (one for a move, a copy and a delete, none for a restore).
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-013 [EARS-O]: A move onto or into itself is refused
> IF the destination of a move is the source, inside the source, or an ancestor of the source THEN THE Files context SHALL refuse the move and change nothing.

- **Business rules:** Today the destination is deleted first when overwrite is given, so such a move destroys the document and its artifacts. The check applies with or without overwrite, after the check that the source exists (`'<source>' does not exist`) and before the check that the destination exists; it replaces the no-clobber refusal given today for a move onto itself or onto an ancestor without overwrite. The refusal comes before anything is deleted.
- **User visible text:** `ERR_INVALID_ARGUMENT: '<source>' cannot be moved onto or into itself`
- **Scenario:** SC-002
- **Priority:** Must

### 6.2 Modified
#### FR-MOD-001 [EARS-E]: Move, rename, trash no longer discard (modifies SPEC-0019/FR-NEW-011)
> WHEN a converted document is replaced, edited, overwritten by a plain file, extracted over by an archive or deleted for good THE Documents context SHALL discard its artifacts and its conversion text, AND SHALL keep them when it is moved, renamed or trashed.

- **Before and after:** Before: any change including a move, a rename, a deletion to the trash made the artifacts go away. After: only a change of the bytes, an overwrite or a permanent deletion does. Git pull, checkout, merge and revert keep their rule (only documents whose content differs).
- **Reason:** a member must not convert again after tidying files.
- **Scenario:** SC-007

#### FR-MOD-002 [EARS-E]: A copy starts with the artifacts of its source (modifies SPEC-0019/FR-NEW-012)
> WHEN a converted document is copied THE Documents context SHALL give the copy its own artifacts under FR-NEW-003.

- **Before and after:** Before: a copy started with none and the source kept its own. After: the copy has its own equal set.
- **Reason:** a member must not convert again after tidying files.
- **Scenario:** SC-003

### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
Moving, copying, trashing or restoring a folder of 100 converted documents completes within 10 seconds for the member.
### 7.2 Security and access
Only members move, copy, delete or restore, and only members read the artifacts afterwards (existing rule).
### 7.3 Privacy and compliance
Artifacts are project content; they stay with the document and are removed with it for good.
### 7.4 Usability and accessibility
Not applicable: no screen is added.
### 7.5 Reliability
An operation either carries the artifacts with the document or fails whole (FR-NEW-010). A read during an operation sees the artifacts at the old path or at the new one, never at neither once the operation has answered.
### 7.6 Operability
The audit log keeps its entries as before (one for a move, a copy and a delete, none for a restore) and gets none for the artifacts; refused operations leave no entry.

## 8. Edge cases
| Category | Case | Expected behavior | Test |
|---|---|---|---|
| Empty states | empty folder; plain file; no artifacts | nothing created | E2E-032, E2E-031 |
| Boundary values | allowance exactly equal; 100 documents | fits; all follow | E2E-059, E2E-038 |
| Concurrent actions | move during a conversion | refused conversion | E2E-020 |
| Slow or lost connectivity | not applicable: no network call | | |
| Access states | non member; second member | refused; allowed | E2E-017, E2E-029 |
| Data variations | accents, spaces, emoji in names | follow | E2E-037 |
| Navigation | move back; move twice; repeat | follow; refused | E2E-035, E2E-039, E2E-027 |
| Partial operations | refused copy, restore, overwrite | nothing changed | E2E-018, E2E-019, E2E-021, E2E-022 |
| Dependency failures | not applicable: no converter call | | |
| Ordering and timing | restore after path reuse; retention sweep trash | renamed; kept | E2E-043, E2E-045 |
| Exhaustion | session allowance | refused whole | E2E-018, E2E-019 |

## 9. Acceptance tests (the contract)
> Behavioral and technology agnostic. Convention: **document D** is `/docs/report.pdf` in project `proj`, converted by a converter that returned 2 images (`image-1` caption `Figure 1`, OCR text `Revenue 4.2 M EUR`, and `image-2`), 1 table (`table-1` captioned `Costs`) and the headings `Report` and `Costs`; the weight of D's artifacts is 3 000 bytes. **D2** is `/docs/sub/other.pdf` converted with 1 image captioned `Chart` and 1 000 bytes of artifacts. "Reads the artifacts" means the image list, the table list, the structure and the view. "The allowance" is the session write allowance; the allowance left is observed by a write of exactly that many bytes succeeding and one byte more being refused for quota; a test that observes it several times computes each expected remainder from the starting value minus its earlier observation writes, the probe being a write to a scratch file.

### 9.1 Summary
| Test | Action | Category | Scenario | Requirements | Priority |
|---|---|---|---|---|---|
| E2E-001 | New | requirement | SC-001 | FR-NEW-001, FR-MOD-001 | Critical |
| E2E-002 | New | requirement | SC-001 | FR-NEW-001 | High |
| E2E-003 | New | requirement | SC-002 | FR-NEW-002, FR-MOD-001 | High |
| E2E-004 | New | requirement | SC-001 | FR-NEW-012, FR-NEW-001 | High |
| E2E-005 | New | requirement | SC-003 | FR-NEW-003, FR-MOD-002 | Critical |
| E2E-006 | New | requirement | SC-003 | FR-NEW-004 | High |
| E2E-007 | New | requirement | SC-005 | FR-NEW-007, FR-NEW-008 | Critical |
| E2E-008 | New | requirement | SC-005 | FR-NEW-008 | High |
| E2E-009 | New | requirement | SC-004 | FR-NEW-006 | High |
| E2E-010 | New | requirement | SC-004 | FR-NEW-006 | High |
| E2E-011 | New | requirement | SC-006 | FR-NEW-009 | High |
| E2E-012 | New | requirement | SC-003 | FR-NEW-003, FR-MOD-002 | High |
| E2E-013 | New | failure | SC-008 | FR-NEW-010 | High |
| E2E-014 | New | failure | SC-008 | FR-NEW-010 | Medium |
| E2E-015 | New | failure | SC-002 | FR-NEW-013, FR-NEW-010 | Medium |
| E2E-016 | New | failure | SC-005 | FR-NEW-008 | Medium |
| E2E-017 | New | failure | SC-008 | FR-NEW-010 | High |
| E2E-018 | New | failure | SC-003 | FR-NEW-005, FR-NEW-010 | Critical |
| E2E-019 | New | failure | SC-003 | FR-NEW-004, FR-NEW-005, FR-NEW-010 | High |
| E2E-020 | New | failure | SC-008 | FR-NEW-010 | Medium |
| E2E-021 | New | failure | SC-005 | FR-NEW-008, FR-NEW-010 | Medium |
| E2E-022 | New | failure | SC-004 | FR-NEW-010, FR-NEW-006 | Medium |
| E2E-023 | New | failure | SC-005 | FR-NEW-007 | Medium |
| E2E-024 | New | edge | SC-003 | FR-NEW-003 | Medium |
| E2E-025 | New | failure | SC-005 | FR-NEW-008, FR-NEW-010 | Medium |
| E2E-026 | New | failure | SC-008 | FR-NEW-010 | Medium |
| E2E-027 | New | failure | SC-001 | FR-NEW-001, FR-NEW-010 | Medium |
| E2E-028 | New | failure | SC-005 | FR-NEW-007 | Medium |
| E2E-029 | New | edge | SC-001 | FR-NEW-001 | Medium |
| E2E-030 | New | failure | SC-008 | FR-NEW-010 | Low |
| E2E-031 | New | edge | SC-007 | FR-NEW-011 | High |
| E2E-032 | New | edge | SC-002 | FR-NEW-002, FR-NEW-011 | Low |
| E2E-033 | New | edge | SC-002 | FR-NEW-002 | High |
| E2E-034 | New | edge | SC-002 | FR-NEW-002, FR-NEW-011 | Medium |
| E2E-035 | New | edge | SC-001 | FR-NEW-001 | Medium |
| E2E-036 | New | edge | SC-004 | FR-NEW-006, FR-NEW-011 | High |
| E2E-037 | New | edge | SC-001 | FR-NEW-001, FR-NEW-003 | Medium |
| E2E-038 | New | edge | SC-002 | FR-NEW-002 | Medium |
| E2E-039 | New | edge | SC-001 | FR-NEW-001 | Medium |
| E2E-040 | New | requirement | SC-007 | FR-MOD-001 | High |
| E2E-041 | New | edge | SC-003 | FR-NEW-003 | Medium |
| E2E-042 | New | edge | SC-005 | FR-NEW-007, FR-NEW-011 | High |
| E2E-043 | New | edge | SC-005 | FR-NEW-008 | High |
| E2E-044 | New | edge | SC-005 | FR-NEW-007, FR-NEW-008 | High |
| E2E-045 | New | edge | SC-005 | FR-NEW-007, FR-NEW-008 | Medium |
| E2E-046 | New | edge | SC-007 | FR-MOD-001 | Medium |
| E2E-047 | New | edge | SC-007 | FR-NEW-012 | Medium |
| E2E-048 | New | edge | SC-001 | FR-NEW-012 | Medium |
| E2E-049 | New | edge | SC-001 | FR-NEW-012 | Medium |
| E2E-050 | New | edge | SC-003 | FR-NEW-003 | Low |
| E2E-051 | New | side effect | SC-003 | FR-NEW-003, FR-NEW-001, FR-NEW-012 | High |
| E2E-052 | New | side effect | SC-003 | FR-NEW-003 | High |
| E2E-053 | New | side effect | SC-005 | FR-NEW-007 | Medium |
| E2E-054 | New | side effect | SC-001 | FR-NEW-012 | High |
| E2E-055 | New | side effect | SC-001 | FR-NEW-012 | Low |
| E2E-056 | New | side effect | SC-008 | FR-NEW-010 | Medium |
| E2E-057 | New | side effect | SC-003 | FR-NEW-004 | Medium |
| E2E-058 | New | edge | SC-006 | FR-NEW-009 | Medium |
| E2E-059 | New | edge | SC-003 | FR-NEW-005 | Medium |
| E2E-060 | New | failure | SC-001 | FR-NEW-013, FR-NEW-010 | High |
| E2E-061 | New | failure | SC-002 | FR-NEW-013, FR-NEW-010 | High |
| E2E-062 | New | edge | SC-004 | FR-NEW-006, FR-NEW-004 | Medium |
| E2E-063 | New | failure | SC-006 | FR-NEW-009, FR-NEW-010 | Medium |
| E2E-064 | New | edge | SC-005 | FR-NEW-007 | Medium |
| E2E-065 | New | failure | SC-001 | FR-NEW-013, FR-NEW-010 | High |
| E2E-066 | New | failure | SC-002 | FR-NEW-013, FR-NEW-010 | High |
| E2E-067 | New | edge | SC-004 | FR-NEW-006, FR-NEW-003 | Medium |
| E2E-068 | New | edge | SC-003 | FR-NEW-004, FR-NEW-006 | Medium |
| E2E-069 | New | failure | SC-004 | FR-NEW-006, FR-NEW-010 | High |
| E2E-070 | New | failure | SC-007 | FR-MOD-001, FR-NEW-010 | Medium |
| E2E-071 | New | failure | SC-001 | FR-NEW-001, FR-NEW-010 | Medium |
| E2E-MOD-005 | Modified | requirement | SC-004 | FR-MOD-002 | High |
| E2E-MOD-006 | Modified | requirement | SC-001 | FR-MOD-001 | High |
| E2E-MOD-007 | Modified | requirement | SC-003 | FR-MOD-002 | High |
| E2E-MOD-001 | Modified | requirement | SC-001 | FR-MOD-001 | High |
| E2E-MOD-002 | Modified | requirement | SC-003 | FR-MOD-002 | High |
| E2E-MOD-003 | Modified | requirement | SC-005 | FR-MOD-001 | High |
| E2E-MOD-004 | Modified | requirement | SC-002 | FR-MOD-001 | High |

Coverage by category: requirement 13, failure 24, edge 27, side effect 7, modified 7. Happy 20 (the requirement tests and the 7 modified ones), failure 24: happy to failure ratio 1:1.20 (beats 1:1)

### 9.2 New tests
#### E2E-001: a converted document is moved
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-MOD-001
- **Given** project `proj`, Alice (`alice@example.com`) a member, D converted
- **When** Alice moves `/docs/report.pdf` to `/archive/report.pdf`
- **Then** reading `/archive/report.pdf` gives the 2 images (`image-1` caption `Figure 1`, OCR text `Revenue 4.2 M EUR`), the table `table-1` captioned `Costs`, the headings `Report` and `Costs` and the full view, all as before the move
- **And** reading the images of `/docs/report.pdf` is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **And** the document was not converted again
- **Priority:** Critical

#### E2E-002: a converted document is renamed in place
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** D converted
- **When** Alice renames `/docs/report.pdf` to `/docs/report-2026.pdf`
- **Then** `/docs/report-2026.pdf` gives the same artifacts as before, and the old name is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **Priority:** High

#### E2E-003: a folder holding converted documents is moved
- **Category:** requirement
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002, FR-MOD-001
- **Given** `/docs/report.pdf` (D) and `/docs/sub/other.pdf` (D2, one image captioned `Chart`) both converted
- **When** Alice moves `/docs` to `/library`
- **Then** `/library/report.pdf` and `/library/sub/other.pdf` give the artifacts they had, and nothing is readable under `/docs`
- **Priority:** High

#### E2E-004: the conversion text follows without a converter call
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012, FR-NEW-001
- **Given** D converted through a converter that counts its calls (1 call so far)
- **When** Alice moves D, then reads its extracted text and its view
- **Then** both answers equal those given before the move, and the converter was called 1 time in total
- **Priority:** High

#### E2E-005: a converted document is copied
- **Category:** requirement
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003, FR-MOD-002
- **Given** D converted; the session write allowance has 100 000 bytes left
- **When** Alice copies `/docs/report.pdf` to `/docs/report-copy.pdf`
- **Then** `/docs/report-copy.pdf` gives the same 2 images, 1 table and 2 headings as D, and D still gives its own
- **And** the allowance left is lower by the file's own size plus the 3 000 bytes of D's artifacts
- **Priority:** Critical

#### E2E-006: a folder holding converted documents is copied
- **Category:** requirement
- **Scenario:** SC-003
- **Requirements:** FR-NEW-004
- **Given** `/docs/report.pdf` (D) and `/docs/sub/other.pdf` (D2) converted; the allowance left is 100 000 bytes
- **When** Alice copies `/docs` to `/docs-copy`
- **Then** both copies give their own artifacts and both sources keep theirs
- **Priority:** High

#### E2E-007: trash then restore
- **Category:** requirement
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** D converted
- **When** Alice deletes `/docs/report.pdf` (it goes to the trash) then restores it from the trash
- **Then** `/docs/report.pdf` gives its images, table, headings and view as before the deletion
- **And** between the deletion and the restore, reading the images of `/docs/report.pdf` is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **Priority:** Critical

#### E2E-008: restore under a new name after a collision
- **Category:** requirement
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008
- **Given** D converted; Alice deleted it; she then created a plain file `/docs/report.pdf` with the text `new`
- **When** Alice restores the trashed document
- **Then** it comes back as `/docs/report.pdf_restored` and gives D's artifacts; the plain file `/docs/report.pdf` still reads `new` and has no artifacts
- **Priority:** High

#### E2E-009: overwrite by move
- **Category:** requirement
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** D converted (2 images) and `/docs/old.pdf` converted with 1 image captioned `Old`
- **When** Alice moves `/docs/report.pdf` onto `/docs/old.pdf` with overwrite
- **Then** `/docs/old.pdf` gives D's 2 images and no trace of `Old`; `/docs/report.pdf` is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **Priority:** High

#### E2E-010: overwrite by copy
- **Category:** requirement
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** D converted and `/docs/old.pdf` converted with 1 image captioned `Old`
- **When** Alice copies `/docs/report.pdf` onto `/docs/old.pdf` with overwrite
- **Then** `/docs/old.pdf` gives D's artifacts, none of `Old`; D still gives its own
- **Priority:** High

#### E2E-011: a hard deleted document loses its artifacts
- **Category:** requirement
- **Scenario:** SC-006
- **Requirements:** FR-NEW-009
- **Given** D converted; the server allows hard delete
- **When** Alice hard deletes `/docs/report.pdf`, then writes a new plain file at `/docs/report.pdf`
- **Then** the trash does not list D, and reading the images of the new file is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Priority:** High

#### E2E-012: the source of a copy keeps its artifacts
- **Category:** requirement
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003, FR-MOD-002
- **Given** D converted and copied to `/docs/c.pdf`
- **When** Alice edits `/docs/c.pdf`, then reads the artifacts of both
- **Then** `/docs/c.pdf` has no artifacts (the edit discards them as before) and D still gives all of its own
- **Priority:** High

#### E2E-013: a move onto an existing folder without overwrite
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D converted; `/archive` is an existing folder
- **When** Alice moves `/docs/report.pdf` onto `/archive` without overwrite
- **Then** the move is refused with `ERR_NO_CLOBBER: '/archive' exists (pass overwrite=true)`; D gives its artifacts at `/docs/report.pdf`
- **Priority:** High

#### E2E-014: a move of a document that does not exist
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** no file `/docs/ghost.pdf`
- **When** Alice moves `/docs/ghost.pdf` to `/archive/ghost.pdf`
- **Then** the move is refused with `ERR_NOT_FOUND: '/docs/ghost.pdf' does not exist` and no artifact appears at `/archive/ghost.pdf`
- **Priority:** Medium

#### E2E-015: a folder moved into itself
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** `/docs` holds D converted
- **When** Alice moves `/docs` to `/docs/inner`
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs' cannot be moved onto or into itself`, and D still gives its artifacts at `/docs/report.pdf`
- **Priority:** Medium

#### E2E-016: restore of an unknown trash item
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008
- **Given** a trash holding no item at `/.mcp_trash/missing`
- **When** Alice restores the trash item `/.mcp_trash/missing`
- **Then** the restore is refused with `ERR_NOT_FOUND: '/.mcp_trash/missing' is not a trashed entry`
- **Priority:** Medium

#### E2E-017: a non member cannot move, copy or restore
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D converted and a trashed converted document; Eve (`eve@example.com`) is not a member of `proj`
- **When** Eve moves, copies and restores them
- **Then** each is refused with `ERR_FORBIDDEN: 'eve@example.com' is not a member of 'proj'`
- **And** every artifact is unchanged for Alice
- **Priority:** High

#### E2E-018: a copy that does not fit the session allowance
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** D converted (file of 600 bytes, artifacts of 3 000 bytes); the allowance left is 3 500 bytes
- **When** Alice copies `/docs/report.pdf` to `/docs/c.pdf`
- **Then** the copy is refused with `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded`, `/docs/c.pdf` does not exist
- **And** D still gives its artifacts
- **And** the allowance left is still 3 500 bytes
- **Priority:** Critical

#### E2E-019: a folder copy where the last document's artifacts do not fit
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-004, FR-NEW-005, FR-NEW-010
- **Given** `/docs` holds D (artifacts 3 000 bytes) and D2 (artifacts 1 000 bytes) converted; the allowance left is 3 500 bytes
- **When** Alice copies `/docs` to `/docs-copy`
- **Then** the copy is refused with `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded` and `/docs-copy` does not exist at all
- **And** the sources keep their artifacts
- **Priority:** High

#### E2E-020: a move while the document is being converted
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D is being converted: the converter has not answered yet
- **When** Alice moves `/docs/report.pdf` to `/archive/report.pdf` and the converter then answers
- **Then** the conversion is refused with the existing refusal for a document changed during its conversion; if D already had artifacts before, they are found at `/archive/report.pdf`; otherwise no artifacts exist at either path
- **Priority:** Medium

#### E2E-021: restoring the same item twice
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008, FR-NEW-010
- **Given** D trashed and restored once
- **When** Alice restores the same trash item again
- **Then** the second restore is refused with `ERR_NOT_FOUND: '<trash path of D>' is not a trashed entry`, and D still gives its artifacts at the restored path
- **Priority:** Medium

#### E2E-022: a refused move onto an existing document keeps both sets
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-006
- **Given** D and `/docs/old.pdf` converted
- **When** Alice moves D onto `/docs/old.pdf` without overwrite
- **Then** the move is refused with `ERR_NO_CLOBBER: '/docs/old.pdf' exists (pass overwrite=true)`; both documents still give their own artifacts
- **Priority:** Medium

#### E2E-023: reading the old path after a trash
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** D trashed
- **When** Alice reads the images, tables, structure and view of `/docs/report.pdf`
- **Then** each is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **And** the artifacts of the trashed item are readable only after its restore: reading them at the trash location is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`
- **Priority:** Medium

#### E2E-024: a copy into a folder that does not exist yet
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** D converted and no folder `/nowhere`
- **When** Alice copies D to `/nowhere/c.pdf`
- **Then** the copy succeeds, `/nowhere` is created and `/nowhere/c.pdf` gives D's artifacts, charged as in E2E-005
- **Priority:** Medium

#### E2E-025: a restore of a document whose folder was removed
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008, FR-NEW-010
- **Given** D trashed, then `/docs` replaced by a plain file named `/docs`
- **When** Alice restores D
- **Then** the restore is refused with `ERR_NO_CLOBBER: '/docs' already exists and is not a directory`, and D stays in the trash with its artifacts
- **Priority:** Medium

#### E2E-026: a move under a plain file
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D converted and a plain file `/flat.txt`
- **When** Alice moves D to `/flat.txt/report.pdf`
- **Then** the move is refused with `ERR_NO_CLOBBER: '/flat.txt' already exists and is not a directory`; D keeps its artifacts
- **Priority:** Medium

#### E2E-027: the same move twice
- **Category:** failure
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-010
- **Given** D converted and moved to `/archive/report.pdf`
- **When** Alice repeats the move of `/docs/report.pdf`
- **Then** she is refused with the missing source refusal and `/archive/report.pdf` still gives D's artifacts
- **Priority:** Medium

#### E2E-028: a non member cannot read the artifacts of a trashed document
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** D trashed; Eve is not a member
- **When** Eve reads the images of `/docs/report.pdf`
- **Then** she is refused with `ERR_FORBIDDEN: 'eve@example.com' is not a member of 'proj'`
- **Priority:** Medium

#### E2E-029: a second member moves a converted document
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** D converted by Alice; Bob (`bob@example.com`) is a member of `proj`
- **When** Bob moves D to `/archive/report.pdf`
- **Then** Alice and Bob both read D's artifacts at `/archive/report.pdf`
- **Priority:** Medium

#### E2E-030: an unknown project
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** no project `ghost`
- **When** Alice moves a file in `ghost`
- **Then** she is refused with `ERR_PROJECT_NOT_FOUND: project 'ghost' not found`
- **Priority:** Low

#### E2E-031: a plain file moved and copied
- **Category:** edge
- **Scenario:** SC-007
- **Requirements:** FR-NEW-011
- **Given** `/notes.txt` never converted
- **When** Alice moves it to `/n.txt` and copies it to `/m.txt`
- **Then** reading the images of either is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`; no artifact was created
- **Priority:** High

#### E2E-032: an empty folder moved
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002, FR-NEW-011
- **Given** an empty folder `/empty`
- **When** Alice moves it to `/vide`
- **Then** the move succeeds and nothing is created
- **Priority:** Low

#### E2E-033: nested folders with three converted documents
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** `/a/x.pdf`, `/a/b/y.pdf`, `/a/b/c/z.pdf` converted with 1, 2 and 3 images
- **When** Alice moves `/a` to `/z`
- **Then** `/z/x.pdf`, `/z/b/y.pdf` and `/z/b/c/z.pdf` give 1, 2 and 3 images with their captions
- **Priority:** High

#### E2E-034: a folder mixing converted and plain files
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002, FR-NEW-011
- **Given** `/mix/a.pdf` converted and `/mix/b.txt` plain
- **When** Alice moves `/mix` to `/mixed`
- **Then** `/mixed/a.pdf` gives its artifacts and `/mixed/b.txt` has none
- **Priority:** Medium

#### E2E-035: a document moved back to its original path
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** D converted and moved to `/archive/report.pdf`
- **When** Alice moves it back to `/docs/report.pdf`
- **Then** `/docs/report.pdf` gives D's artifacts
- **Priority:** Medium

#### E2E-036: a converted document overwritten by a plain file
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-011
- **Given** D converted and a plain file `/docs/plain.txt`
- **When** Alice moves `/docs/plain.txt` onto `/docs/report.pdf` with overwrite
- **Then** `/docs/report.pdf` has no artifacts (`ERR_NOT_FOUND: No artifacts: document not extracted`) and holds the plain text
- **Priority:** High

#### E2E-037: names with accents and spaces
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-003
- **Given** D converted
- **When** Alice moves it to `/Archives été/Rapport n° 1.pdf` and copies it to `/Archives été/Copie 🌐.pdf`
- **Then** both give D's artifacts
- **Priority:** Medium

#### E2E-038: a hundred converted documents in one folder
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** `/many` holds 100 converted documents, each with 1 image captioned `n<k>`
- **When** Alice moves `/many` to `/lots`
- **Then** all 100 give their own image caption, and the move completes within 10 seconds
- **Priority:** Medium

#### E2E-039: two moves in a row
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** D converted
- **When** Alice moves it to `/a.pdf` then to `/b.pdf`
- **Then** `/b.pdf` gives D's artifacts and `/a.pdf` and `/docs/report.pdf` are refused
- **Priority:** Medium

#### E2E-040: an edit after a move discards as before
- **Category:** requirement
- **Scenario:** SC-007
- **Requirements:** FR-MOD-001
- **Given** D converted and moved to `/archive/report.pdf`
- **When** Alice replaces its bytes with a new version
- **Then** the artifacts are gone (`ERR_NOT_FOUND: No artifacts: document not extracted`)
- **Priority:** High

#### E2E-041: a converted copy converts again on its own
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** D copied to `/c.pdf` and both converted again by a converter returning 1 image captioned `New`
- **When** Alice converts `/c.pdf` again
- **Then** `/c.pdf` gives only `New`; D still gives its own previous artifacts
- **Priority:** Medium

#### E2E-042: another file at the old path of a trashed document
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-011
- **Given** D trashed and a plain file created at `/docs/report.pdf`
- **When** Alice reads its images
- **Then** `ERR_NOT_FOUND: No artifacts: document not extracted`; the trashed D still returns its artifacts after a restore under another name
- **Priority:** High

#### E2E-043: restore after the original path was reused
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-008
- **Given** D trashed; a different converted document `/docs/report.pdf` created (1 image captioned `Other`)
- **When** Alice restores D
- **Then** D comes back as `/docs/report.pdf_restored` with its own 2 images; `/docs/report.pdf` still gives `Other`
- **Priority:** High

#### E2E-044: a folder trashed and restored
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** `/docs` with D and D2 converted
- **When** Alice deletes `/docs` and restores it
- **Then** both documents give their artifacts at their original paths
- **Priority:** High

#### E2E-045: a document trashed by the retention sweep keeps its artifacts
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** D converted and not read for longer than the file retention, so the automatic sweep moves it to the trash
- **When** Alice restores D from the trash
- **Then** D comes back at `/docs/report.pdf` with its artifacts
- **Priority:** Medium

#### E2E-046: a project soft deleted and restored
- **Category:** edge
- **Scenario:** SC-007
- **Requirements:** FR-MOD-001
- **Given** D converted; the platform administrator removes and restores the project
- **When** Alice reads D's images
- **Then** D gives its artifacts as before (existing behavior, unchanged)
- **Priority:** Medium

#### E2E-047: chunk sets and search entries behave as before
- **Category:** edge
- **Scenario:** SC-007
- **Requirements:** FR-NEW-012
- **Given** D converted with a chunk set and a search entry
- **When** Alice moves D
- **Then** the chunk set and the search entry are found at the new path as before this change
- **Priority:** Medium

#### E2E-048: the images keep their ids and the table its caption
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012
- **Given** D converted
- **When** Alice moves D
- **Then** the images are `image-1` and `image-2`, the table is `table-1` captioned `Costs`, the same ids and captions as before
- **Priority:** Medium

#### E2E-049: OCR text and headings follow
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012
- **Given** D converted
- **When** Alice moves and copies D
- **Then** the moved and the copied document give the OCR text `Revenue 4.2 M EUR` and the headings `Report` and `Costs`
- **Priority:** Medium

#### E2E-050: a second copy of a copy
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** D copied to `/c1.pdf`
- **When** Alice copies `/c1.pdf` to `/c2.pdf`
- **Then** `/c2.pdf` gives D's artifacts and the allowance is charged for each copy
- **Priority:** Low

#### E2E-051: a copy is charged to the allowance, a move is not
- **Category:** side effect
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003, FR-NEW-001, FR-NEW-012
- **Given** D converted; the allowance left noted
- **When** Alice moves D, then copies D
- **Then** the move leaves the allowance left unchanged and the copy lowers it by the file's size plus the 3 000 bytes of artifacts
- **Priority:** High

#### E2E-052: project storage is not charged for artifacts
- **Category:** side effect
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** D converted; the project's used storage noted
- **When** Alice copies D
- **Then** the used storage grows by the copied file's size only
- **Priority:** High

#### E2E-053: the trash listing is unchanged
- **Category:** side effect
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** D converted
- **When** Alice deletes D and lists the trash
- **Then** the listing shows the same fields as before this change (trash path, original path, size, kind, deletion time, deleted by, days to purge)
- **Priority:** Medium

#### E2E-054: no converter call on any of these operations
- **Category:** side effect
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012
- **Given** D converted through a converter that counts its calls (1 call)
- **When** Alice moves, copies, trashes and restores D
- **Then** the converter was called 1 time in total
- **Priority:** High

#### E2E-055: the audit log keeps its entries
- **Category:** side effect
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012
- **Given** D converted
- **When** Alice moves, copies, deletes and restores D
- **Then** the audit log has the same one entry for the move, the copy and the delete as before this change, none for the restore (as before) and none for the artifacts
- **Priority:** Low

#### E2E-056: a refused operation changes nothing the operator sees
- **Category:** side effect
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D converted
- **When** Alice's copy is refused for quota (E2E-018)
- **Then** the audit log has no entry for the copy and the used storage is unchanged
- **Priority:** Medium

#### E2E-057: a folder copy charges the artifacts of each document and keeps every source
- **Category:** side effect
- **Scenario:** SC-003
- **Requirements:** FR-NEW-004
- **Given** `/docs` holds D and D2 converted (artifacts 3 000 and 1 000 bytes); the allowance left noted
- **When** Alice copies `/docs` to `/docs-copy`
- **Then** the allowance left is lower by 4 000 bytes (the files themselves charge nothing for a folder copy, as before), and both sources still give their artifacts
- **Priority:** Medium

#### E2E-058: removing the project removes the artifacts
- **Category:** edge
- **Scenario:** SC-006
- **Requirements:** FR-NEW-009
- **Given** D converted; the platform administrator removes `proj` and its grace period passes with the removal purge
- **When** an administrator recreates a project `proj` and Alice writes `/docs/report.pdf`
- **Then** reading the images of the new file is refused with `ERR_NOT_FOUND: No artifacts: document not extracted`; nothing of D remains
- **Priority:** Medium

#### E2E-059: a copy that fits the allowance exactly
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-005
- **Given** D converted (file of 600 bytes, artifacts of 3 000 bytes); the allowance left is exactly 3 600 bytes
- **When** Alice copies D
- **Then** the copy succeeds with its artifacts, and a further write of 1 byte is refused for quota
- **Priority:** Medium

#### E2E-060: a document moved onto itself with overwrite
- **Category:** failure
- **Scenario:** SC-001
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** D converted
- **When** Alice moves `/docs/report.pdf` onto `/docs/report.pdf` with overwrite
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs/report.pdf' cannot be moved onto or into itself` and D still gives its artifacts
- **Priority:** High

#### E2E-061: a folder moved onto its ancestor with overwrite
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** `/docs/sub/other.pdf` (D2) converted
- **When** Alice moves `/docs/sub` onto `/docs` with overwrite
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs/sub' cannot be moved onto or into itself` and D2 still gives its artifacts
- **Priority:** High

#### E2E-062: a folder copied onto an existing folder with overwrite
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-004
- **Given** `/src` holds D converted and `/dst` holds `/dst/report.pdf` converted (1 image captioned `Old`) and `/dst/keep.pdf` converted (1 image captioned `Keep`)
- **When** Alice copies `/src` onto `/dst` with overwrite
- **Then** `/dst/report.pdf` gives D's artifacts and none of `Old`, `/dst/keep.pdf` still gives `Keep`
- **Priority:** Medium

#### E2E-063: a hard delete that the server refuses keeps everything
- **Category:** failure
- **Scenario:** SC-006
- **Requirements:** FR-NEW-009, FR-NEW-010
- **Given** D converted; the server does not allow hard delete
- **When** Alice hard deletes D
- **Then** she is refused with `ERR_NOT_SUPPORTED: hard delete disabled (server started without allow_hard_delete)` and D still gives its artifacts
- **Priority:** Medium

#### E2E-064: a stale file trashed by the retention sweep and a member's delete are alike
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-007
- **Given** D and D2 converted; D deleted by Alice, D2 trashed by the retention sweep
- **When** Alice restores both
- **Then** both come back with their artifacts
- **Priority:** Medium

#### E2E-065: a move onto itself without overwrite
- **Category:** failure
- **Scenario:** SC-001
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** D converted
- **When** Alice moves `/docs/report.pdf` onto `/docs/report.pdf` without overwrite
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs/report.pdf' cannot be moved onto or into itself` (not the no-clobber refusal) and D still gives its artifacts
- **Priority:** High

#### E2E-066: a folder moved onto its ancestor without overwrite
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** `/docs/sub/other.pdf` (D2) converted
- **When** Alice moves `/docs/sub` onto `/docs` without overwrite
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs/sub' cannot be moved onto or into itself` and D2 still gives its artifacts
- **Priority:** High

#### E2E-067: a file copied onto itself with overwrite
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-003
- **Given** D converted; the allowance left noted
- **When** Alice copies `/docs/report.pdf` onto `/docs/report.pdf` with overwrite
- **Then** D still gives all its artifacts and the allowance is charged as for a copy (file size plus 3 000 bytes)
- **Priority:** Medium

#### E2E-068: a folder copied into itself
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-004, FR-NEW-006
- **Given** `/docs` holds D converted
- **When** Alice copies `/docs` to `/docs/inner` with overwrite
- **Then** `/docs/inner/report.pdf` gives D's artifacts once, `/docs/report.pdf` still gives its own, and no document is copied twice
- **Priority:** Medium

#### E2E-069: a copy onto an existing document without overwrite
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-010
- **Given** D and `/docs/old.pdf` (1 image captioned `Old`) converted
- **When** Alice copies D onto `/docs/old.pdf` without overwrite
- **Then** the copy is refused with `ERR_NO_CLOBBER: '/docs/old.pdf' exists (pass overwrite=true)`; both documents keep their own artifacts and nothing is charged
- **Priority:** High

#### E2E-070: an edit refused for quota keeps the artifacts
- **Category:** failure
- **Scenario:** SC-007
- **Requirements:** FR-MOD-001, FR-NEW-010
- **Given** D converted; the allowance left is 10 bytes
- **When** Alice replaces D's bytes with a 1 000 byte file
- **Then** the write is refused with `ERR_WRITE_QUOTA_EXCEEDED: session write quota of <n> bytes exceeded` and D still gives its artifacts
- **Priority:** Medium

#### E2E-071: a rename under a plain file
- **Category:** failure
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-010
- **Given** D converted and a plain file `/flat.txt`
- **When** Alice renames D to `/flat.txt/report.pdf`
- **Then** the rename is refused with `ERR_NO_CLOBBER: '/flat.txt' already exists and is not a directory` and D still gives its artifacts
- **Priority:** Medium

### 9.3 Modified tests
#### E2E-MOD-005: a copy over a document replaces its artifacts (was SPEC-0019 test 093)
- **Validated before:** after D is copied over `/docs/old.pdf` with overwrite the artifacts are gone or the old rule applies.
- **Validates now:** after D is copied over `/docs/old.pdf` with overwrite the artifacts follow as FR-NEW-006 says (the destination takes the incoming artifacts).
- **Scenario:** SC-004
- **Requirements:** FR-MOD-002
- **Given** D converted
- **When** D is copied over `/docs/old.pdf` with overwrite by Alice
- **Then** the destination gives the artifacts of the incoming document and none of its own previous ones

#### E2E-MOD-006: moving away and back keeps the artifacts (was SPEC-0019 test 097)
- **Validated before:** after a converted document is moved away and back the artifacts are gone or the old rule applies.
- **Validates now:** after a converted document is moved away and back the artifacts follow as FR-NEW-001 says (a move away and a move back keep them).
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** D converted
- **When** A converted document is moved away and back by Alice
- **Then** after the second move the document gives its artifacts at the original path

#### E2E-MOD-007: a copy is charged the file and its artifacts (was SPEC-0019 test 099 and a_copy_is_charged_the_file_bytes_only)
- **Validated before:** after D is copied the artifacts are gone or the old rule applies.
- **Validates now:** after D is copied the artifacts follow as FR-NEW-003 says (the copy is charged the file and its artifacts).
- **Scenario:** SC-003
- **Requirements:** FR-MOD-002
- **Given** D converted
- **When** D is copied by Alice
- **Then** the allowance left is lower by the file size plus the weight of the artifacts (before: by the file size only)

#### E2E-MOD-001: a move no longer discards (was SPEC-0019 test 088)
- **Validated before:** after a converted document is moved the artifacts are gone or the old rule applies.
- **Validates now:** after a converted document is moved the artifacts follow as FR-NEW-001 says (they follow the document to the new path).
- **Scenario:** SC-001
- **Requirements:** FR-MOD-001
- **Given** D converted
- **When** A converted document is moved by Alice
- **Then** reading the artifacts at the destination gives those of before the operation (and at the old path is refused as not a file)

#### E2E-MOD-002: a copy no longer starts with none (was SPEC-0019 test 089)
- **Validated before:** after a converted document is copied the artifacts are gone or the old rule applies.
- **Validates now:** after a converted document is copied the artifacts follow as FR-NEW-003 says (the copy has its own equal set).
- **Scenario:** SC-003
- **Requirements:** FR-MOD-002
- **Given** D converted
- **When** A converted document is copied by Alice
- **Then** reading the artifacts at the copy gives those of the source, and the source still gives its own

#### E2E-MOD-003: trash and restore keep the artifacts (was SPEC-0019 test 090)
- **Validated before:** after a document is trashed and restored the artifacts are gone or the old rule applies.
- **Validates now:** after a document is trashed and restored the artifacts follow as FR-NEW-007 and FR-NEW-008 say (they stay with the trashed item and return on restore).
- **Scenario:** SC-005
- **Requirements:** FR-MOD-001
- **Given** D converted
- **When** A document is trashed and restored by Alice
- **Then** after the restore the artifacts are at the restored path; while in the trash the original path is refused as not a file

#### E2E-MOD-004: a folder move keeps the artifacts (was SPEC-0019 test 095)
- **Validated before:** after a folder of converted documents is moved the artifacts are gone or the old rule applies.
- **Validates now:** after a folder of converted documents is moved the artifacts follow as FR-NEW-002 says (every document of the folder keeps its artifacts).
- **Scenario:** SC-002
- **Requirements:** FR-MOD-001
- **Given** D converted
- **When** A folder of converted documents is moved by Alice
- **Then** reading the artifacts at the destination gives those of before the operation (and at the old path is refused as not a file)

### 9.4 Removed tests
None.

## 10. Traceability
| Scenario | Requirements | Happy | Failure | Edge |
|---|---|---|---|---|
| SC-001 | FR-MOD-001, FR-NEW-001, FR-NEW-003, FR-NEW-010, FR-NEW-012, FR-NEW-013 | E2E-001, E2E-002, E2E-004, E2E-MOD-006, E2E-MOD-001 | E2E-027, E2E-060, E2E-065, E2E-071 | E2E-029, E2E-035, E2E-037, E2E-039, E2E-048, E2E-049, E2E-054, E2E-055 |
| SC-002 | FR-MOD-001, FR-NEW-002, FR-NEW-010, FR-NEW-011, FR-NEW-013 | E2E-003, E2E-MOD-004 | E2E-015, E2E-061, E2E-066 | E2E-032, E2E-033, E2E-034, E2E-038 |
| SC-003 | FR-MOD-002, FR-NEW-001, FR-NEW-003, FR-NEW-004, FR-NEW-005, FR-NEW-006, FR-NEW-010, FR-NEW-012 | E2E-005, E2E-006, E2E-012, E2E-MOD-007, E2E-MOD-002 | E2E-018, E2E-019 | E2E-024, E2E-041, E2E-050, E2E-051, E2E-052, E2E-057, E2E-059, E2E-068 |
| SC-004 | FR-NEW-003, FR-NEW-004, FR-NEW-006, FR-NEW-010, FR-NEW-011 | E2E-009, E2E-010, E2E-MOD-005 | E2E-022, E2E-069 | E2E-036, E2E-062, E2E-067 |
| SC-005 | FR-NEW-007, FR-NEW-008, FR-NEW-010, FR-NEW-011 | E2E-007, E2E-008, E2E-MOD-003 | E2E-016, E2E-021, E2E-023, E2E-025, E2E-028 | E2E-042, E2E-043, E2E-044, E2E-045, E2E-053, E2E-064 |
| SC-006 | FR-NEW-009, FR-NEW-010 | E2E-011 | E2E-063 | E2E-058 |
| SC-007 | FR-MOD-001, FR-NEW-010, FR-NEW-011, FR-NEW-012 | E2E-040 | E2E-070 | E2E-031, E2E-046, E2E-047 |
| SC-008 | FR-NEW-010 |  | E2E-013, E2E-014, E2E-017, E2E-020, E2E-026, E2E-030 | E2E-056 |

SC-008 is made of refusals by design; its success side is the other scenarios (E2E-051 charges a successful move nothing).

## 11. Open questions
None.

## 12. Glossary (MANDATORY)
| Term | Definition | Context |
|---|---|---|
| Artifacts | the images, tables, headings and conversion text kept from a document's last conversion | Documents |
| Document D, D2 | the reference converted documents of the tests | tests |
| Allowance | the session write allowance | Files |
| Trash | where a deleted file or folder waits before a purge for good | Files |
| Restore | bringing a trashed item back, renamed when its path is taken | Files |
| Overwrite | replacing an existing destination on a move or a copy | Files |
| Removal for good | a hard delete (when the server allows it) or the removal of a project past its grace period | Files |
| Reads the artifacts | the image list, the table list, the structure and the view | tests |

## 13. Implementability gate (MANDATORY)
| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 6 | 7 | NOT-IMPLEMENTABLE |
| 2 | 4 | 7 | NOT-IMPLEMENTABLE |
| 3 | 0 | 7 | IMPLEMENTABLE |

**Amendments applied:** hard delete replaces the trash purge; folder copy charge; weight definition; FR-NEW-013 and its tests; restore not charged and not audited; missing parents created; refusal texts quoted from the code; copy onto itself rule; failure tests added; 14.3 inputs. **No implementation detail outside Section 14:** PASS (refusal texts are user visible)

## 14. Evidence appendix (for /sdlc-design and the auditor, not for the product owner)
### 14.1 Current State evidence
| # | Claim (Section 2) | Evidence |
|---|---|---|
| 1 | a set is tied to its path and to the node revision it was built from; the current set is hidden on a revision mismatch | `crates/core/src/storage/meta.rs:236-252,1277-1290` |
| 2 | every write, move and restore renews the revision; a move calls `tx_rename` | `storage/meta.rs:147,1881-1887,2351`; `core/fs_ops.rs:1322-1328` |
| 3 | soft delete renames into the trash with `carry_sets=false`; restore renames back and retries with `_restored` | `storage/meta.rs:2523-2538`; `tools/trash.rs:229-249` |
| 4 | a single file copy goes through `read_bytes`, `admit_file` and `write_bytes_atomic` (new revision, no artifacts); a folder copy through `copy_tree` and `put_file`; quota admission of a copy | `core/fs_ops.rs:1352-1385`; `storage/volume.rs:261-295` |
| 5 | the sweep deletes superseded sets after a grace and current sets whose node is missing or whose revision differs | `storage/meta.rs:1176-1184`; `purge.rs:32,262,306-310` |
| 6 | project removal deletes every table of the volume list; a soft delete and restore keeps them | `storage/mod.rs:522-528`; `storage/meta.rs:2788-2800` |
| 7 | chunk sets follow a move and are removed on delete; search entries and jobs follow through hooks | `storage/meta.rs:1893-1903,2446,2530`; `search/indexer.rs:489,540,556`; `tools/lifecycle.rs:59-146` |
| 8 | artifacts are charged to the session only | `core/fs_ops.rs:880-895` (admission and session charge of a conversion), `safety.rs:150-170`; SPEC-0020 DEC-008 (`sdlc/specs/SPEC-0020-project-quota-enforcement/sdlc-design.md:201-204`) |
| 9 | existing tests asserting the discard and changing here: 088 move, 089 copy, 090 trash and restore, 095 folder move, 093 copy over, 097 away and back, 099 copy charge, `a_copy_is_charged_the_file_bytes_only`; unchanged: 094 archive, 108 edit, 092 pull, 141 checkout, 137 project restore, 134 move during conversion | `crates/core/src/tools/artifacts.rs` (`spec_0019_e2e_*`) |
| 10 | SPEC-0019 FR-NEW-011 and FR-NEW-012 wording | `sdlc/specs/SPEC-0019-document-artifacts/sdlc-spec.md:306-317` |
Test command: `cargo test -p fsa-core --lib -- tools::artifacts purge:: storage::`

### 14.3 Design inputs
- A move must carry the set pointer and sets to the new path in the same transaction as the rename, as chunk sets do (`storage/meta.rs:1893-1903`); `current_artifact_set` joins on the node revision, so the node revision stored in the pointer and the set must be updated with the rename.
- The set stores its `path` and `node_rev`; a copy needs a duplicate set with new ids and a blob reference per image (content addressed blobs, `incref`), under the quota admission lock.
- The trash path keeps the set; the sweep must not delete a set whose node still exists at a trash path. A trash entry is removed only by hard delete of its trash path (`core/fs_ops.rs:1263`) or by project removal; the set must go at that point (detection signal: a `doc_artifact_sets` row with no matching node after a hard delete). One change at `rename_into_trash` covers `fs.delete` and the retention sweep (`purge.rs:95`, `rename_with_collision_retry` at `core/fs_ops.rs:1175`); `trash_restore` is shared with the web trash screen. A read at `/.mcp_trash/...` must not return the artifacts (detection signal: the read answers the artifacts instead of `No artifacts`).
- Overwrite deletes the destination first (`core/fs_ops.rs:1313-1322`); its set must go with it before the incoming set is attached.
- Seven SPEC-0019 test groups change expectation (088, 089, 090, 093, 095, 097, and 099 with the copy charge test); detection signal: `cargo test -p fsa-core --lib -- tools::artifacts` fails on them. The golden contract is unchanged.
- Overwrite on move needs the destination's deletion and the rename in one transaction (`core/fs_ops.rs:1313-1322` deletes first today); overwrite on copy needs the destination's set replaced in the same transaction as `put_file`. Detection signal: a failure injected between the steps leaves both sets or neither.
- `rename_into_trash` removes sets under the path (`storage/meta.rs:1893-1903,2523-2538`) and must keep them; a restore goes through `tx_rename(carry_sets=true)` which moves only chunk sets today, and no doc table is touched by a rename; `sweep_stale_artifact_sets` deletes a set whose node is missing at its stored path or whose revision differs (`meta.rs:1176-1184`), so a move must rewrite `path` and `node_rev` of the pointer and the set, or the next sweep deletes it.
- `copy_file` and `copy_tree` share blobs and bypass any hook (`storage/volume.rs:264-295`); the copy's blob references are taken under the admission lock (`quota.rs:30-40`). A folder copy admits files with 0 session bytes today (`core/fs_ops.rs:1352`), so the artifact weight is charged separately.
- The allowance is observed in tests through `SafetyManager::session_remaining` and the safety write quota setting (`safety.rs:150-170`), and in the scenarios by writes of exact sizes.
- A trashed item sits at a trash path where `ensure_file` accepts it; reading its artifacts there must be refused (FR-NEW-007).
- Test hooks: E2E-045 runs the retention sweep with an explicit now (`purge.rs`, `sweep_project_files_at`); E2E-058 runs the project purge past its grace period (`storage/mod.rs:522-528`).
- A folder hard delete must also drop the sets of every document under it (`core/fs_ops.rs:1263`); a move of a folder onto an existing folder with overwrite deletes the whole destination tree, so the destination's other documents lose their artifacts with their files, unlike the folder copy merge.
- The restore name is the whole trash name plus `_restored` (`tools/trash.rs:242`, `/a.txt_restored`), not an inserted suffix.
