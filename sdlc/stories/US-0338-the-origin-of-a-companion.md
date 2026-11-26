# US-0338: the origin of a companion

> Spec: SPEC-0113 (sdlc/specs/SPEC-0113-converted-documents-rebuild-after-the-source-changes/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> Files touched: see design section 7
> Closes spec: no

## Objective
Every conversion records which source made its companion, and a companion not made by conversion or edited since is manual.

## Requirements covered
FR-NEW-001, 002, 013 (X8).

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0113_001 e2e_0113_002 e2e_0113_003`

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
