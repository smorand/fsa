# US-0351: the start: address, banner, key check, stored and seed rules

> Spec: SPEC-0116 (sdlc/specs/SPEC-0116-settings-server-auth/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0350
> Files touched: see design section 6
> Closes spec: no

## Objective
The start: address, banner, key check, stored and seed rules.

## Requirements covered
FR-NEW-004, 010 to 012, 015, 020, 021, 028, 031, 032, 049

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0116_` (tests of this story: E2E-008 to 010, 027 to 034, 040 to 043, 067 to 071, 080 to 081, 103, 108, 110 to 112, 120 to 121, 123 to 124, 126 to 127, 132, 178, 199 to 200)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
