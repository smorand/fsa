# US-0315: a conversion uses the resolved converter

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0314
> min_tier: 1
> Files touched: 9
> Closes spec: no

## Objective
Make every conversion path choose the converter of the project, else the platform default, else the configuration file converter, from the registry at each call with no restart, finishing a running conversion with the converter it started with, and leave the configuration file converter and its start check as they are.

## Requirements covered
- SPEC-0106/FR-NEW-010: WHILE a document is converted THE platform SHALL use the converter of the project, else the platform default converter, else the converter of the configuration file, else refuse.
- SPEC-0106/FR-NEW-011: WHILE a conversion is running THE platform SHALL finish it with the converter it started with, AND a later conversion SHALL use what the registry holds at its start, with no restart.
- SPEC-0106/FR-NEW-013 (partial: one set of rules): The platform SHALL apply to a converter of the registry the rules it applies to the converter of the configuration file: the answer cap, the sandbox of a call, the failure report, the document metadata of the converter answer, the order of refusals and the write that stays when a conversion fails.
- SPEC-0106/FR-NEW-020: The platform SHALL keep reading the converter of the configuration file, checked when the platform starts, and SHALL behave exactly as before when the registry holds no default and no project converter.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ docs:: tools::write tools::document api::dataplane`. Tests convert with a project converter, the default and the file converter, change them between conversions, run all three paths, check an upgraded platform with an empty registry and a refusal to start on an invalid file converter.

## Design excerpt
### Decisions
- DEC-006 (SPEC-0106): `AppState::doc_service_for(mount_id)` resolves in the order project, default, `AppState.doc_service`, builds the service from the row at each call and returns an `Arc`; the callers hold it for the whole conversion, which gives FR-NEW-011 (a running conversion keeps its converter). No cache, no new `AppState` field. **Obliged by:** SPEC-0106/FR-NEW-010, FR-NEW-011. **Rationale:** SPEC-0102 DEC-007 resolves search targets at call time the same way. **Alternatives considered:** a cache keyed by id and update time; an `ArcSwap`. **Source:** existing code. **Evidence:** `settings/registry.rs:716-741`, `state.rs:25`, `app.rs:126`.
- DEC-016 (SPEC-0106): Program converters are built with `CliDocService::new(DocServiceCliConfig{command, timeout_secs}, ...)`, the existing launch with an argument list and `expand_argv`; no shell is introduced and the environment of the child is not cleared (proposal P2). **Obliged by:** SPEC-0106/FR-NEW-014, FR-NEW-002. **Rationale:** the same sink with the same controls. **Alternatives considered:** clearing the environment (changes the file converter). **Source:** existing code. **Evidence:** `docs/service.rs:355,404-412,656-683`.
- DEC-018 (SPEC-0106): The configuration file converter, its start check (`validate_doc_service`) and its `AppState` slot stay as they are; with an empty registry `doc_service_for` returns the slot unchanged. **Obliged by:** SPEC-0106/FR-NEW-020. **Rationale:** non regression. **Alternatives considered:** moving it into the registry (BL-0173). **Source:** existing code. **Evidence:** `config.rs:1181,1317-1380`, `app.rs:126`.
### Data model
No schema change.
### Contracts
`AppState::doc_service_for(mount_id) -> Result<Option<Arc<dyn DocService>>>`; `docs::resolve::build(&Converter, Option<SecretText>)`; every caller of `state.doc_service.as_deref()` (`tools/write.rs:143`, `tools/document.rs:118-121`, `mcp/server.rs:2050,2968`, `api/dataplane.rs:690-708,1631,1663`) resolves once per request and holds the `Arc`; the upload batch check keeps resolving once, each file resolves again.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/state.rs`, `crates/core/src/docs/resolve.rs`, `docs/mod.rs` | modify / create: `doc_service_for`, `build` |
| `crates/core/src/settings/converters.rs` | modify: `resolve` |
| `crates/core/src/tools/write.rs`, `tools/document.rs`, `mcp/server.rs`, `api/dataplane.rs` | modify: callers |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
### Existing patterns
Call time resolution `settings/registry.rs:716-741`; `from_config` `docs/service.rs:158`; the fixture helpers `docs/manifest_e2e.rs:76-163`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-009: a conversion uses the project converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_009_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010
- **Given** converter A (`Pandoc local`) returns `# From A` and converter B (`Docling API`) returns `# From B`; the platform default is A; the converter of `proj` is B
- **When** Bob writes `/report.pdf` in `proj` with the conversion option on and reads its extracted text
- **Then** the text is `# From B`
- **Priority:** Critical

### SPEC-0106/E2E-010: a conversion uses the platform default when the project has none
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_010_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010
- **Given** the platform default is converter A; `proj` has no converter
- **When** Bob converts `/report.pdf`
- **Then** the text is `# From A`
- **Priority:** Critical

### SPEC-0106/E2E-011: a conversion uses the configuration file converter when nothing is selected
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_011_*`)
- **Scenario:** SC-010
- **Requirements:** FR-NEW-010, FR-NEW-020
- **Given** no default, no project converter, and the configuration file defines a converter returning `# From file`
- **When** Bob converts `/report.pdf`
- **Then** the text is `# From file`
- **Priority:** High

