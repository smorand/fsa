# US-0327: engine reads by vector, fusion and clamp

> Spec: SPEC-0109 (sdlc/specs/SPEC-0109-search-chunks-and-images/)
> Nature: FEAT
> Priority: P1
> Depends on: none
> Files touched: see design section 7
> Closes spec: no

## Objective
engine reads by vector, fusion and clamp.

## Requirements covered
FR-NEW-002, 003, 009 (engine side).

## Independent Test
`cargo test -p fsa-core --lib --features rag -- e2e_0109_` (E2E-013, 017 and the unit tests of fusion and clamp).

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002), `--features rag` for vector tests (AUTO-009).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
