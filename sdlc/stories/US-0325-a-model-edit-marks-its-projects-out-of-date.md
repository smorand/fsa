# US-0325: a model edit marks its projects out of date, PostgreSQL profile tests

> Spec: SPEC-0108 (sdlc/specs/SPEC-0108-vectors-per-project-dimension-and-recalculation/)
> Nature: FEAT
> Priority: P1
> Depends on: see design section 8
> Files touched: see design section 7
> Closes spec: no

## Objective
a model edit marks its projects out of date, PostgreSQL profile tests.

## Requirements covered
FR-NEW-010, FR-MOD-001.

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0108_` (E2E-012, 019).

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] RED evidence for every test before production code.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
