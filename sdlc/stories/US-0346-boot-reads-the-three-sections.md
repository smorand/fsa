# US-0346: boot reads the three sections

> Spec: SPEC-0115 (sdlc/specs/SPEC-0115-settings-git-web-context7/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0345
> Files touched: see design section 6
> Closes spec: no

## Objective
boot reads the three sections.

## Requirements covered
FR-NEW-007 to 011

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0115_` (tests of this story: E2E-003, 004, 005, 011, 013, 014, 017, 020, 029 to 032, 038, 042 to 046, 049, 050, 051)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
