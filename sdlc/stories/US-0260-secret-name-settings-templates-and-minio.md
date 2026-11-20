# US-0260: secret name settings, templates and MinIO variables

> Spec: SPEC-0099 (sdlc/specs/SPEC-0099-rename-the-project-to-fsa/)
> Nature: DEBT
> Priority: P1
> Depends on: US-0259
> min_tier: 2
> Files touched: 10
> Closes spec: no

## Objective
Move the two secret name settings, the OCR key variable and the GitHub client secret variable, to the `FSA_` defaults with a fallback to the old defaults when the key is absent from the configuration, and rename the template placeholders and the MinIO, public URL, DSN and doc service token variables.

## Requirements covered
- SPEC-0099/DR-003 (partial: the defaults `FSA_OCR_KEY` and `FSA_GITHUB_CLIENT_SECRET`, the template placeholders `${FSA_MINIO_ENDPOINT}`, `${FSA_MINIO_ACCESS_KEY}`, `${FSA_MINIO_SECRET_KEY}`, `${FSA_DOC_SERVICE_TOKEN}`, `${FSA_META_DSN}`, `${FSA_PUBLIC_URL}` and `FSA_EMBEDDING_KEY`, `FSA_RERANK_KEY` in examples): The server SHALL read these environment variables under the `FSA_` prefix.
- SPEC-0099/DR-007: WHEN the setting `git.github_client_secret_env` or `extract.ocr.api_key_env` is absent from the configuration, the new default variable is unset and the old default variable is set THE server SHALL use the old variable's value and print the DR-006 line for it.
- SPEC-0099/DR-005 (partial: only the variable names and their fallback change): The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.

## Independent Test
DT-002 passes: with `git.github_client_secret_env` absent, `FSA_GITHUB_CLIENT_SECRET` unset and `MCPFS_GITHUB_CLIENT_SECRET=old`, the secret read is `old` with the DR-006 warning; written explicitly (even as the default name) it is used as written with no fallback; same for the OCR key.

## Design excerpt
### Decisions
- DEC-003 (SPEC-0099): dual maintenance of the six environment variables and of `~/.config/mcp-fs`, removal at release 1.0.0. **Alternatives considered:** break at once (rejected: the default for a public contract is dual maintenance), warn on every read (rejected: noise). **Implemented by:** DR-006, DR-007, DR-008.
- DEC-011 (SPEC-0099): the two secret name settings (`extract.ocr.api_key_env`, `git.github_client_secret_env`) keep an absent or written distinction in the configuration, so a written value equal to a default name is not mistaken for an absent one. **Form:** both fields become `Option<String>` with default `None`; the `Debug` snapshots print `api_key_env: None` and `github_client_secret_env: None`, and the assertion near `config.rs:1294` becomes `is_none()`. **Alternatives considered:** compare the value to the default string (rejected: it cannot tell "written" from "absent", two agents would differ for `api_key_env: FSA_OCR_KEY`). **Implemented by:** DR-007. **Evidence:** the defaults are applied at deserialization, `config.rs:452`, `config.rs:503`, and read as plain strings at `git/oauth/device_flow.rs:280` and `docs/ocr.rs:83`.
- DEC-013 (SPEC-0099): one module `crate::env_compat` reads a variable by its new name then its old name, with a process wide set of names already warned, a function to read a name given an environment closure (so `resolve_config_path_with` stays testable) and a test only reset. Every reader in DR-003 goes through it. **Obliged by:** DR-006, DR-007. **Rationale:** one place owns precedence, blank handling and the warning text, so DT-001 tests one implementation. **Alternatives considered:** a fallback at each of the six call sites (rejected: six copies of the warning and the blank rule). **Source:** design. **Evidence:** the readers at `cli.rs:404-405`, `git/oauth/store.rs:112`, `git/oauth/device_flow.rs:280`, `docs/ocr.rs:83`.
- DEC-014 (SPEC-0099): the two secret name accessors keep returning `String`, so `device_flow.rs` and `ocr.rs` call sites do not change shape; the `Option` lives in the configuration struct only (DEC-011). **Obliged by:** DR-007. **Rationale:** smallest change at the call sites. **Alternatives considered:** `Option` returned to callers (rejected: spreads the fallback to every call site). **Source:** design. **Evidence:** `device_flow.rs:280`, `docs/ocr.rs:83`.

### Data model
None.

### Contracts
Config keys unchanged: `extract.ocr.api_key_env` and `git.github_client_secret_env`; only their default and their absent versus written meaning change. `FSA_MINIO_*`, `FSA_META_DSN`, `FSA_PUBLIC_URL`, `FSA_DOC_SERVICE_TOKEN` have no fallback.

