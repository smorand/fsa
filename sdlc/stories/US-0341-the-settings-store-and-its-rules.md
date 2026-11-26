# US-0341: The settings store and its rules

> Spec: SPEC-0114 (sdlc/specs/SPEC-0114-settings-store-and-bootstrap/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> Files touched: see design section 7
> Closes spec: no

## Objective
The settings store and its rules.

## Requirements covered
FR-NEW-001 to 005, 013, 014

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0114_`

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
