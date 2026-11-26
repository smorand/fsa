# US-0340: fs recalculate conversion postgresql docs and contract

> Spec: SPEC-0113 (sdlc/specs/SPEC-0113-converted-documents-rebuild-after-the-source-changes/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> Files touched: see design section 7
> Closes spec: no

## Objective
A member can rebuild a converted document by hand; the behavior holds on PostgreSQL; docs and contract are current.

## Requirements covered
FR-NEW-010 (S2, X11).

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0113_018 e2e_0113_019 e2e_0113_020 e2e_0113_021`

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
