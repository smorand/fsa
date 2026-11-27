# US-0345: the three sections and their rules

> Spec: SPEC-0115 (sdlc/specs/SPEC-0115-settings-git-web-context7/)
> Nature: FEAT
> Priority: P1
> Depends on: -
> Files touched: see design section 6
> Closes spec: no

## Objective
the three sections and their rules.

## Requirements covered
FR-NEW-001 to 006, 012, 013, 015 to 017

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0115_` (tests of this story: E2E-001, 002, 006 to 010, 012, 015, 016, 018, 019, 022 to 028, 033, 034, 036, 037, 039, 040, 041, 047, 052, 053, 054)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
