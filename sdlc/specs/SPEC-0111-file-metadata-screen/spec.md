# File metadata screen: functional specification

> Id: SPEC-0111
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 2 areas (the read page, the recalculate action), 12 scenarios, no new persisted concept, no new dependency, no new tool. Auto run, part 5 of 5 (BL-0175); parts 1 to 4 are landed (v0.12.0 to v0.15.0).
> Status: implemented
> Generated on: 2026-11-26
> Project: existing
> From backlog: BL-0175
> Depends on: SPEC-0101, SPEC-0104, SPEC-0100
> Security: stored XSS and CSRF sweep (FR-NEW-007, FR-NEW-009)
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
The metadata of a file (document text, images with caption and OCR text, tables, headings, chunk sets and their chunks) is readable only through tools. A person who wants to follow, test and debug it needs a page. This increment adds the browser screen `/app/file-metadata`: a member opens a file of a project, sees its metadata, steps through the chunks of a set one by one, and presses a button to recalculate the file and watch the job state.

## 2. Current State (MANDATORY, every depth)
Four browser screens exist: `/app/tokens`, `/app/trash`, `/app/deleted-projects`, `/app/models` (plus `/app/converters`). They share one convention: the caller is the forwarded header, the bearer header or the read only `mcpfs_token` cookie; a post authenticated by the cookie needs a single use `csrf_token` issued by the page; every value is escaped by `escape_html`; every action calls the typed function a tool calls, never a second implementation. The read tools exist (`fs.extract_text`, `fs.get_document_view`, `fs.list_images`, `fs.list_tables`, `fs.get_document_structure`, `fs.list_chunk_sets`, `fs.list_chunks`, `fs.get_chunk`, `fs.list_derived_jobs`, `fs.recalculate`). None of them has a page.

### 2.1 What users can do today
- Read each datum through a tool call, one tool per datum (14.1 row 1).
- Not see the chunks of a file, the next chunk, or the state of a recalculation without a client (14.1 row 1).

### 2.2 Specs governing this area
- SPEC-0104 (read tools of the document metadata), SPEC-0101 (chunk sets), SPEC-0100 (recalculation and jobs), SPEC-0011 and SPEC-0102 (screen convention, reused unchanged).

### 2.3 What is verified today
Each tool by its own `e2e_` tests; the convention by the tests of the other screens. No test opens a page for a file.

## 3. Scope
### 3.1 In scope
- `GET /app/file-metadata?mount_id=&path=` showing the metadata, with a project picker when `mount_id` is absent.
- Stepping through the chunks of a set (previous, next) and paging the chunk list.
- `POST /app/file-metadata/recalculate` for the kinds `search_index` and `chunks`, answering the job.
- Same identity, CSRF and escaping convention as the other screens.
### 3.2 Out of scope, and why
- Stored vectors (`fs.get_vectors`): optional in the request, not needed by an approved scenario (S-10).
- Editing chunks, deleting or creating sets, vector recalculation (`admin.recalculate_*`): not asked.
- Picture thumbnails: the picture download needs a bearer header that a cookie page cannot send; the page links the picture id only.
- A new tool or REST route (S-2B): the page calls the typed functions of the tools.
### 3.3 Constraints imposed by the context
- The UI follows the existing screens (server side HTML in the module, no assets, no script); S-25 (rust-embed, app-design) is for new apps (S-2B, AUTO-022).
- The 152 frozen tool contract is untouched.

## 4. Actors
| Actor | Role |
|---|---|
| Member | opens files of its projects, steps through chunks, recalculates |
| Outsider (not a member, platform administrator included) | refused |

## 4.5 Bounded contexts
Browser screens (identity, anti forgery), file metadata (artifacts, chunks), derived jobs.

## 5. Usage scenarios
### 5.1 Happy
- **S1** A member opens `/app/file-metadata?mount_id=atlas&path=/report.pdf`: the conversion text, each image with id, page, caption and OCR text, each table with caption and size, the headings, and every chunk set with strategy, settings, default flag, counts, state and build time.
- **S2** The member opens a chunk (`chunk=2` of a set): the full text, heading path, pages, tokens; links to the previous and the next chunk when they exist.
- **S3** The member presses recalculate (`search_index` or `chunks`): the page answers with the job, its kind and state, and the jobs of the file are listed.
### 5.2 Failures
- **F1** A caller who is not a member of the project is refused with 403 and no metadata.
- **F2** An unknown file gives a clear page "file not found" with no section (404).
- **F3** A recalculation post by cookie with a missing, unknown or wrong person `csrf_token` is refused with 403 and starts no job.
- **F4** Recalculating a file whose job is already running or waiting answers that file's job as the tool does and leaves no second waiting job.
- **F5** No identity at all: 401.
### 5.3 Extra
- **X1** Every stored value is escaped: file name, caption, OCR text, heading, chunk text, set settings.
- **X2** A set of 10000 chunks is paged: 100 rows per page, a link to the next page, never the whole set.
- **X3** A file with no conversion shows an empty state per section, not an error.
- **X4** A `csrf_token` works once: a replay is refused and starts no second job.

