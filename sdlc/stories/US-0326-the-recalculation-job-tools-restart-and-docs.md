# US-0326: the recalculation job, its tools, restart, docs and contract

> Spec: SPEC-0108 (sdlc/specs/SPEC-0108-vectors-per-project-dimension-and-recalculation/)
> Nature: FEAT
> Priority: P1
> Depends on: see design section 8
> Files touched: see design section 7
> Closes spec: yes

## Objective
the recalculation job, its tools, restart, docs and contract.

## Requirements covered
FR-NEW-008 to 009, 011 to 016, FR-MOD-002.

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0108_` (E2E-003, 004, 006, 007, 008, 014, 016, 017).

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] RED evidence for every test before production code.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
