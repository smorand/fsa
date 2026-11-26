# US-0337: PostgreSQL, docs and contract

> Spec: SPEC-0112 (sdlc/specs/SPEC-0112-git-and-archive-schedule-indexing/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0336
> Files touched: see design section 7
> Closes spec: yes

## Objective
The new tests run on PostgreSQL, README, AGENTS.md and .agent_docs describe the behavior, the golden is regenerated and unchanged.

## Requirements covered
FR-NEW-001, 004 on PostgreSQL; docs.

## Independent Test
`env MCPFS_TEST_PG_DSN=... cargo test -p fsa-core --lib -- e2e_0112_018`

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] A failing test recorded (RED run) before any production code, for every test of the story.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
