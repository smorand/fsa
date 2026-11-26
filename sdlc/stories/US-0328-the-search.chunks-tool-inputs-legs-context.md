# US-0328: the search.chunks tool: inputs, legs, context

> Spec: SPEC-0109 (sdlc/specs/SPEC-0109-search-chunks-and-images/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0327
> Files touched: see design section 7
> Closes spec: no

## Objective
the search.chunks tool: inputs, legs, context.

## Requirements covered
FR-NEW-001 to 008, 011, 012.

## Independent Test
`cargo test -p fsa-core --lib --features rag -- e2e_0109_` (E2E-001 to 003, 005 to 012, 014, 015).

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002), `--features rag` for vector tests (AUTO-009).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
