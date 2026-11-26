# US-0331: the search.documents tool

> Spec: SPEC-0110 (sdlc/specs/SPEC-0110-search-documents/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0330
> Files touched: see design section 7
> Closes spec: no

## Objective
the search.documents tool.

## Requirements covered
FR-NEW-001 to 009.

## Independent Test
`cargo test -p fsa-core --lib --features rag -- e2e_0110_`.

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002), `--features rag` for vector tests (AUTO-009).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
