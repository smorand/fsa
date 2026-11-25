# US-0310: http converters and their encrypted secret

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0309
> min_tier: 1
> Files touched: 6
> Closes spec: no

## Objective
Accept the http form: address, header name and value, file field, response field and an encrypted write only secret, with the exact checks of the spec, and the unreadable master key refusal.

## Requirements covered
- SPEC-0106/FR-NEW-003: IF a converter has the form `http` THEN THE platform SHALL accept it only with an http or https address that carries no user name or password and a non empty file field, SHALL accept no command, AND SHALL keep a secret given with it encrypted and write only.
- SPEC-0106/FR-NEW-015 (partial: the stored secret is encrypted and never answered): The platform SHALL NOT reveal the secret of a converter in any answer, listing, record of changes, screen, log line or failure text.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_`. Tests create http converters with and without a secret, check every http rule text and order, a blank secret, the header checks, a 4097 character secret, the missing master key, and that no answer holds the secret.

## Design excerpt
### Decisions
- DEC-004 (SPEC-0106): The tool inputs are `serde_json::Value` fields (no `Debug` derive) and the rule module `converter_rules.rs` produces the type texts (`<field> must be text`, the range texts, `command must be a list of text arguments`, `clear_secret must be true or false`); a field is given when present and not `null`, and the order of checks is the one written in FR-NEW-001 to FR-NEW-005. **Obliged by:** SPEC-0106/FR-NEW-001, 002, 003, 004, 005. **Rationale:** typed argument structs would turn a wrong type into a generic parse error with another text. **Alternatives considered:** typed structs with custom deserializers (text mapping scattered). **Source:** spec. **Evidence:** `mcp/server.rs:1582` (current typed provider args).
- DEC-005 (SPEC-0106): The secret uses `SecretText`, `encrypt` and `decrypt` of the registry; a blank secret is no secret; the header name and the value are checked with `reqwest::header::HeaderName` and `HeaderValue` (no dependency added); update keeps the stored blob unless a non blank secret or `clear_secret` is given, so it needs no key otherwise. **Obliged by:** SPEC-0106/FR-NEW-003, FR-NEW-005, FR-NEW-015. **Rationale:** the cipher and its failure text exist. **Alternatives considered:** a hand written header grammar. **Source:** existing code. **Evidence:** `settings/secrets.rs:12-57`, `settings/registry.rs:77`.
- DEC-011 (SPEC-0106): Secrets never reach a log: `tracing` calls of the registry carry converter ids and names only; no input or answer struct derives `Debug` over a secret; answers carry `has_secret`. **Obliged by:** SPEC-0106/FR-NEW-015. **Rationale:** the SPEC-0102 rule. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `settings/secrets.rs:29-37`.
### Data model
Columns `url`, `auth_header`, `secret_enc` (blob), `file_field`, `response_field` of `converter` are used.
### Contracts
Rules in the order of FR-NEW-003: url required (empty or blank counts), scheme, host and 2048 characters, user name, command given, `auth_header` with a secret (a blank header is `Authorization` without a secret), `file_field`, header name, header value, secret length (after trimming), master key. The secret is stored with `encrypt`; answers carry `has_secret`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/settings/converter_rules.rs`, `converters.rs`, `converter_store.rs` | modify |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
| `crates/core/src/tools/converters.rs` | modify only if the tool needs the key error mapped |
### Existing patterns
`SecretText`, `encrypt` `settings/secrets.rs:12-57`; the missing key text `settings/registry.rs:77`; the provider endpoint rule `settings/registry.rs:38-50`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-002: create an http converter with a secret
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_002_*`)
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-003
- **Given** no converter exists
- **When** Alice creates `Docling API`, form `http`, url `http://localhost:8099/convert`, secret `Bearer tok-123`
- **Then** the answer shows `auth_header` `Authorization`, `file_field` `file`, `response_field` empty, `has_secret` true and no secret value anywhere in it
- **Priority:** Critical

### SPEC-0106/E2E-027: an http converter without a url
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_027_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with no url
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url is required for an http converter`
- **Priority:** High

### SPEC-0106/E2E-028: an http converter with a url that is not http or https
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_028_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with the url `ftp://host/convert`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url must be an http or https URL`
- **Priority:** High

