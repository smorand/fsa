# US-0343: Tools, REST routes and the settings screen

> Spec: SPEC-0114 (sdlc/specs/SPEC-0114-settings-store-and-bootstrap/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0342
> Files touched: see design section 7
> Closes spec: no

## Objective
Tools, REST routes and the settings screen.

## Requirements covered
FR-NEW-003 to 008

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0114_`

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
