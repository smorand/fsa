# US-0317: arguments stay literal, secrets stay hidden, no redirect

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0316
> min_tier: 1
> Files touched: 5
> Closes spec: no

## Objective
Prove and enforce the safety rules: arguments never reach a shell, the secret is sent as the header value and never revealed anywhere, a failure text redacts it before the cut, and no http converter follows a redirect. Carries the exploit tests.

## Requirements covered
- SPEC-0106/FR-NEW-014: The platform SHALL NOT pass a converter command to a shell: each argument is given to the program as one argument whatever characters it holds.
- SPEC-0106/FR-NEW-015: The platform SHALL NOT reveal the secret of a converter in any answer, listing, record of changes, screen, log line or failure text.
- SPEC-0106/FR-MOD-001: IF a conversion fails and the failure text quotes the converter's own answer THEN THE platform SHALL replace every occurrence of the converter's secret in that text by `[redacted]`.
- SPEC-0106/FR-MOD-002: No http converter, whether it comes from the registry or from the configuration file, SHALL follow a redirect.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ docs::service`. Exploit tests carry the literal payloads `x; touch /tmp/fsa-pwned #`, `$(touch /tmp/fsa-pwned)`, a backtick form and `x | touch /tmp/fsa-pwned`, and check the file stays absent; the echo stubs check `[redacted]` and the secret straddling the 2048 byte cut; the redirect stubs check the second stub saw nothing.

## Design excerpt
### Decisions
- DEC-010 (SPEC-0106): `ApiDocService::new` sets `redirect(Policy::none())`; a 3xx falls in the existing non success branch (`document service returned HTTP <status>: <slice>`). **Obliged by:** SPEC-0106/FR-MOD-002, FR-NEW-003. **Rationale:** one client builder serves both sources. **Alternatives considered:** a policy that strips custom headers (leaves the request to another host). **Source:** existing code. **Evidence:** `docs/service.rs:537-546,611-615`.
- DEC-011 (SPEC-0106): Secrets never reach a log: `tracing` calls of the registry carry converter ids and names only; no input or answer struct derives `Debug` over a secret; answers carry `has_secret`. **Obliged by:** SPEC-0106/FR-NEW-015. **Rationale:** the SPEC-0102 rule. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `settings/secrets.rs:29-37`.
- DEC-012 (SPEC-0106): `redact(text, secret)` replaces every occurrence of the trimmed secret (nothing when blank) in the whole response body before `head_of` cuts it, for the registry secret and the file token; applied to failure texts only. **Obliged by:** SPEC-0106/FR-MOD-001, FR-NEW-015. **Rationale:** cutting first would leak a prefix of a straddling secret. **Alternatives considered:** redacting after the cut. **Source:** spec. **Evidence:** `docs/service.rs:611-615,833`.
- DEC-016 (SPEC-0106): Program converters are built with `CliDocService::new(DocServiceCliConfig{command, timeout_secs}, ...)`, the existing launch with an argument list and `expand_argv`; no shell is introduced and the environment of the child is not cleared (proposal P2). **Obliged by:** SPEC-0106/FR-NEW-014, FR-NEW-002. **Rationale:** the same sink with the same controls. **Alternatives considered:** clearing the environment (changes the file converter). **Source:** existing code. **Evidence:** `docs/service.rs:355,404-412,656-683`.
### Data model
No schema change.
### Contracts
`docs::redact(text, secret)`; `ApiDocService::new` with `redirect(Policy::none())`; the failure text `document service returned HTTP <status>: <slice>` built from the redacted whole body then cut by `head_of`; the file token redacted the same way.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/service.rs`, `docs/mod.rs` | modify: redirect policy, `redact` |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
| `crates/core/src/docs/manifest_e2e.rs` | modify: `serve` becomes `pub(crate)` if not yet |
| the existing test of a converter failure that quotes the answer | modify (E2E-MOD-001) |
### Existing patterns
`docs/service.rs:537-546,611-615,827-833`. Exploit stand ins: `record-args` script, an echo stub, a 302 and a 307 stub, an exact header stub. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-059: a failed converter answer that echoes the secret
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_059_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-015, FR-MOD-001
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has the secret `Bearer tok-SECRET-123`; the stub answers an error whose body repeats the header it received
- **When** Bob converts `/report.pdf` through it
- **Then** the failure shown to Bob holds `[redacted]` where the secret was and never the text `tok-SECRET-123`
- **And** the same holds for the converter of the configuration file with its own token
- **Priority:** Critical

### SPEC-0106/E2E-067: an argument that looks like a shell command stays one argument
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_067_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-014
- **Given** no converter and no file `/tmp/fsa-pwned`; `record-args` writes the arguments it receives into the Markdown it returns
- **When** Alice creates a `program` converter with the command `["record-args", "x; touch /tmp/fsa-pwned #", "{document}"]` and Bob converts `/report.pdf` with it
- **Then** the extracted text, in which `record-args` writes the arguments it received, lists exactly 3 arguments, the second being `x; touch /tmp/fsa-pwned #`
- **And** no file `/tmp/fsa-pwned` exists (it did not exist before)
- **Priority:** Critical

### SPEC-0106/E2E-077: the secret is sent as the header value
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_077_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has `auth_header` `X-Api-Key` and the secret `abc`
- **When** Bob converts `/report.pdf`
- **Then** the conversion gives `# From B`, because the stub answers `# From B` only when it receives the header `X-Api-Key: abc` and no `Authorization` header (otherwise it answers 401)
- **Priority:** Medium

