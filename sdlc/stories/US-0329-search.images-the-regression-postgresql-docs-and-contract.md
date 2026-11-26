# US-0329: search.images, the regression, PostgreSQL, docs and contract

> Spec: SPEC-0109 (sdlc/specs/SPEC-0109-search-chunks-and-images/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0328
> Files touched: see design section 7
> Closes spec: yes

## Objective
search.images, the regression, PostgreSQL, docs and contract.

## Requirements covered
FR-NEW-009, 010, 001 (images).

## Independent Test
`cargo test -p fsa-core --lib --features rag -- e2e_0109_` (E2E-004, 005 to 006 (images), 016, 018).

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002), `--features rag` for vector tests (AUTO-009).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
