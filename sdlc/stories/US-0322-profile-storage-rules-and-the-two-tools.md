# US-0322: profile storage, rules and the two tools

> Spec: SPEC-0107 (sdlc/specs/SPEC-0107-per-project-indexing-profile/)
> Nature: FEAT
> Priority: P1
> Depends on: none
> min_tier: 1
> Files touched: 9
> Closes spec: no

## Objective
A project stores a profile; `admin.get_indexing_profile` and `admin.set_indexing_profile` read and change it.

## Requirements covered
FR-NEW-001 to 009, FR-NEW-011, FR-NEW-013, FR-NEW-014 (design 5, DEC-001 to DEC-006).

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0107_` (E2E-001 to 003, 005 to 012, 014, 015).

## Constraints
No dashes as punctuation. Tool count 148.

## Definition of done
- [ ] RED evidence for every test before production code.
- [ ] Gate green: fmt check, clippy all features, `cargo test --workspace`.
- [ ] Commit `US-0322: profile storage, rules and the two tools`.
