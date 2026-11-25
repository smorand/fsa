# US-0321: the journey, the upgrade and the docs

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0320
> min_tier: 2
> Files touched: 7
> Closes spec: yes

## Objective
Close the spec: the end to end journey, an upgrade that leaves the registry and the record empty, and the docs.

## Requirements covered
- SPEC-0106/FR-NEW-020 (partial: the upgrade): The platform SHALL keep reading the converter of the configuration file, checked when the platform starts, and SHALL behave exactly as before when the registry holds no default and no project converter.
- SPEC-0106/FR-NEW-016 (partial: nothing recorded by an upgrade): The platform SHALL record every creation, update and deletion of a converter and every change of the platform default and of the converter of a project, with who did it, when, what it concerned and which fields changed, AND SHALL NOT record any value.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_` then `make check`. The journey creates two converters, a default and a project converter, converts, clears and deletes; the upgrade test starts on a database without the new tables and checks nothing is written.

## Design excerpt
### Decisions
- DEC-018 (SPEC-0106): The configuration file converter, its start check (`validate_doc_service`) and its `AppState` slot stay as they are; with an empty registry `doc_service_for` returns the slot unchanged. **Obliged by:** SPEC-0106/FR-NEW-020. **Rationale:** non regression. **Alternatives considered:** moving it into the registry (BL-0173). **Source:** existing code. **Evidence:** `config.rs:1181,1317-1380`, `app.rs:126`.
### Data model
No schema change.
### Contracts
No new contract. If a test finds a defect it is fixed here and said so.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
| `README.md`, `AGENTS.md` | modify: converters, the test command `cargo test -p fsa-core --lib -- e2e_0106_`, tool and route counts |
| `.agent_docs/architecture.md`, `.agent_docs/tools.md`, `.agent_docs/api.md`, `.agent_docs/config.md`, `.agent_docs/testing.md` | modify |
| `config/local.yaml.template` | modify: the `doc_service` comment |
### Existing patterns
`.agent_docs/config.md` doc_service block; SPEC-0102 docs for the registry pattern.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-016: a converter set up end to end
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_016_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-001, FR-NEW-003, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-007
- **Given** no converter; the stand in HTTP converter B and the stand in program A are running
- **When** Alice creates A and B, makes A the default, gives `proj` the converter B, Bob converts `/report.pdf`, Alice clears the project converter, Bob converts `/other.pdf`, Alice clears the default and deletes B
- **Then** the first text is `# From B`, the second `# From A`, the deletion of B succeeds only after the project converter was cleared
- **Priority:** Critical

### SPEC-0106/E2E-105: an upgrade leaves the registry and the record empty
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_105_*`)
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020, FR-NEW-016
- **Given** an existing platform upgrades
- **When** Alice lists converters and changes
- **Then** both are empty and nothing was written by the upgrade
- **Priority:** Medium

## Constraints
- No production change expected.
- No dashes as punctuation in docs.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: All files named in Paths.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.
- [ ] Closes spec: the spec full acceptance set green (every E2E-001 to E2E-150 and E2E-MOD-001) and every requirement of SPEC-0106 cited by at least one story with status done or landed.
- [ ] One commit `US-0321: the journey, the upgrade and the docs` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