### SPEC-0106/E2E-012: a change applies to the next conversion without a restart
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_012_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-011
- **Given** the platform default is A and `/a.pdf` was converted with it
- **When** Alice sets B as the default, Bob converts `/b.pdf`
- **Then** `/b.pdf` gives `# From B` and `/a.pdf` still gives `# From A`
- **Priority:** High

### SPEC-0106/E2E-055: no converter anywhere
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_055_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012
- **Given** no project converter, no default and no converter in the configuration file
- **When** Bob converts `/report.pdf`
- **Then** the conversion is refused with `ERR_NOT_SUPPORTED: document service is not configured` and nothing is written
- **Priority:** High

### SPEC-0106/E2E-060: a converter removed between the check and the use
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_060_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-011, FR-NEW-010
- **Given** Bob's conversion has started with converter B
- **When** Alice deletes B after clearing the project selection while the conversion runs
- **Then** the running conversion ends with `# From B`; the next conversion uses the default
- **Priority:** Medium

### SPEC-0106/E2E-068: the project converter wins over the default and the file
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_068_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010
- **Given** the file converter, a platform default A and a project converter B all exist
- **When** Bob converts
- **Then** the text is `# From B`; after Alice clears the project converter `# From A`; after she clears the default `# From file`
- **Priority:** High

### SPEC-0106/E2E-075: all three conversion paths use the resolved converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_075_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-013
- **Given** the converter of `proj` is B
- **When** Bob converts through a write with the option, through an upload with the option and through the conversion of an existing file
- **Then** all three give `# From B`
- **Priority:** High

### SPEC-0106/E2E-079: the other project is not affected
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_079_*`)
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009, FR-NEW-010
- **Given** `proj` has converter B, `other` has none, the default is A
- **When** a member of each converts
- **Then** `proj` gives `# From B`, `other` gives `# From A`
- **Priority:** Medium

### SPEC-0106/E2E-080: the configuration file converter is untouched while the registry is empty
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_080_*`)
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020
- **Given** the platform upgrades with an empty registry and a configuration file converter
- **When** Bob converts and Alice lists converters
- **Then** the conversion behaves as before the upgrade and the list is empty
- **Priority:** High

### SPEC-0106/E2E-081: a registry default works with the configuration file converter disabled
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_081_*`)
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020, FR-NEW-010
- **Given** the configuration file converter is disabled and the platform default is A
- **When** Bob converts
- **Then** the conversion succeeds with `# From A`
- **Priority:** Medium

### SPEC-0106/E2E-082: the configuration file is still checked at start
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_082_*`)
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020
- **Given** the configuration file converter has two `{document}` placeholders
- **When** the platform starts
- **Then** it refuses to start with the existing configuration error text
- **Priority:** Medium

### SPEC-0106/E2E-095: a conversion in flight keeps the project converter it started with
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_095_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-011
- **Given** Bob's conversion in `proj` has started with converter B
- **When** Alice sets converter A as the converter of `proj` while it runs, then Bob converts another file
- **Then** the running conversion ends with `# From B`, the next one gives `# From A`
- **Priority:** Medium

### SPEC-0106/E2E-098: a refused creation changes nothing
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_098_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001, FR-NEW-010
- **Given** one converter exists and `proj` converts with the configuration file converter
- **When** Alice sends 3 refused creations (empty name, bad url, bad timeout)
- **Then** the list still holds one converter, the record of changes gained nothing and `proj` still converts as before
- **Priority:** Medium

### SPEC-0106/E2E-104: a platform with only a registry default converts
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_104_*`)
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020, FR-NEW-010
- **Given** an upgraded platform whose configuration file converter is disabled; Alice creates a converter and sets it as default
- **When** Bob converts
- **Then** the conversion works and the configuration file stays as it was
- **Priority:** Medium

### SPEC-0106/E2E-106: creating converters changes no conversion
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_106_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-010
- **Given** `proj` converts with the configuration file converter
- **When** Alice creates two converters without choosing any
- **Then** Bob's next conversion still gives `# From file`
- **Priority:** Medium

### SPEC-0106/E2E-107: the companion text and the report do not depend on the converter source
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_107_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-013
- **Given** the same stand in answer is given by a registry converter and by the configuration file converter
- **When** Bob converts one file with each
- **Then** the companion text and the conversion report are identical
- **Priority:** Medium

### SPEC-0106/E2E-110: no converter anywhere after an upgrade
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_110_*`)
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020, FR-NEW-010
- **Given** an upgraded platform whose configuration file converter is disabled and an empty registry
- **When** Bob writes `/report.pdf` with the conversion option
- **Then** he gets `ERR_NOT_SUPPORTED: document service is not configured` and nothing is written
- **Priority:** Medium

## Constraints
- Keep the order of checks of `write_bytes_documented` for now (US-0316 adds the secret step).
- With an empty registry behave as before.
- No cache and no `AppState` field.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0315: a conversion uses the resolved converter` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
