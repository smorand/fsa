# US-0348: the server section, the section list and the change order

> Spec: SPEC-0116 (sdlc/specs/SPEC-0116-settings-server-auth/)
> Nature: FEAT
> Priority: P1
> Depends on: -
> Files touched: see design section 6
> Closes spec: no

## Objective
The server section, the section list and the change order.

## Requirements covered
FR-NEW-001 to 003, 038, 042, 046, 048

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0116_` (tests of this story: E2E-001, 003 to 005, 007, 037 to 039, 044 to 046, 066, 119, 125, 139, 154 to 156, 172 to 177, 179, 191 to 196, 198)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
