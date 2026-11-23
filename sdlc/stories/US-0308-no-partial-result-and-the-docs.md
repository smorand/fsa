# US-0308: non members, no converter call, audit and the docs

> Spec: SPEC-0105 (sdlc/specs/SPEC-0105-document-artifacts-follow-their-document/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0307
> min_tier: 2
> Files touched: 5
> Closes spec: yes

## Objective
Close the spec: a non member cannot move, copy or restore, no operation calls the converter, the audit log keeps its entries, and the README, AGENTS.md and agent docs describe artifacts following their document.

## Requirements covered
- SPEC-0105/FR-NEW-010: A refused or failed move, copy or restore SHALL NOT change the artifacts of any document, those of the source, of the destination and of the trash item.
- SPEC-0105/FR-NEW-011: A move, copy, trash or restore of a file that has no artifacts SHALL NOT give it or its copy any artifacts.
- SPEC-0105/FR-NEW-012: A move, copy, trash or restore SHALL NOT call the document converter, AND SHALL NOT alter the content of the artifacts it keeps.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0105_` then `make check`. Tests count converter calls over a move, copy, trash and restore, compare the audit log, refuse Eve on each operation, replace the bytes of a document after a move, and refuse a copy over the allowance without an audit entry.

## Design excerpt
### Decisions
- DEC-009 (SPEC-0105): Admission before writing: a single file copy calls `quota::admit_file_with(..., extra_session = weight)`, a folder copy calls `admit_paths(session_bytes = total weight, ...)`; both refuse before anything is written, so nothing is kept (FR-NEW-005). If `copy_artifact_sets` fails after the files were written, the copied files are removed and the session charge refunded; a failure after an overwrite is reported as an internal error. **Obliged by:** SPEC-0105/FR-NEW-005, FR-NEW-010. **Rationale:** the allowance check is the only expected refusal. **Alternatives considered:** write the sets inside `put_file` (couples the volume to documents). **Source:** existing code. **Evidence:** `quota.rs:47-62,137-160`, `core/fs_ops.rs:1352-1385`.
- DEC-013 (SPEC-0105): Chunk sets and search entries keep their rules (chunk sets carried on move and restore, removed on trash and delete, not copied); the new code touches none of them and the audit log is unchanged (a restore writes none). **Obliged by:** SPEC-0105/FR-NEW-012. **Rationale:** non regression. **Alternatives considered:** none. **Source:** SPEC-0100, SPEC-0101. **Evidence:** `storage/meta.rs:1893-1903`, `search/indexer.rs:489-556`.
### Data model
No schema change.
### Contracts
No new contract. If a test finds a defect it is fixed here and said so.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/tools/artifacts_follow_e2e.rs` | modify: tests |
| `README.md`, `AGENTS.md` | modify: artifacts follow, the test command `cargo test -p fsa-core --lib -- e2e_0105_` |
| `.agent_docs/architecture.md`, `.agent_docs/testing.md`, `.agent_docs/tools.md` | modify: the carry rule, the weight, the move guard |
### Existing patterns
README artifact section; `.agent_docs/architecture.md` artifact store paragraph.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0105/E2E-017: a non member cannot move, copy or restore
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_017_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D converted and a trashed converted document; Eve (`eve@example.com`) is not a member of `proj`
- **When** Eve moves, copies and restores them
- **Then** each is refused with `ERR_FORBIDDEN: 'eve@example.com' is not a member of 'proj'`
- **And** every artifact is unchanged for Alice
- **Priority:** High

### SPEC-0105/E2E-030: an unknown project
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_030_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** no project `ghost`
- **When** Alice moves a file in `ghost`
- **Then** she is refused with `ERR_PROJECT_NOT_FOUND: project 'ghost' not found`
- **Priority:** Low

### SPEC-0105/E2E-054: no converter call on any of these operations
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_054_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012
- **Given** D converted through a converter that counts its calls (1 call)
- **When** Alice moves, copies, trashes and restores D
- **Then** the converter was called 1 time in total
- **Priority:** High

### SPEC-0105/E2E-055: the audit log keeps its entries
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_055_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-012
- **Given** D converted
- **When** Alice moves, copies, deletes and restores D
- **Then** the audit log has the same one entry for the move, the copy and the delete as before this change, none for the restore (as before) and none for the artifacts
- **Priority:** Low

### SPEC-0105/E2E-056: a refused operation changes nothing the operator sees
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_056_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D converted
- **When** Alice's copy is refused for quota (E2E-018)
- **Then** the audit log has no entry for the copy and the used storage is unchanged
- **Priority:** Medium

## Constraints
- No production change expected.
- No dashes as punctuation in docs.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: All files named in Paths.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.
- [ ] Closes spec: the spec full acceptance set green (every E2E-001 to E2E-071 and E2E-MOD-001 to E2E-MOD-007) and every requirement of SPEC-0105 cited by at least one story with status done or landed.
- [ ] One commit `US-0308: non members, no converter call, audit and the docs` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0105` and the requirement ids.
