# US-0334: the recalculate action and docs

> Spec: SPEC-0111 (sdlc/specs/SPEC-0111-file-metadata-screen/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0333
> Files touched: see design section 7
> Closes spec: yes

## Objective
The recalculate form and POST with CSRF, the job answer, README, AGENTS and agent docs.

## Requirements covered
FR-NEW-008, 009 (S3, F3, F4, X4).

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0111_003 e2e_0111_006 e2e_0111_007 e2e_0111_012`

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
