# US-0349: the auth section and its change rules

> Spec: SPEC-0116 (sdlc/specs/SPEC-0116-settings-server-auth/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0348
> Files touched: see design section 6
> Closes spec: no

## Objective
The auth section and its change rules.

## Requirements covered
FR-NEW-005, 006, 013, 022, 023, 026, 027, 029, 030, 037, 039, 041, 043, 045, 047

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0116_` (tests of this story: E2E-002, 006, 011 to 017, 035 to 036, 059 to 062, 064, 074 to 077, 083, 094 to 102, 113 to 116, 140 to 142, 144 to 146, 151 to 153, 158 to 162, 166 to 169, 180 to 182)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
