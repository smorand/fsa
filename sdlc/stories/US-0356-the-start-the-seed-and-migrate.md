# US-0356: The start, the seed and migrate

> Spec: SPEC-0117 (sdlc/specs/SPEC-0117-settings-infra-stores/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0355
> Files touched: see design section 6
> Closes spec: no

## Objective
The start, the seed and migrate.

## Requirements covered
FR-NEW-011 to 013, 015

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0117_` (tests of this story: E2E-029, 031, 048 to 054)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