## 6. Functional requirements
### 6.1 New
- **FR-NEW-001** `GET /app/file-metadata` is always mounted. With `mount_id` and `path` it answers 200 `text/html` with five sections: text, images, tables, headings, chunk sets, then the jobs of the file and the recalculate forms.
- **FR-NEW-002** The text is the stored conversion text (`fs.get_document_view`, view `text`), cut to 20000 characters with the full length stated. Images, tables and headings are the first page (100) of their tool, with a link to the next page when a marker is returned.
- **FR-NEW-003** Chunk sets are `fs.list_chunk_sets`. The set shown is `set_id` or the default set; its chunks are `fs.list_chunks`, 100 per page by `marker`; `chunk=N` shows `fs.get_chunk` N in full with links to N-1 and N+1 when they exist.
- **FR-NEW-004** `mount_id` absent: a project picker of the caller's projects, no metadata.
- **FR-NEW-005** Refusals: not a member 403 (before any metadata is read); unknown file 404 with the text `file not found`; a folder or a bad marker or chunk number the matching tool error, never a stack trace; no identity 401 JSON as the other screens.
- **FR-NEW-006** A file with no conversion shows `No conversion yet` for text, images, tables and headings; a file with no set shows `No chunk set`. Status 200.
- **FR-NEW-007** Every value from storage or from the request is escaped by `escape_html` before it enters the page, in text and in attribute positions; the page has no script.
- **FR-NEW-008** `POST /app/file-metadata/recalculate` with `mount_id`, `path`, `kind` (`search_index` or `chunks`), optional `set_id`, `csrf_token` calls `tools::derived::recalculate`, the function of `fs.recalculate`, as the caller; on success it answers 200 with the page showing the answer job (id, kind, state, seconds left) and the jobs of the file; a tool refusal is shown with its status (403, 404, 400, 501).
- **FR-NEW-009** CSRF as the other screens: required only when the cookie authenticated; one use, bound to the person; a missing, unknown, replayed or other person token is 403 `{"error":"ERR_FORBIDDEN","detail":"missing or invalid csrf_token"}`; membership is checked first. Bearer or header identity is exempt.
- **FR-NEW-010** The page reads and writes nothing else: opening it changes no file, no access time and starts no job.

## 7. Business rules
- The page shows what the tools answer; it computes nothing (S-2B).
- The recalculation rules (one waiting job per file and kind, the running one marked stale) are those of SPEC-0100, unchanged.

## 8. Decisions
- DEC-001 (S-05) Unknown file is 404 with a clear sentence, not an empty 200: the status tells scripts and people alike.
- DEC-002 (S-05) The answer of a post is the page itself, not a redirect: the job answer (its id, its state) is what the person asked to see, and a redirect would lose it.
- DEC-003 (S-10) No vectors on the page.
- DEC-004 (S-05) Text cut at 20000 characters, 100 rows per list page, as the tools page at 100.

## 9. Acceptance
### 9.1 Scenarios to automated tests
Every scenario of section 5 has one end to end test named `e2e_0111_NNN_<slug>` (12 tests, `cargo test -p fsa-core --lib -- e2e_0111_`): 001 S1, 002 S2, 003 S3, 004 F1, 005 F2, 006 F3, 007 F4, 008 F5, 009 X1, 010 X2, 011 X3, 012 X4.
### 9.2 Non regression
`e2e_new_058` (merge order of `app.rs`), the other screens' tests, the tool contract golden (unchanged).
### 9.3 Out of the tests
A visual check by hand (section 14.3).

## 14. Evidence
### 14.1 Current State proof
1. The screens: `crates/core/src/models_screen.rs`, `trash_screen.rs`, `token_screen.rs` (convention); the tools: `tools/artifacts.rs`, `tools/chunks.rs`, `tools/derived.rs` (all `pub(crate)` typed functions).
### 14.2 Security sweep
Stored XSS: five stored sources (name, caption, OCR, heading, chunk text) all pass `escape_html`, pinned by X1. CSRF: FR-NEW-009 and X4. Authorization: FR-NEW-005. Path traversal: the path goes through `normalize_path`.
### 14.3 By hand
`/app/file-metadata?mount_id=<project>&path=/<converted file>`.
