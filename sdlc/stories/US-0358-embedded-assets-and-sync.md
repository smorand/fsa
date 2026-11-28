# US-0358: embedded assets and sync

> Spec: SPEC-0118 (sdlc/specs/SPEC-0118-app-design-screens/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> Files touched: see design section 6
> Closes spec: no

## Objective
Embedded assets and sync.

## Requirements covered
FR-NEW-001, FR-NEW-002

## Independent Test
`env -u MCPFS_TEST_PG_DSN cargo test -p fsa-core --lib -- e2e_0118_` (tests of this story: see the design test to story map)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before production code.

## Definition of done
- [ ] A failing test recorded (RED run) before production code.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated.