### Paths
| Path | Action |
|---|---|
| `crates/core/src/config.rs:452,503`, `OcrConfig` and `GitConfig` | `api_key_env` and `github_client_secret_env` become `Option<String>` with default `None`; accessors return `String` (DEC-014): written value, else `FSA_OCR_KEY` or `FSA_GITHUB_CLIENT_SECRET`, with the old default as a fallback through the helper |
| `crates/core/src/git/oauth/device_flow.rs:280,422`, `crates/core/src/docs/ocr.rs:83,163` | call the accessors; struct literals become `Some(..)` |
| `config/minio.yaml.template:26-28`, `config/local.yaml.template:71` | live `${FSA_...}` placeholders |
| `crates/core/src/config.rs:1724-1726,1898-1899` tests, `crates/core/src/storage/blob/s3.rs:269`, `crates/core/tests/full_stack_e2e.rs:83,88,90` | `FSA_MINIO_*`, `FSA_PUBLIC_URL` |
| `Makefile:20`, `.env.example` | `FSA_MINIO_*` |
| `.agent_docs/search.md:18,25` | `FSA_EMBEDDING_KEY`, `FSA_RERANK_KEY` in the examples |
| `crates/fsa/tests/env_compat_e2e.rs` | add DT-002 |

### Existing patterns
The defaults are applied at deserialization by `fn d_ocr_key_env()` (`config.rs:452`) and `fn d_github_secret_env()` (`config.rs:503`), read as plain strings at `docs/ocr.rs:83` and `device_flow.rs:280`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

The success criterion of the debt, restated: the existing suite passes unmodified except the rename edits declared for this story: `config.rs:1732-1733` snapshots and the assertion near `:1294` (absent form), `config.rs:1724-1726` and `:1898-1899` (variable names), `device_flow.rs:422` and `docs/ocr.rs:163` (`Some(..)`), every other literal of `OcrConfig` or `GitConfig` setting the two fields (`Some(..)`), `s3.rs:269` and `full_stack_e2e.rs:83,88,90` (`FSA_MINIO_*`) (DR-003, DR-007, DEC-011). Command: `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` (the container and the developer variable keep their names, DEC-005).

### SPEC-0099/DT-002: old default secret variable falls back
- Level / Driver / Location: functional e2e / `crates/fsa/tests/env_compat_e2e.rs` and the unit tests of `config.rs`
- **Validates:** DR-007. Given a configuration without `git.github_client_secret_env`, `FSA_GITHUB_CLIENT_SECRET` unset, `MCPFS_GITHUB_CLIENT_SECRET=old`; Then the secret read is `old` with the DR-006 line. Same for `extract.ocr.api_key_env` and `MCP_FS_OCR_KEY` then `FSA_OCR_KEY`. Given the setting written as `MY_SECRET`; Then only `MY_SECRET` is read and no old name is consulted. Given `api_key_env: FSA_OCR_KEY` written explicitly, `FSA_OCR_KEY` unset and `MCP_FS_OCR_KEY=old`; Then the key read is empty and standard error holds no warning.


## Constraints
- The `Debug` snapshots of the default config print `api_key_env: None` and `github_client_secret_env: None`; update `LOCAL_TEMPLATE_SNAPSHOT` and `MINIO_TEMPLATE_SNAPSHOT` (`config.rs:1732-1733`) and the assertion near `config.rs:1294` accordingly.
- Do not rename `MCPFS_TEST_*` and the other developer variables (DEC-005).
- No fallback for the MinIO, DSN, public URL and doc service token variables.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (a test that pins behavior which already holds says so instead).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: every test of the suite not named in the exceptions passes unchanged; `tool-contract-golden.json` schemas and descriptions do not change.
- [ ] DEBT: the existing suite passes unmodified, exceptions: `config.rs:1732-1733` snapshots and the assertion near `:1294` (absent form), `config.rs:1724-1726` and `:1898-1899` (variable names), `device_flow.rs:422` and `docs/ocr.rs:163` (`Some(..)`), every other literal of `OcrConfig` or `GitConfig` setting the two fields (`Some(..)`), `s3.rs:269` and `full_stack_e2e.rs:83,88,90` (`FSA_MINIO_*`) (DR-003, DR-007, DEC-011) (Section 8.4 of the design).
- [ ] Docs updated: prose that names what this story renames is left to US-0263, except where a documented command stops working (then fixed here).
- [ ] Self review (full): the diff renames only what the requirements name, the retained identifiers of Section 5 are untouched, no behavior moved.
- [ ] One commit `US-0260: secret name settings, templates and MinIO variables` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0099` and the requirement ids.
