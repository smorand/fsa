# A converted document is rebuilt after its source changes: functional specification

> Id: SPEC-0113
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 3 areas (companion origin, rebuild kind, manual recalculation), 17 scenarios, one new table, one tool argument value, no new dependency. Auto run 2, part 2 of 6 (BL-0176).
> Status: implemented
> Generated on: 2026-11-26
> Project: existing
> From backlog: BL-0176
> Depends on: SPEC-0100, SPEC-0105, SPEC-0106
> Security: none (no new input, no new surface; the background conversion reads files its project already holds)
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
A source document converted to Markdown (`fs.documentize`, or an upload with the conversion flag) leaves a companion `.md`, a set of images, tables and headings. When the source is replaced, nothing refreshes them. This increment records on each companion what made it (its origin), adds the kind `conversion` to the SPEC-0100 rebuild mechanism so a replaced source is converted again once it has settled, and lets a member ask for it by hand with `fs.recalculate`. A companion that was written or edited by hand is never overwritten.

## 2. Current State (MANDATORY)
`write_companion` (`core/fs_ops.rs`) converts inside the request of an upload with the flag or of `fs.documentize`, charges quota through `land_conversion` (the volume admission lock), writes the companion at `companion_md_path` and commits the artifact set. Nothing records that the companion came from a conversion: `/a.pdf` and `/a.docx` share `/a.md`, and a hand written `/a.md` is indistinguishable from a converted one. The scheduler (`search::derived::Scheduler`) knows the kinds `search_index`, `chunks` and `vectors`; `fs.recalculate` accepts `search_index` and `chunks`.

### 2.1 What users can do today
- Replace `/a.pdf`: the companion, images and tables stay those of the old bytes until someone calls `fs.documentize` again (14.1 row 1).
- Nothing tells which source made `/a.md` (14.1 row 2).

### 2.2 Specs governing this area
SPEC-0019 and SPEC-0103 (conversion, artifacts), SPEC-0105 (artifacts follow their document), SPEC-0106 (converter resolution), SPEC-0100 (the scheduler, the settle delay, `derived_job`), SPEC-0020 (admission lock).

### 2.3 What is verified today
Conversion by its own tests; the scheduler by `e2e_0100_`. No test checks a conversion after a source change.

## 3. Scope
### 3.1 In scope
- A recorded origin per companion, set by every conversion (upload with flag, `fs.documentize`, background rebuild).
- The kind `conversion` of `derived_job`: scheduled by a change of a converted source, run after the settle delay, listed by `fs.list_derived_jobs`.
- `fs.recalculate` with `kind: "conversion"`.
- Upload with the flag or `fs.documentize` cancels a waiting conversion of the same source.
### 3.2 Out of scope, and why
- Chunk sets and search entries of the companion: the existing write hooks index the companion after a conversion exactly as after `fs.documentize` (`after_companion`).
- A screen button for the new kind: the SPEC-0111 screen is not changed.
- Following a moved or trashed source: SPEC-0105 rules stand, no new rebuild (DEC-006).
### 3.3 Constraints imposed by the context
No second conversion path: the background run calls the same `write_companion` engine. The tool contract changes only for `fs.recalculate` (description and `kind` text): the golden is regenerated and reviewed.

## 4. Actors
| Actor | Role |
|---|---|
| Member | replaces sources, asks a recalculation |
| Server | rebuilds after the settle delay, under the system identity `system:conversion` |

## 4.5 Bounded contexts
Document conversion, derived data jobs (SPEC-0100), project storage quota (SPEC-0020).

