# US-0352: the secret variable names

> Spec: SPEC-0116 (sdlc/specs/SPEC-0116-settings-server-auth/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0351
> Files touched: see design section 6
> Closes spec: no

## Objective
The secret variable names.

## Requirements covered
FR-NEW-018, 019, 024, 034, 036, 044

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0116_` (tests of this story: E2E-050 to 058, 065, 072 to 073, 078 to 079, 082, 084, 086, 090 to 093, 104 to 107, 109, 128 to 131, 136 to 138, 163 to 165, 183 to 190)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
