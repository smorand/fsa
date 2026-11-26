# US-0335: git operations follow the index

> Spec: SPEC-0112 (sdlc/specs/SPEC-0112-git-and-archive-schedule-indexing/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> Files touched: see design section 7
> Closes spec: no

## Objective
Every git operation that writes the volume hands its created or changed files to the scheduler and its deleted files to the delete rule.

## Requirements covered
FR-NEW-001 to 003, 005 to 007 (S1, S3, F1, F2, F3, X1, X2, X3, X5, X6, X7).

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0112_001 e2e_0112_002 e2e_0112_003 e2e_0112_004 e2e_0112_005 e2e_0112_006 e2e_0112_007 e2e_0112_008 e2e_0112_009 e2e_0112_010 e2e_0112_011`

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