### SPEC-0106/E2E-029: an http converter with a user name in the url
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_029_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003, FR-NEW-015
- **Given** no converter exists
- **When** Alice creates an `http` converter with the url `http://bob:pw@localhost:8099/convert`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url must not carry a user name or password`, and the answer does not quote the url
- **Priority:** High

### SPEC-0106/E2E-030: a secret without a header name
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_030_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with the secret `tok` and the header name `   `
- **Then** she is refused with `ERR_INVALID_ARGUMENT: auth_header is required when a secret is set`
- **Priority:** High

### SPEC-0106/E2E-031: an empty file field
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_031_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with the file field `  `
- **Then** she is refused with `ERR_INVALID_ARGUMENT: file_field must not be empty`
- **Priority:** High

### SPEC-0106/E2E-032: a command given to an http converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_032_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with a valid url and the command `["c", "{document}"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command is not valid for an http converter`
- **Priority:** High

### SPEC-0106/E2E-037: a secret of 4097 characters
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_037_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates a valid `http` converter with a secret of 4097 characters
- **Then** she is refused with `ERR_INVALID_ARGUMENT: secret must be at most 4096 characters`
- **Priority:** High

### SPEC-0106/E2E-038: a secret when no master key is configured
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_038_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003, FR-NEW-015
- **Given** the platform has no master key
- **When** Alice creates a valid `http` converter with the secret `tok` on a platform without a master key
- **Then** she is refused with `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY` and no converter is created
- **Priority:** High

### SPEC-0106/E2E-097: a converter created with every field given
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_097_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001, FR-NEW-003, FR-NEW-004
- **Given** no converter
- **When** Alice creates `Docling API`, form `http`, url `https://converter.example.com/v1/convert`, auth_header `X-Api-Key`, secret `k`, file_field `document`, response_field `markdown`, extensions `["pdf"]`, max_input_bytes 1000000, timeout_seconds 60
- **Then** the answer shows every value as given, `has_secret` true
- **Priority:** Medium

### SPEC-0106/E2E-125: a blank secret means no secret
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_125_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003, FR-NEW-005
- **Given** no converter
- **When** Alice creates an `http` converter with the secret `   `, then updates `Docling API` (which has a secret) with the secret `   `
- **Then** the creation stores no secret (`has_secret` false) and the update keeps the stored secret
- **Priority:** Medium

### SPEC-0106/E2E-126: a header name that is not valid
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_126_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter
- **When** Alice creates an `http` converter with `auth_header` `X Api Key` and the secret `abc`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: auth_header is not a valid header name` and the text does not quote the name
- **Priority:** Medium

### SPEC-0106/E2E-127: a secret that is not a valid header value
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_127_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003, FR-NEW-015
- **Given** no converter
- **When** Alice creates an `http` converter with a secret holding a line break
- **Then** she is refused with `ERR_INVALID_ARGUMENT: secret is not a valid header value` and the text does not quote the secret
- **Priority:** Medium

### SPEC-0106/E2E-128: an address without a host or longer than 2048 characters
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_128_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter
- **When** Alice creates an `http` converter with the url `http://`, then with a url of 2049 characters
- **Then** each is refused with `ERR_INVALID_ARGUMENT: url must be an http or https URL` and the text does not quote the url
- **Priority:** Medium

### SPEC-0106/E2E-143: an empty url
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_143_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter
- **When** Alice creates an `http` converter with the url `` (empty text)
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url is required for an http converter`
- **Priority:** Medium

### SPEC-0106/E2E-144: a blank header name without a secret
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_144_*`)
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter
- **When** Alice creates an `http` converter with a valid url, no secret and `auth_header` `  `
- **Then** it is created with `auth_header` `Authorization`; with a secret and the same blank header she is refused with `ERR_INVALID_ARGUMENT: auth_header is required when a secret is set`
- **Priority:** Medium

## Constraints
- Do not use the secret outside the encrypt call; `SecretText` has no `Display`.
- No new dependency: header checks use `reqwest::header::{HeaderName, HeaderValue}`.
- The conversion itself is not touched here.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0310: http converters and their encrypted secret` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
