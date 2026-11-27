# US-0350: identity with several keys

> Spec: SPEC-0116 (sdlc/specs/SPEC-0116-settings-server-auth/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0349
> Files touched: see design section 6
> Closes spec: no

## Objective
Identity with several keys.

## Requirements covered
FR-NEW-007 to 009, 025, 035, 040

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0116_` (tests of this story: E2E-018 to 026, 063, 085, 087 to 089, 117 to 118, 122, 133 to 135, 143, 147 to 150, 157, 170 to 171, 197)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
