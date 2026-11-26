# US-0333: the file metadata page

> Spec: SPEC-0111 (sdlc/specs/SPEC-0111-file-metadata-screen/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> Files touched: see design section 7
> Closes spec: no

## Objective
The read page /app/file-metadata mounted in app.rs: text, images, tables, headings, chunk sets, chunk stepping, jobs, project picker, escaping, paging, empty state, refusals.

## Requirements covered
FR-NEW-001 to 007, 010 (S1, S2, F1, F2, F5, X1, X2, X3).

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0111_001 e2e_0111_002 e2e_0111_004 e2e_0111_005 e2e_0111_008 e2e_0111_009 e2e_0111_010 e2e_0111_011`

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