## 5. Usage scenarios
### 5.1 Happy
- **S1** Replace `/a.pdf` that was converted: after the settle delay the companion, the images and the tables are those of the new bytes.
- **S2** `fs.recalculate` `kind: "conversion"` on `/a.pdf` converts it now and answers a running job.
- **S3** `/a.pdf` and `/a.docx` share `/a.md`: only the source that converted last triggers a rebuild when replaced.
### 5.2 Failures
- **F1** The converter is down: the job ends `failed` with the reason, the old companion stays; the images and tables stay unreadable as for any changed document (SPEC-0019) until a conversion lands.
- **F2** `/a.md` was not made by conversion: a recalculation leaves it untouched, the job ends `failed` with a note.
- **F3** The project quota would be exceeded: nothing is written, the job ends `failed`, the old companion stays (images and tables as in F1).
- **F4** The source changes while the conversion runs: the result is discarded, a new rebuild follows the new bytes.
### 5.3 Extra
- **X1** Replacing the older of two sources schedules nothing (see S3).
- **X2** A companion edited by hand, then its source replaced: nothing is scheduled, the companion stays.
- **X3** `fs.documentize` during the delay cancels the waiting job.
- **X4** An upload with the conversion flag cancels the waiting job.
- **X5** A source moved or trashed schedules no new rebuild (SPEC-0105).
- **X6** A source written again with the same bytes never calls the converter.
- **X7** A conversion and an upload racing at the quota boundary never exceed the limit (same admission lock).
- **X8** A database from before this version opens, its companions read as manual and are never overwritten.
- **X9** No converter configured: the job ends `failed` with `document service is not configured`.
- **X10** A restart keeps a waiting conversion job and fails a running one then queues it again.
- **X11** Same behaviors on PostgreSQL.
- **X12** A member edits the companion while the converter runs: the edit is kept, the job ends `failed` with the note.
- **X13** The source reverts to the bytes the companion was made from while another conversion runs: that run ends `superseded`.

## 6. Functional requirements
### 6.1 New
- **FR-NEW-001** Every conversion that lands a companion records its origin: the companion path, the source path, the sha256 of the source and the sha256 of the companion written. A later conversion of another source to the same companion path replaces the record.
- **FR-NEW-002** A companion is "converted" when a record exists for its path, names the source, and the companion node still has the recorded sha256. A companion with no record, or whose bytes differ from the record (edited by hand), is "manual".
- **FR-NEW-003** A write of a source file schedules one `conversion` job (cause `change`, settle delay of SPEC-0100) only when the companion of that path is converted and its record names this source and a different source sha256. When the record already carries the new sha256 (the conversion ran in the same request), a waiting job is cancelled instead. Nothing is scheduled when search has no scheduler.
- **FR-NEW-004** The job converts the source read when it starts, with the converter of the project (SPEC-0106 resolution), and replaces the companion, the images, the tables and the headings through the `write_companion` engine. Its cause `change` skips the converter when the source sha256 equals the recorded one (job `done`). Its cause `manual` always converts.
- **FR-NEW-005** The job never overwrites a manual companion: when the companion exists and is manual, it ends `failed` with the reason `'<md>' was not made by conversion or was edited since, left untouched`, nothing written. The check is made again under the volume admission lock at the instant of the write, so an edit made while the converter runs is kept and ends the job the same way. When the companion is absent, a `manual` job creates it, a `change` job ends `failed` with `'<md>' is gone, not recreated`.
- **FR-NEW-006** The conversion is charged as an upload is: through the volume admission lock (`quota::admit_sibling`), the project check first. A refusal writes nothing and ends the job `failed` with the quota message. The session part of the charge goes to the identity `system:conversion` and is refunded when the job ends, so it never accumulates.
- **FR-NEW-007** A change of the source while the job runs raises its stale flag (the SPEC-0100 rule): the result is discarded before it is written, the job ends `superseded` and the change's own job follows. A conversion refused by the node revision check ends `superseded` when the flag is raised.
- **FR-NEW-008** After a conversion landed, the companion is handed to `indexer::after_companion` exactly as `fs.documentize` does.
- **FR-NEW-009** `fs.documentize` and the upload flag cancel the waiting `conversion` job of the source they convert (FR-NEW-003 covers the upload by its sha256 rule; `after_companion` cancels for `fs.documentize` by the origin record).
- **FR-NEW-010** `fs.recalculate` accepts `kind: "conversion"` on a file: refusals in the contract order (project, member, kind, file, folder), then `set_id` given is `ERR_INVALID_ARGUMENT` (`set_id applies to kind 'chunks' only`), search off is `ERR_NOT_SUPPORTED`. It starts a job of cause `manual` as the other kinds do (the waiting one goes without a record, a running one becomes stale).
- **FR-NEW-011** Deleting a source forgets its conversion jobs (as every kind); moving it drops its conversion jobs; a restart keeps waiting conversion rows and ends running ones `failed` (`interrupted by a server restart`) then queues the source again.
- **FR-NEW-012** `fs.list_derived_jobs` lists the jobs with `kind: "conversion"`; a failed one carries its `reason`.
- **FR-NEW-013** A database created before this version has no origin table rows: the table is created at open, every companion reads as manual (FR-NEW-002).
### 6.2 Changed
- `fs.recalculate`: the `kind` text and the tool description name the third kind; the `kind` error reads `kind must be 'search_index', 'chunks' or 'conversion'`.
- Companion writes (`write_companion`) also record the origin (FR-NEW-001); their answer is unchanged.

