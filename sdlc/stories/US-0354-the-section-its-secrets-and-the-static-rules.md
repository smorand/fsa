# US-0354: The section, its secrets and the static rules

> Spec: SPEC-0117 (sdlc/specs/SPEC-0117-settings-infra-stores/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> Files touched: see design section 6
> Closes spec: no

## Objective
The section, its secrets and the static rules.

## Requirements covered
FR-NEW-001 to 004, 009, 014, 016

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0117_` (tests of this story: E2E-001 to 008, 010 to 014, 016, 017, 056, 059)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
