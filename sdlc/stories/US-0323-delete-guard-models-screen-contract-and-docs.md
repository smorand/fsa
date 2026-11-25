# US-0323: delete guard, models screen, contract and docs

> Spec: SPEC-0107 (sdlc/specs/SPEC-0107-per-project-indexing-profile/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0322
> min_tier: 2
> Files touched: 8
> Closes spec: yes

## Objective
A used model cannot be deleted; the models screen shows each profile; contract and docs are current.

## Requirements covered
FR-NEW-010, FR-NEW-012, FR-MOD-001 (E2E-004, E2E-013), golden contract, README, AGENTS.md, `.agent_docs`.

## Independent Test
`cargo test -p fsa-core --lib -- e2e_0107_` then the full gate.

## Definition of done
- [ ] RED evidence first. Gate green. Docs updated. Closes spec: all 15 tests green.
- [ ] Commit `US-0323: delete guard, models screen, contract and docs`.
