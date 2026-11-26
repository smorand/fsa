# Git operations and archive extraction schedule indexing: functional specification

> Id: SPEC-0112
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 2 areas (git working tree changes, archive extraction), 14 scenarios, no new persisted concept, no new dependency, no new tool. Auto run 2, part 1 of 6 (BL-0177).
> Status: implemented
> Generated on: 2026-11-26
> Project: existing
> From backlog: BL-0177
> Depends on: SPEC-0100
> Security: none (no new input, no new surface)
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
A file that a git operation or an archive extraction creates or changes is never indexed until it is written again, so search silently misses it. This increment hands those files to the SPEC-0100 scheduler (`search::derived::Scheduler`), exactly as a written file is, and makes the files they delete leave the index as a deleted file does.

## 2. Current State (MANDATORY)
The write hooks of `search::indexer` (`after_write_reread`, `after_delete`, `after_move`, `after_copy`, `after_restore`) are called by write, edit, patch, document, delete, move, copy and trash tools. No hook exists in `tools/git.rs`, `git/merge.rs`, `tools/archive.rs` or `tools/archive_single.rs` (`rg indexer:: crates/core/src` finds none there). The git operations write the volume through one function, `git::merge::apply_changes_atomically` (called by `charge_and_apply`), plus `git.checkout_file` and the clone import; the extraction writes through `write_bytes_atomic`.

### 2.1 What users can do today
- After `git.remote_pull`, `git.merge`, `git.branch_switch`, `git.reset`, `git.revert`, `git.cherry_pick`, `git.stash_apply` or `fs.extract_archive`, search misses the changed files until each is saved again (14.1 row 1).
- A file a git operation deletes keeps its search entries (14.1 row 1).

### 2.2 Specs governing this area
SPEC-0100 (the scheduler and the settle delay), SPEC-0101 (chunk sets, rebuilt with the file), SPEC-0015 (extraction), SPEC-0105 (artifacts follow their document).

### 2.3 What is verified today
Each operation by its own tests; the write hooks by `e2e_0100_`. No test checks the index after a git operation or an extraction.

## 3. Scope
### 3.1 In scope
- Every git operation that changes working tree files of the volume: `git.branch_switch`, `git.branch_create` (with `checkout`), `git.branch_reset`, `git.reset`, `git.merge`, `git.merge_resolve`, `git.remote_pull`, `git.rebase` and its continue, `git.cherry_pick` and its continue, `git.revert` and its continue, `git.stash_save`, `git.stash_apply`, `git.stash_pop`, `git.checkout_file`, `git.remote_clone`.
- `fs.extract_archive` (and its REST twin) for every format, including the single file formats (`.gz`, `.bz2`, `.xz`).
### 3.2 Out of scope, and why
- A size limit for indexing: none exists today (a rebuild reads any UTF-8 text); none is invented here (AUTO-026).
- Git smart HTTP pushes: they change the object store, not the volume files.
- Copying chunk sets or entries from the previous version of a file: the rebuild owns them (SPEC-0101).
### 3.3 Constraints imposed by the context
Never a second indexing path: the hooks are `search::indexer::after_write_reread` and `after_delete`. Audit lines are unchanged. The 152 tool contract is unchanged.

## 4. Actors
| Actor | Role |
|---|---|
| Member | runs git operations and extractions |

## 4.5 Bounded contexts
Git working tree, archive extraction, search indexing (SPEC-0100).

## 5. Usage scenarios
### 5.1 Happy
- **S1** A pull that changes 3 files: after the settle delay `search.query` finds all 3.
- **S2** Extracting a zip of 5 documents: after the delay all 5 are searchable.
- **S3** A merge that deletes one file and renames another: the deleted file and the old name leave the index, the new name is searchable.
### 5.2 Failures
- **F1** A merge that conflicts changes no file and schedules no job (the conflicting paths included).
- **F2** A project whose index mode is none schedules nothing, for a git operation and for an extraction.
- **F3** A second operation on the same file before the delay collapses into one rebuild.
### 5.3 Extra
- **X1** A rename in a commit is a delete plus a create: the old path loses its entries, the new path is scheduled.
- **X2** A binary file is scheduled like any file and ends as a finished rebuild with no entry, never a failed job (AUTO-026).
- **X3** The rebuilt entries equal those of a write of the same text (same scheduler, same cause `change`).
- **X4** An extraction that fails halfway schedules only the files it wrote.
- **X5** An operation refused by the quota schedules nothing.
- **X6** The audit lines of every operation are unchanged and no line is added.
- **X7** Every operation of 3.1 is covered: a table driven test per operation.

## 6. Functional requirements
### 6.1 New
- **FR-NEW-001** After a git operation of 3.1 has written or deleted files of the volume, each written path is handed to `after_write_reread` and each deleted path to `after_delete`, once per path, whatever the answer of the operation (success, refusal after a partial apply included): exactly the paths the apply really touched.
- **FR-NEW-002** An operation that touches nothing (conflict answer, refusal before the apply, quota refusal) schedules nothing and forgets nothing.
- **FR-NEW-003** A rename in a commit appears as a deletion of the old path and a creation of the new one (the tree delta has no rename detection); both follow FR-NEW-001.
- **FR-NEW-004** After `fs.extract_archive` has written files, each file written (not the directories) is handed to `after_write_reread`, whatever the answer, so an extraction failing halfway schedules the files already written and no other. `overwrite=true` replacing a file counts as a change.
- **FR-NEW-005** No hook runs while a repository lock or the admission lock is held; the hooks run after the apply or write finished.
- **FR-NEW-006** A project whose index mode is none, or a server with search off, gets no job and no read from any hook (the hooks already answer nothing, `active_backend`).
- **FR-NEW-007** Audit lines, answers and errors of every operation are byte identical to the previous version.
### 6.2 Changed
None. A path under the trash folder is never scheduled (as `arrive` does).

## 7. Business rules
The settle delay, the one waiting job per file, the stale marking of a running job and the retry rules are those of SPEC-0100, unchanged.

## 8. Decisions
- DEC-001 (S-05) The scope is every git operation that writes the volume, not only the seven of the backlog: the index must follow the files, and the apply is one function (AUTO-026).
- DEC-002 (S-05) Deleted paths follow `after_delete` (rule of fs.delete): the scheduler forgets its jobs, the entries go.
- DEC-003 (S-10) No indexing size limit is invented.

## 9. Acceptance
### 9.1 Scenarios to automated tests
`e2e_0112_NNN_<slug>` (`cargo test -p fsa-core --lib -- e2e_0112_`): 001 S1, 002 S3, 003 F1, 004 F2 git, 005 F3, 006 X1, 007 X2, 008 X3, 009 X6, 010 X7 table, 011 X5 git; 012 S2, 013 F2 archive, 014 X4, 015 X5 archive, 016 single file formats, 017 overwrite; 018 PostgreSQL.
### 9.2 Non regression
`e2e_0100_`, `e2e_0105_`, git and archive suites, the tool contract golden (unchanged).
### 9.3 Out of the tests
A pull then a search by hand (14.3).

## 14. Evidence
### 14.1 Current State proof
1. `rg indexer:: crates/core/src` shows no call in `tools/git.rs`, `git/merge.rs`, `tools/archive.rs`, `tools/archive_single.rs`.
### 14.2 Security sweep
No new input, no new surface. The hooks read files already readable by the operation.
### 14.3 By hand
`git.remote_pull` then `search.query` after the settle delay; `fs.extract_archive` of a zip then `search.query`; `fs.list_derived_jobs` shows the jobs.
