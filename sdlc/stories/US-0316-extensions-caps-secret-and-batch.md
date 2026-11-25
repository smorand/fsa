# US-0316: extensions, size cap, unreadable secret and batch behavior

> Spec: SPEC-0106 (sdlc/specs/SPEC-0106-document-converters-set-up-by-an-administrator/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0315
> min_tier: 1
> Files touched: 6
> Closes spec: no

## Objective
Apply the stored extensions and cap at conversion, refuse an extension the converter does not accept without trying another, refuse before any write in the order no converter, extension, size, unreadable secret, and keep the batch behavior as today.

## Requirements covered
- SPEC-0106/FR-NEW-012: IF the extension of the document is not accepted by the converter chosen under FR-NEW-010 THEN THE platform SHALL refuse the conversion and SHALL NOT try another converter.
- SPEC-0106/FR-NEW-013 (partial: the rest: cap, failure report, metadata): The platform SHALL apply to a converter of the registry the rules it applies to the converter of the configuration file: the answer cap, the sandbox of a call, the failure report, the document metadata of the converter answer, the order of refusals and the write that stays when a conversion fails.
- SPEC-0106/FR-NEW-010 (partial: unreadable secret and batch): WHILE a document is converted THE platform SHALL use the converter of the project, else the platform default converter, else the converter of the configuration file, else refuse.
- SPEC-0106/FR-NEW-004 (partial: the cap and extensions in use): The platform SHALL hold for every converter a list of accepted extensions, an answer cap in bytes and a timeout in seconds.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0106_ docs:: tools::write`. Tests convert with custom and empty extensions, `/notxt`, a document over the cap, an answer over the cap, a converter that fails, a secret that cannot be read after a restart with another key, a batch whose default changes between files, and a version 2 manifest through a registry converter.

## Design excerpt
### Decisions
- DEC-007 (SPEC-0106): `DocService` gains `fn preflight(&self) -> Result<()>` (default `Ok`); a registry converter whose secret cannot be read is built as `BrokenSecretService`, which accepts the same extensions and cap and whose `preflight` and `convert` answer `ERR_INTERNAL_ERROR: the secret of converter '<name>' cannot be read`; `write_bytes_documented` and `documentize` call it after `ensure_input_size` and before any write. **Obliged by:** SPEC-0106/FR-NEW-010, FR-NEW-012. **Rationale:** it gives the order no converter, extension, size, secret. **Alternatives considered:** failing at resolution (the extension and size texts would be lost). **Source:** existing code. **Evidence:** `core/fs_ops.rs:635-712,746-770`.
- DEC-008 (SPEC-0106): The stored extensions have no dot and lower case without duplicates; `build` passes them with a dot to `resolve_extensions`; an empty list falls back to `DOC_SERVICE_EXTS`; matching is the existing `eligible_in` (a path ends with the dotted extension). **Obliged by:** SPEC-0106/FR-NEW-004, FR-NEW-012. **Rationale:** the refusal text lists them as `.pdf, .docx` already. **Alternatives considered:** storing the dot (the spec stores none). **Source:** existing code. **Evidence:** `docs/service.rs:44-52,143-190`, `core/fs_ops.rs:746-760`.
- DEC-009 (SPEC-0106): `max_input_bytes` is passed as the service cap, which `ensure_input_size` and the answer limit already read. **Obliged by:** SPEC-0106/FR-NEW-004, FR-NEW-013. **Rationale:** one cap, as for the file converter. **Alternatives considered:** two fields. **Source:** existing code. **Evidence:** `core/fs_ops.rs:763-770`, `docs/service.rs:299`.
- DEC-019 (SPEC-0106): The batch check of an upload keeps resolving once for the converter and extension (as today), and each file resolves again when its own conversion starts; a refusal then stops the upload. **Obliged by:** SPEC-0106/FR-NEW-010. **Rationale:** matches today's partial writes. **Alternatives considered:** pre checking the size (a behavior change). **Source:** existing code. **Evidence:** `api/dataplane.rs:689-708`.
### Data model
No schema change.
### Contracts
`DocService::preflight`; `BrokenSecretService` returning `ERR_INTERNAL_ERROR: the secret of converter '<name>' cannot be read` from `preflight` and `convert`; `write_bytes_documented` and `documentize` call `ensure_documentable`, `ensure_input_size`, `preflight`, then write.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/service.rs`, `docs/resolve.rs` | modify: `preflight`, `BrokenSecretService` |
| `crates/core/src/core/fs_ops.rs` | modify: the preflight step |
| `crates/core/src/settings/converters_e2e.rs` | modify: tests |
### Existing patterns
`core/fs_ops.rs:635-712,746-770`; existing texts `fs_ops.rs:751-757,768`. Tests in `crates/core/src/settings/converters_e2e.rs` reuse the fixture `Fx` of `settings/e2e.rs` (administrator `admin@example.com`, member `bob@example.com` of project `proj`, a master key, `restart`, `rest`, the screen helpers `open` and `submit`) and the stand in converters of `docs/manifest_e2e.rs` (`serve`, script converters); the SPEC text uses Alice for the administrator and Bob for the member.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0106/E2E-054: an extension the resolved converter does not accept
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_054_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012
- **Given** the converter of `proj` accepts only `txt`; the platform default accepts `pdf`
- **When** Bob converts `/report.pdf` in `proj`
- **Then** the conversion is refused with the existing refusal `'/report.pdf' cannot be documented, the document service accepts: .txt`, the platform default is not tried and nothing is written
- **Priority:** High

### SPEC-0106/E2E-056: a converter that fails
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_056_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-013
- **Given** the converter of `proj` returns an error
- **When** Bob converts `/report.pdf`
- **Then** the file is written, the conversion failure is reported in the answer as for a converter of the configuration file and no companion or artifact exists
- **Priority:** High

### SPEC-0106/E2E-057: an answer over the converter cap
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_057_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-013, FR-NEW-004
- **Given** the converter of `proj` has `max_input_bytes` 1000, a 900 byte document `/report.pdf` and the converter returns 1001 bytes
- **When** Bob converts `/report.pdf`
- **Then** the failure is `document service output is over the doc_service.max_input_bytes limit of 1000` and nothing is kept
- **Priority:** High

### SPEC-0106/E2E-064: empty extensions means the built in list
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_064_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012, FR-NEW-004
- **Given** the converter of `proj` has no extensions
- **When** Bob converts `/a.pdf`, `/b.docx` and `/c.xyz`
- **Then** `/a.pdf` and `/b.docx` are converted, `/c.xyz` is refused with the list of the built in extensions, each with its leading dot, in their built in order
- **Priority:** Medium

### SPEC-0106/E2E-065: custom extensions
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_065_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012, FR-NEW-004
- **Given** the converter of `proj` has `extensions` `["txt"]`
- **When** Bob converts `/a.txt` then `/a.pdf`
- **Then** `/a.txt` is converted; `/a.pdf` is refused with `'/a.pdf' cannot be documented, the document service accepts: .txt`
- **Priority:** Medium

### SPEC-0106/E2E-111: an extension matches the end of the name after a dot
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_111_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012, FR-NEW-004
- **Given** the converter of `proj` has `extensions` `["txt"]`
- **When** Bob converts `/notxt` and `/Notes.TXT`
- **Then** `/notxt` is refused with `'/notxt' cannot be documented, the document service accepts: .txt`; `/Notes.TXT` is converted
- **Priority:** Medium

### SPEC-0106/E2E-113: a document over the cap is refused before any write
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_113_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-NEW-013
- **Given** the converter of `proj` has `max_input_bytes` 1000
- **When** Bob writes a 1001 byte `/report.pdf` with the conversion option
- **Then** he is refused with `ERR_INVALID_ARGUMENT: '/report.pdf' is 1001 bytes, over the doc_service.max_input_bytes limit of 1000` and nothing is written
- **Priority:** High

### SPEC-0106/E2E-114: a stored secret that can no longer be read
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_114_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret and is the converter of `proj`; the platform restarts with another master key
- **When** Bob writes `/report.pdf` with the conversion option
- **Then** he is refused with `ERR_INTERNAL_ERROR: the secret of converter 'Docling API' cannot be read` (his file is accepted by the extension and size checks), nothing is written, no other converter is tried and the text holds no secret
- **Priority:** High

### SPEC-0106/E2E-129: batch upload resolves the converter per file
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_129_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-011
- **Given** Bob uploads 2 files with the conversion option and Alice changes the default between the two conversions
- **When** the upload runs
- **Then** each file is converted by the converter chosen when its own conversion starts
- **Priority:** Low

### SPEC-0106/E2E-130: a change between two files of a batch is applied per file
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_130_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** Bob uploads `/a.pdf` and `/b.pdf` with the conversion option; the batch check passes, then Alice sets as default a converter that accepts only `txt` before the second conversion starts
- **When** the upload runs
- **Then** `/a.pdf` is converted and stays; the upload stops with `'/b.pdf' cannot be documented, the document service accepts: .txt` and `/b.pdf` is not written, as for any refusal during a batch today
- **Priority:** Medium

### SPEC-0106/E2E-131: an extension refusal comes before an unreadable secret
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_131_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret that can no longer be read and accepts `pdf` only; it is the converter of `proj`
- **When** Bob writes `/c.xyz` with the conversion option
- **Then** he is refused with `'/c.xyz' cannot be documented, the document service accepts: .pdf`, not with the secret text
- **Priority:** Medium

### SPEC-0106/E2E-132: a size refusal comes before an unreadable secret
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_132_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-004
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has `max_input_bytes` 1000 and a secret that can no longer be read; it is the converter of `proj`
- **When** Bob writes a 1001 byte `/report.pdf` with the conversion option
- **Then** he is refused with `ERR_INVALID_ARGUMENT: '/report.pdf' is 1001 bytes, over the doc_service.max_input_bytes limit of 1000`
- **Priority:** Medium

### SPEC-0106/E2E-148: the converter disappears between two files of a batch
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_148_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** Bob uploads `/a.pdf` and `/b.pdf` with the conversion option; after the batch check Alice clears the only converter before the second conversion starts
- **When** the upload runs
- **Then** `/a.pdf` is converted and stays; the upload stops with `ERR_NOT_SUPPORTED: document service is not configured` and `/b.pdf` is not written
- **Priority:** Low

### SPEC-0106/E2E-076: the answer carries the full document metadata through a database converter
- Level / Driver / Location: functional e2e / the real McpServer tool methods, the real router and the real screen router, with stand in converters / `crates/core/src/settings/converters_e2e.rs` (`e2e_0106_076_*`)
- **Scenario:** SC-004
- **Requirements:** FR-NEW-013
- **Given** an http converter returns a version 2 manifest with one picture and OCR text `Revenue`
- **When** Bob converts `/report.pdf`
- **Then** the image list shows the picture with OCR text `Revenue`, as for a converter of the configuration file
- **Priority:** Medium

## Constraints
- A refusal leaves nothing written.
- Do not change the batch pre check (converter and extension only).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the existing tests of `docs::`, `tools::artifacts` and `tools::converter_bound` keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0321.
- [ ] Self review: every requirement above is met by a named test; every report line text equals the spec; a converter supplied value never reaches a log, a file name or a path; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0316: extensions, size cap, unreadable secret and batch behavior` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0106` and the requirement ids.
