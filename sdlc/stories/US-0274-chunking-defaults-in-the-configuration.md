# US-0274: server defaults for the chunking settings

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 2
> Files touched: 4
> Closes spec: no

## Objective
Add the five server keys `search.chunking.*` with their defaults and bounds, refuse a bad value at start with the spec text whether or not search is enabled, and expose them to the rest of the code as one typed `ChunkDefaults` value. It also creates the test module `chunking/e2e.rs` that every later story extends.

## Requirements covered
- SPEC-0101/FR-NEW-027 (partial: parsing and validation; the defaults are used by US-0276 and US-0280): The system SHALL read the default settings of the chunk sets from the server configuration keys `search.chunking.fixed_size_tokens`, `search.chunking.fixed_overlap_tokens`, `search.chunking.heading_cut_depth`, `search.chunking.max_tokens` and `search.chunking.list_max_items`.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_131 e2e_0101_132 e2e_0101_145 e2e_0101_133 config::`. They load YAML configs with `heading_cut_depth` 9, `max_tokens` `abc` and `2.5`, `fixed_overlap_tokens` 100 with `fixed_size_tokens` 100, absent and empty keys, through `ServerConfig::from_yaml` and `validate()`, and assert the exact error text or the parsed `ChunkDefaults`.

## Design excerpt
### Decisions
- DEC-014 (SPEC-0101): Config keys `search.chunking.*` are text fields with the scalar deserializer and boot validation of `search.settle_delay_seconds`; the defaults struct is passed to the scheduler and the tools. **Obliged by:** SPEC-0101/FR-NEW-027. **Rationale:** same operator behavior. **Alternatives considered:** numeric fields (serde error text). **Source:** existing code. **Evidence:** `config.rs:862-890`, `config.rs:1213`.
### Data model
None.
### Contracts
YAML `search: { chunking: { fixed_size_tokens: 2048, fixed_overlap_tokens: 0, heading_cut_depth: 2, max_tokens: 2048, list_max_items: 40 } }`; each key accepts an integer, float or string scalar (`${VAR}` expanded before parsing). `SearchConfig::chunking() -> ChunkDefaults` (valid after `validate()`). Error: `ERR_INVALID_ARGUMENT: search.chunking.<key> must be a whole number between <min> and <max>`; for the overlap against the size: `ERR_INVALID_ARGUMENT: search.chunking.fixed_overlap_tokens must be smaller than search.chunking.fixed_size_tokens`. Ranges: fixed_size_tokens 32..8192, fixed_overlap_tokens 0..8191, heading_cut_depth 1..6, max_tokens 64..8192, list_max_items 2..200.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/config.rs` | modify: `ChunkingConfig` (text fields, scalar deserializer), `ChunkDefaults`, check inside `validate()` before the search enabled early return |
| `crates/core/src/chunking/mod.rs` | create: `ChunkDefaults` (plain struct of five `u32`) only |
| `crates/core/src/chunking/e2e.rs` | create: test module (registered in `lib.rs` as `#[cfg(test)]`), config tests |
| `config/local.yaml.template`, `.agent_docs/config.md` | modify: document the keys |
### Existing patterns
Copy the settle delay pattern: `de_scalar_text` `config.rs:862`, `parse_settle_delay` `config.rs:880`, the call before the `enabled` return in `validate_search` `config.rs:1213`, and the tests in `search/derived_e2e.rs` (plain `#[test]` with the config lock `crate::git::remote::tests::lock_for_test()`).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-131: a default out of range
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_131_*`)
- **Given** the server configuration has `search.chunking.heading_cut_depth` 9
- **When** the operator starts the server
- **Then** the start stops with `ERR_INVALID_ARGUMENT: search.chunking.heading_cut_depth must be a whole number between 1 and 6`
- **And** with 6 the server starts
- **Priority:** High

### SPEC-0101/E2E-132: a default that is not a whole number
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_132_*`)
- **Given** the server configuration has `search.chunking.max_tokens` `abc`, then `2.5`
- **When** the operator starts the server each time
- **Then** each start stops with `ERR_INVALID_ARGUMENT: search.chunking.max_tokens must be a whole number between 64 and 8192`
- **And** search is disabled in this configuration and the start still stops
- **Priority:** High

### SPEC-0101/E2E-145: an overlap default not below the size default
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_145_*`)
- **Given** the server configuration has `search.chunking.fixed_size_tokens` 100 and `search.chunking.fixed_overlap_tokens` 100
- **When** the operator starts the server
- **Then** the start stops with `ERR_INVALID_ARGUMENT: search.chunking.fixed_overlap_tokens must be smaller than search.chunking.fixed_size_tokens`
- **And** with 99 the server starts
- **Priority:** High

## Constraints
- Do not read the values anywhere except `SearchConfig::chunking()`.
- No CLI flag.
- No new dependency.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/config.md` and the YAML template.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.

- [ ] One commit `US-0274: server defaults for the chunking settings` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
