# US-0355: Probes and data checks

> Spec: SPEC-0117 (sdlc/specs/SPEC-0117-settings-infra-stores/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0354
> Files touched: see design section 6
> Closes spec: no

## Objective
Probes and data checks.

## Requirements covered
FR-NEW-005 to 010

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0117_` (tests of this story: E2E-015, 020 to 026, 028, 030, 040 to 047, 057, 058)

## Constraints
No dashes as punctuation. Stable toolchain by PATH, `env -u MCPFS_TEST_PG_DSN` for the gate. Tests written and run RED before any production code, one commit and one full workspace gate per story.

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
