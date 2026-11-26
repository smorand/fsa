# US-0330: the reconciliation formula and an honest keyword mode

> Spec: SPEC-0110 (sdlc/specs/SPEC-0110-search-documents/)
> Nature: FEAT
> Priority: P1
> Depends on: none
> Files touched: see design section 7
> Closes spec: no

## Objective
the reconciliation formula and an honest keyword mode.

## Requirements covered
FR-NEW-002, 003, 006, 008, FR-MOD-010.

## Independent Test
`cargo test -p fsa-core --lib --features rag -- reconcile e2e_0110_013 e2e_0109_`.

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002), `--features rag` for vector tests (AUTO-009).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