### SPEC-0106/E2E-078: a converter with a secret is kept after a restart
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_078_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003, FR-NEW-016
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) with the secret `tok`; the platform restarts with the same master key
- **When** Bob converts
- **Then** the conversion gives `# From B` (the stub answers it only for the header value `tok`); the list and the record of changes are unchanged
- **Priority:** Medium

### SPEC-0106/E2E-087: the secret is never revealed
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_087_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has the secret `tok-SECRET-123`
- **When** Alice lists, reads the default, reads the project converter, lists the changes, opens the screen and a conversion fails
- **Then** none of these answers, the screen page or the server log holds `tok-SECRET-123`
- **Priority:** Critical

### SPEC-0106/E2E-092: an argument cannot start a program of its own (exploit)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_092_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-014, FR-NEW-002
- **Given** no converter; the file `/tmp/fsa-pwned` does not exist; the stand in program `record-args` writes the arguments it receives into the Markdown it returns
- **When** Alice creates a `program` converter with the command `["record-args", "$(touch /tmp/fsa-pwned)", "`touch /tmp/fsa-pwned`", "x | touch /tmp/fsa-pwned", "{document}"]`, Bob converts `/report.pdf`
- **Then** the extracted text, in which `record-args` writes the arguments it received, lists 5 arguments, the three payloads each as one literal argument
- **And** no file `/tmp/fsa-pwned` exists (it did not exist before the test)
- **Priority:** Critical

### SPEC-0106/E2E-093: a converter answer cannot reveal the secret (exploit)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_093_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-015, FR-MOD-001
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) with the secret `Bearer tok-SECRET-123`; the stub answers status 500 with a body that repeats the header it received
- **When** Bob converts `/report.pdf`
- **Then** the failure text holds `[redacted]` and never `tok-SECRET-123`
- **And** the log holds no `tok-SECRET-123`
- **And** the converter of the configuration file with the token `cfg-TOKEN-456` behaves the same
- **Priority:** Critical

### SPEC-0106/E2E-094: the stored secret cannot be read back by any read (exploit)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_094_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) with the secret `tok-SECRET-123`
- **When** Alice reads every answer and screen of the converter registry, and an operator reads the stored form of the setting
- **Then** the text `tok-SECRET-123` appears in none (the stored form is encrypted)
- **And** the same refusals with no master key leave nothing stored
- **Priority:** Critical

### SPEC-0106/E2E-101: a stored secret is used by the conversion
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_101_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has the secret `Bearer tok-123`
- **When** Bob converts `/report.pdf`
- **Then** the conversion gives `# From B`, because the stub answers `# From B` only for the header `Authorization: Bearer tok-123` (otherwise it answers 401)
- **Priority:** High

### SPEC-0106/E2E-117: a secret that straddles the quoted slice is still redacted
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_117_*`)
- **Scenario:** SC-008
- **Requirements:** FR-MOD-001, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has the secret `tok-SECRET-123`; the stub answers 500 with a body that repeats the received header so that the secret starts before byte 2048 of the body and ends after it
- **When** Bob converts `/report.pdf`
- **Then** the failure text holds `[redacted]` and neither the secret nor any prefix of it longer than 3 characters
- **Priority:** High

### SPEC-0106/E2E-118: an http converter never follows a redirect
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_118_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003, FR-MOD-002, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has `auth_header` `X-Api-Key` and the secret `abc`; the stub answers 302 to a second stub on another port
- **When** Bob converts `/report.pdf`
- **Then** the conversion fails with the converter's HTTP status reported; the second stub received no request and no secret
- **Priority:** High

### SPEC-0106/E2E-135: the configuration file converter does not follow a redirect either
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_135_*`)
- **Scenario:** SC-008
- **Requirements:** FR-MOD-002, FR-NEW-015
- **Given** the configuration file converter is an http converter with the header `X-Api-Key` and the token `cfg-TOKEN-456`; its stub answers 302 to a second stub
- **When** Bob converts `/report.pdf`
- **Then** the conversion fails with the HTTP status reported; the second stub received no request and no token
- **Priority:** High

### SPEC-0106/E2E-141: a redirect to the same host is not followed
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_141_*`)
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003, FR-MOD-002
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists and its stub answers 307 with a relative `Location`
- **When** Bob converts `/report.pdf`
- **Then** the conversion fails with the HTTP status reported and the stub received exactly one request
- **Priority:** Medium

### SPEC-0106/E2E-MOD-001: a failed conversion quotes the converter answer without the secret (was an existing test of a converter failure reported in the answer; /sdlc-design names it)
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_mod001_*`)
- **Validated before:** a failed conversion shows the converter's own error text is reported with the text the converter returned.
- **Validates now:** the same, with every occurrence of the secret replaced by `[redacted]`.
- **Scenario:** SC-008
- **Requirements:** FR-MOD-001
- **Given** a converter holding the secret `tok-SECRET-123` whose error text repeats it
- **When** Bob converts a file and the converter fails
- **Then** the failure shown to Bob holds `[redacted]` and not `tok-SECRET-123`

## Constraints
- Exploit tests must be seen RED first: E2E-093 on today's file token, E2E-092 and E2E-067 against a temporary `sh -c` variant, E2E-094 against a temporary clear text storage.
- Never log a header or a secret.
- `/tmp/fsa-pwned` must be absent at the start of each exploit test.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0317: arguments stay literal, secrets stay hidden, no redirect` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
