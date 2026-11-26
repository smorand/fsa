# US-0336: archive extraction follows the index

> Spec: SPEC-0112 (sdlc/specs/SPEC-0112-git-and-archive-schedule-indexing/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0335
> Files touched: see design section 7
> Closes spec: no

## Objective
fs.extract_archive hands each file it wrote to the scheduler, on success and on a failure halfway.

## Requirements covered
FR-NEW-004, 006, 007 (S2, F2, X4, X5).

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0112_012 e2e_0112_013 e2e_0112_014 e2e_0112_015 e2e_0112_016 e2e_0112_017`

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