## 7. Business rules
The settle delay, the one waiting and one running job per file and kind, the stale marking and the restart recovery are those of SPEC-0100, unchanged.

## 8. Decisions
- DEC-001 (S-2B) The origin is a table `companion_origin` of the volume metadata store, not a column of `nodes`: a node row is rewritten by every write, move and copy path, a table beside it cannot be reset by one of them; the check against the recorded companion sha256 also detects a hand edit without a hook on every write (AUTO-030).
- DEC-002 (S-05) "Hand edited becomes manual" is read from the sha256, not from a flag set by writers.
- DEC-003 (S-05) The rebuild needs the scheduler, so search must be enabled; it does not depend on the index mode (a project that indexes nothing still gets a fresh companion). The origin lookup runs once per write of a file (one primary key read), only when a scheduler exists (AUTO-031).
- DEC-004 (S-10) The session charge of a background conversion goes to `system:conversion` and is refunded: no per person budget is meaningful for a server job (AUTO-032).
- DEC-005 (S-05) A refused job is `failed` with a reason, the one place a member reads a job note.
- DEC-007 (S-2B) The origin row is written inside the locked window that writes the companion, and the write hook reads the companion and the origin under the same admission lock, so it never sees new bytes with the old origin (found by the skeptical review).
- DEC-008 (S-05) A manual recalculation of a source replaces a companion another source converted (the last conversion owns it, as `fs.documentize` with overwrite does); only cause `change` leaves it to its owner.
- DEC-006 (S-10) A moved or trashed source gets no rebuild; its origin record stays and names the old path, so the moved file reads as having no origin.

## 9. Acceptance
### 9.1 Scenarios to automated tests
`e2e_0113_NNN_<slug>` (`cargo test -p fsa-core --lib -- e2e_0113_`): 001 FR-NEW-001 origin recorded, 002 FR-NEW-002 manual and hand edited, 003 X8, 004 S1, 005 S3 and X1, 006 X2, 007 X6, 008 F1, 009 F2, 010 X9, 011 F3, 012 F4, 013 X3, 014 X4, 015 X5, 016 X7, 017 X10, 018 S2, 019 FR-NEW-010 refusals, 020 a recalculation supersedes a running conversion, 021 X11 PostgreSQL flow, 022 FR-NEW-007 a false guard drops the result, 023 FR-NEW-001, 002 and X7 on PostgreSQL, 024 a hand edit during the conversion is kept, 025 the write hook waits for a landing conversion, 026 a revert during the run supersedes it. FR-NEW-012 (listing, reason) is read by 008, 009 and 010.
### 9.2 Non regression
`e2e_0100_`, `e2e_0105_`, `e2e_0106_`, the conversion suites, the tool contract golden (regenerated, one tool changed).
### 9.3 Out of the tests
Replace a converted PDF then read its companion after the delay (14.3).

## 14. Evidence
### 14.1 Current State proof
1. `rg "companion_origin|JobKind::Conversion" crates/core/src` finds nothing before this change; `fs_ops.rs:488` `companion_md_path` replaces the extension.
2. `docs/extract.rs:488-497`: `/a.pdf` and `/a.docx` map to `/a.md`.
### 14.2 Security sweep
No new input. The background job reads a file of its own project with the project converter and writes inside the project under the admission lock; it runs under a fixed system identity, never a member's. A secret of a converter stays write only (SPEC-0106).
### 14.3 By hand
Convert `/a.pdf`, replace it, wait the settle delay, read `/a.md`; `fs.list_derived_jobs` shows the job; edit `/a.md`, replace `/a.pdf` again, nothing changes; `fs.recalculate` kind `conversion` on the edited one ends `failed` with the note.
