# US-0339: the conversion job after a source change

> Spec: SPEC-0113 (sdlc/specs/SPEC-0113-converted-documents-rebuild-after-the-source-changes/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> Files touched: see design section 7
> Closes spec: no

## Objective
A replaced converted source is converted again after the settle delay, never over a manual companion, charged like an upload.

## Requirements covered
FR-NEW-003 to 009, 011, 012 (S1, S3, F1 to F4, X1 to X7, X9, X10).

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0113_004 to e2e_0113_017`

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
