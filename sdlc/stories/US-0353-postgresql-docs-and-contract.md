# US-0353: PostgreSQL, docs and contract

> Spec: SPEC-0116 (sdlc/specs/SPEC-0116-settings-server-auth/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0352
> Files touched: see design section 6
> Closes spec: yes

## Objective
PostgreSQL, docs and contract.

## Requirements covered
FR-NEW-017

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0116_` (tests of this story: E2E-047 to 049)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
