# US-0324: vectors at the project dimension and fs.get_vectors

> Spec: SPEC-0108 (sdlc/specs/SPEC-0108-vectors-per-project-dimension-and-recalculation/)
> Nature: FEAT
> Priority: P1
> Depends on: see design section 8
> Files touched: see design section 7
> Closes spec: no

## Objective
vectors at the project dimension and fs.get_vectors.

## Requirements covered
FR-NEW-001 to 007.

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0108_` (E2E-001, 002, 005, 009, 010, 011, 013, 015, 018, 020).

## Constraints
No dashes as punctuation. Stable toolchain by PATH (AUTO-001), `env -u MCPFS_TEST_PG_DSN` for the gate (AUTO-002).

## Definition of done
- [ ] RED evidence for every test before production code.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Docs updated in the story that changes behavior.
