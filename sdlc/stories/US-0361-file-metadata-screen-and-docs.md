# US-0361: file metadata screen and docs

> Spec: SPEC-0118 (sdlc/specs/SPEC-0118-app-design-screens/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0360
> Files touched: see design section 6
> Closes spec: yes

## Objective
File metadata screen and docs.

## Requirements covered
FR-NEW-012, 014

## Independent Test
`env -u MCPFS_TEST_PG_DSN cargo test -p fsa-core --lib -- e2e_0118_` (tests of this story: see the design test to story map)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before production code.

## Definition of done
- [ ] A failing test recorded (RED run) before production code.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated.
