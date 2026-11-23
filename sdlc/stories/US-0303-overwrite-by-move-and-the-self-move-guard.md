# US-0303: overwrite by move and the move onto itself refusal

> Spec: SPEC-0105 (sdlc/specs/SPEC-0105-document-artifacts-follow-their-document/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0302
> min_tier: 2
> Files touched: 4
> Closes spec: no

## Objective
Let a move with overwrite replace the destination's artifacts with the moved document's (or leave none), and refuse a move onto, into or over an ancestor of itself before anything is deleted.

## Requirements covered
- SPEC-0105/FR-NEW-006 (partial: overwrite by move): WHEN a member moves or copies a document onto an existing document with overwrite THE Documents context SHALL replace the destination's artifacts by those of the incoming document, AND the destination SHALL have no artifacts when the incoming document has none.
- SPEC-0105/FR-NEW-013: IF the destination of a move is the source, inside the source, or an ancestor of the source THEN THE Files context SHALL refuse the move and change nothing.
- SPEC-0105/FR-NEW-010 (partial: refusals leave every set as before): A refused or failed move, copy or restore SHALL NOT change the artifacts of any document, those of the source, of the destination and of the trash item.
- SPEC-0105/FR-NEW-012 (partial: chunk sets and search entries unchanged by a move): A move, copy, trash or restore SHALL NOT call the document converter, AND SHALL NOT alter the content of the artifacts it keeps.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0105_ api::dataplane tools::lifecycle`. Tests move D onto a converted document and onto a plain one with overwrite, move D onto itself with and without overwrite, a folder into itself and onto its ancestor, and a move while a conversion runs.

## Design excerpt
### Decisions
- DEC-006 (SPEC-0105): `move_path` checks, after the source exists and before the destination exists, whether the destination equals the source, lies under it, or is an ancestor of it (path prefix with a `/` boundary, `/` itself an ancestor of everything), and refuses with `ToolError::invalid_argument("'<source>' cannot be moved onto or into itself")`. **Obliged by:** SPEC-0105/FR-NEW-013. **Rationale:** today overwrite deletes the destination first. **Alternatives considered:** guard in `tx_rename` only (the delete has already run). **Source:** existing code. **Evidence:** `core/fs_ops.rs:1296-1328`.
- DEC-002 (SPEC-0105): `tx_rename` takes a second flag `carry_artifacts`. For each moved node, with its new revision `r`: delete the pointer row at the new path, then `UPDATE doc_artifact_sets SET path=new, node_rev=r WHERE path=old AND superseded_at IS NULL`, then `UPDATE doc_artifact_current SET path=new, node_rev=r WHERE path=old`, all inside the existing loop and transaction. `rename` passes true; sets left at an overwritten destination keep their old revision and are swept. **Obliged by:** SPEC-0105/FR-NEW-001, FR-NEW-002, FR-NEW-006. **Rationale:** a move already rewrites revisions per node and carries chunk sets the same way; one transaction means all or nothing (FR-NEW-010). **Alternatives considered:** key by node id (schema and reader change); update the sets at the call site (not atomic with the rename). **Source:** existing code. **Evidence:** `storage/meta.rs:1841-1900`, `:1176-1184`, `:1277-1290`.
- DEC-013 (SPEC-0105): Chunk sets and search entries keep their rules (chunk sets carried on move and restore, removed on trash and delete, not copied); the new code touches none of them and the audit log is unchanged (a restore writes none). **Obliged by:** SPEC-0105/FR-NEW-012. **Rationale:** non regression. **Alternatives considered:** none. **Source:** SPEC-0100, SPEC-0101. **Evidence:** `storage/meta.rs:1893-1903`, `search/indexer.rs:489-556`.
### Data model
No schema change.
### Contracts
`move_path` guard: after `client.exists(src)` (missing source `'<src>' does not exist`) and before the destination check, when `dst == src`, `dst` starts with `src + "/"`, or `src` starts with `dst + "/"` (or `dst == "/"`): `ERR_INVALID_ARGUMENT: '<src>' cannot be moved onto or into itself`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/core/fs_ops.rs` | modify: guard in `move_path` |
| `crates/core/src/tools/artifacts_follow_e2e.rs` | modify: tests |
| `crates/core/src/tools/lifecycle.rs` or its tests | modify only if an existing test expected `NO_CLOBBER` for a move onto itself |
### Existing patterns
`move_path` `core/fs_ops.rs:1296-1328`; existing no clobber tests in `tools/lifecycle.rs`; the pointer deletion at the destination is in `tx_rename` (US-0302).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0105/E2E-009: overwrite by move
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_009_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006
- **Given** D converted (2 images) and `/docs/old.pdf` converted with 1 image captioned `Old`
- **When** Alice moves `/docs/report.pdf` onto `/docs/old.pdf` with overwrite
- **Then** `/docs/old.pdf` gives D's 2 images and no trace of `Old`; `/docs/report.pdf` is refused with `ERR_NOT_FOUND: not a file: /docs/report.pdf`
- **Priority:** High

### SPEC-0105/E2E-015: a folder moved into itself
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_015_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** `/docs` holds D converted
- **When** Alice moves `/docs` to `/docs/inner`
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs' cannot be moved onto or into itself`, and D still gives its artifacts at `/docs/report.pdf`
- **Priority:** Medium

### SPEC-0105/E2E-020: a move while the document is being converted
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_020_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-010
- **Given** D is being converted: the converter has not answered yet
- **When** Alice moves `/docs/report.pdf` to `/archive/report.pdf` and the converter then answers
- **Then** the conversion is refused with the existing refusal for a document changed during its conversion; if D already had artifacts before, they are found at `/archive/report.pdf`; otherwise no artifacts exist at either path
- **Priority:** Medium

### SPEC-0105/E2E-022: a refused move onto an existing document keeps both sets
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_022_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-006
- **Given** D and `/docs/old.pdf` converted
- **When** Alice moves D onto `/docs/old.pdf` without overwrite
- **Then** the move is refused with `ERR_NO_CLOBBER: '/docs/old.pdf' exists (pass overwrite=true)`; both documents still give their own artifacts
- **Priority:** Medium

### SPEC-0105/E2E-029: a second member moves a converted document
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_029_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** D converted by Alice; Bob (`bob@example.com`) is a member of `proj`
- **When** Bob moves D to `/archive/report.pdf`
- **Then** Alice and Bob both read D's artifacts at `/archive/report.pdf`
- **Priority:** Medium

### SPEC-0105/E2E-036: a converted document overwritten by a plain file
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_036_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-006, FR-NEW-011
- **Given** D converted and a plain file `/docs/plain.txt`
- **When** Alice moves `/docs/plain.txt` onto `/docs/report.pdf` with overwrite
- **Then** `/docs/report.pdf` has no artifacts (`ERR_NOT_FOUND: No artifacts: document not extracted`) and holds the plain text
- **Priority:** High

### SPEC-0105/E2E-047: chunk sets and search entries behave as before
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_047_*`)
- **Scenario:** SC-007
- **Requirements:** FR-NEW-012
- **Given** D converted with a chunk set and a search entry
- **When** Alice moves D
- **Then** the chunk set and the search entry are found at the new path as before this change
- **Priority:** Medium

### SPEC-0105/E2E-060: a document moved onto itself with overwrite
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_060_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** D converted
- **When** Alice moves `/docs/report.pdf` onto `/docs/report.pdf` with overwrite
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs/report.pdf' cannot be moved onto or into itself` and D still gives its artifacts
- **Priority:** High

### SPEC-0105/E2E-061: a folder moved onto its ancestor with overwrite
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_061_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** `/docs/sub/other.pdf` (D2) converted
- **When** Alice moves `/docs/sub` onto `/docs` with overwrite
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs/sub' cannot be moved onto or into itself` and D2 still gives its artifacts
- **Priority:** High

### SPEC-0105/E2E-065: a move onto itself without overwrite
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_065_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** D converted
- **When** Alice moves `/docs/report.pdf` onto `/docs/report.pdf` without overwrite
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs/report.pdf' cannot be moved onto or into itself` (not the no-clobber refusal) and D still gives its artifacts
- **Priority:** High

### SPEC-0105/E2E-066: a folder moved onto its ancestor without overwrite
- Level / Driver / Location: functional e2e / the real McpServer tool methods, on a document converted by a SPEC-0103 stand in converter / `crates/core/src/tools/artifacts_follow_e2e.rs` (`e2e_0105_066_*`)
- **Scenario:** SC-002
- **Requirements:** FR-NEW-013, FR-NEW-010
- **Given** `/docs/sub/other.pdf` (D2) converted
- **When** Alice moves `/docs/sub` onto `/docs` without overwrite
- **Then** the move is refused with `ERR_INVALID_ARGUMENT: '/docs/sub' cannot be moved onto or into itself` and D2 still gives its artifacts
- **Priority:** High

## Constraints
- The overwrite deletion of the destination stays outside the rename transaction (existing).
- Do not touch trash or copy.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/tools.md` line for `fs.move` at US-0308.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0303: overwrite by move and the move onto itself refusal` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0105` and the requirement ids.
