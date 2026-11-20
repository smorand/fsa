# Rename the project from mcp-fs to FSA: technical debt design

> Id: SPEC-0099
> Nature: DEBT
> Depth: L
> Depth evidence: 4 areas (Rust packages, advertised names, environment and configuration contract, scripts and docs), 94 files and 655 occurrences, public contracts changed
> Status: implemented
> Generated on: 2026-11-19
> Behavior change: BREAKING on the binary name and the MCP server name, see Section 7
> From backlog: BL-0163
> Depends on: none
> Security: n/a
> CVSS: n/a
> Affected: n/a
> Fixed in: n/a

## 1. Debt
### DBT-001: the product carries the name of one transport
- **Current structure:** the product is named `mcp-fs` in its packages (`crates/mcp-fs/Cargo.toml:2`, `crates/core/Cargo.toml:2`), its binary (`crates/mcp-fs/Cargo.toml:12`), the name it advertises to MCP clients (`crates/core/src/mcp/server.rs:39`), its command line (`crates/core/src/cli.rs:43`), its documents and its environment variables (`cli.rs:19-23`, `config.rs:453`, `config.rs:504`, `git/oauth/store.rs:26`).
- **Cost paid today:** the name says MCP while the product is reached by MCP, REST and a web app; the owner decided on 2026-11-19 that the product is FSA (File System Agentic). Every later spec of the FSA roadmap (BL-0164 to BL-0173) would otherwise be written under the old name.
- **Target structure:** the product, its binary and its packages are `fsa`; what clients see is FSA; the old environment variables and the old user configuration directory keep working for a deprecation window; identifiers stored in existing deployments keep their old spelling.
- **Observable behavior:** unchanged, except the declared breaks of Section 7.

## 2. Perimeter: the Current State (MANDATORY)
Recounted on 2026-11-19 with `rg -i -c 'mcp[-_ ]?fs' --hidden` excluding `sdlc/`, `plan/`, `target/`, `.git/`, `.implement/` and `Cargo.lock`: **94 files, 655 occurrences**.

| Group | Files | Occurrences | Main files (occurrences) |
|---|---|---|---|
| Rust source and inline tests | 52 | 328 | `crates/core/src/cli.rs` 45, `config.rs` 38, `storage/blob/s3.rs` 16, `tools/git.rs` 15, `search/bm25_pg.rs` 14, `search/vector_pg.rs` 13, `git/http/mod.rs` 13 |
| Rust integration tests (`tests/` directories) | 5 | 51 | `crates/core/tests/full_stack_e2e.rs` 20, `crates/mcp-fs/tests/sql_server_removed.rs` 13, `crates/core/tests/git_negotiation_e2e.rs` 10, `crates/core/tests/schema_golden.rs` 5, `crates/mcp-fs/tests/crate_split.rs` 3 |
| Docs and text | 14 | 182 | `FUNCTIONAL_TESTS.md` 47, `.agent_docs/testing.md` 43, `README.md` 28, `.agent_docs/config.md` 22, `AGENTS.md` 16, `specs/AUDIT.md` 1, `TOOL_CONTRACT.txt` 1 |
| Scripts and tooling | 12 | 56 | `agent.sh` 11, `tests/functional/**` 26 over 5 files, `docker-compose.test.yml` 6, `config/agent_test.yaml` is counted under config files |
| Manifests | 5 | 14 | `crates/mcp-fs/Cargo.toml` 7, `crates/core/Cargo.toml` 3, `Cargo.toml` 2, `crates/agent/Cargo.toml` 1, `deny.toml` 1 |
| Config files | 3 | 15 | `config/minio.yaml.template` 7, `config/agent_test.yaml` 5, `config/local.yaml.template` 3 |
| Dotfiles | 2 | 8 | `.env.example` 7, `.gitignore` 1 |
| Golden json | 1 | 1 | `tool-contract-golden.json` (the regenerate hint, line 3) |

Of the 655, 48 are the lowercase shell helper `mcp fs.<tool>` (`FUNCTIONAL_TESTS.md` 40, two functional scripts 8), which is retained (DR-010).

**By form** (same exclusions, `rg -F -c`): `mcp-fs-core` 13 files 53; `mcp_fs_core` 9 files 23; `crates/mcp-fs` 9 files 16; `mcp-fs-rust` 2 files 2 (`Cargo.toml:10`, `README.md:43`); `MCPFS_` 40 files 191; `MCP_FS_` 6 files 20; lowercase `mcpfs` 38 files 133; the word `mcp-fs` 60 files 261.

**Non source occurrences:** `TOOL_CONTRACT.txt:727` (the data value `"project_id": "mcp-fs"` of a recorded example, renamed in step 5), `specs/AUDIT.md:36` (`mcp-fs keys`, `mcp-fs token`, renamed in step 5), `.env.example` 7, `.gitignore` 1, `.agent_docs/*.md` 88 over 8 files, `docker-compose.test.yml` 6 (comment and the test database name `mcpfs`), `deny.toml:31` (comment), `specs/audit.py` 3 (hardcodes `crates/mcp-fs/src`), `tests/functional/**` 26 over 5 files. No Dockerfile, no `.github/`, no `.pre-commit-config.yaml`, no launchd or systemd unit, no submodule exist. History folders `sdlc/` (328 files) and `plan/` (7 files) are not touched.

**Outside this repository** (listed, not edited by this entry): `os-config/kitty/tabs.json:26` (working directory of a terminal tab), `os-config/bash/bashrc:555-556` (exports `MCPFS_TOKEN_KEY`), `mcp-exec/backlog/BL-0004_mcp-fs-artifact-storage-integration.md:11`, `config-a2a/scenarios/client.py:32`. No MCP client registration naming `mcp-fs` exists under `~/.pi/agent` or `~/.config`.

**Contracts touched:**
| Contract | Internal or public | Caller |
|---|---|---|
| binary `mcp-fs` and the `-p mcp-fs` package | public | operators, `run.sh`, `agent.sh`, `test.sh`, functional scripts |
| MCP `serverInfo.name` `"mcp-fs"` (`crates/core/src/mcp/server.rs:39`, asserted `crates/core/src/app.rs:577`) | public | every MCP client |
| environment `MCP_FS_CONFIG`, `MCP_FS_CONFIG_DIR`, `MCP_FS_CONFIG_NAME` (`cli.rs:19-23`) | public | operators |
| environment `MCPFS_TOKEN_KEY` (`git/oauth/store.rs:26`, read `store.rs:112`), default names `MCPFS_GITHUB_CLIENT_SECRET` (`config.rs:504`), `MCP_FS_OCR_KEY` (`config.rs:453`) | public | operators |
| user configuration directory `~/.config/mcp-fs/config.yaml` (`cli.rs:30,37`) | public | operators |
| git `agent=mcp-fs/0.1.0` and `realm="mcp-fs"` (`git/http/mod.rs:68,70,256`) | public (wire) | git clients |
| the author written by the test helper `seed_commit` (`git/odb.rs:528-529`) | internal | tests |
| OpenAPI title `mcp-fs REST API` and page title `mcp-fs API` (`api/openapi.rs:91,1580`) | public | API readers |
| bucket prefix `mcpfs-` (`config.rs:244`), trash directory `.mcp_trash` (`config.rs:364`), refs `refs/mcpfs/` (`git/remote.rs:1453,1456`), advisory lock key `mcpfs_migrate:` (`storage/rel/postgres.rs:386`), cookie `mcpfs_token` (`trash_screen.rs:245`, `deleted_projects_screen.rs:212`) | persisted data or external | existing deployments |

**Guardrail coverage:** 4037 tests in `mcp-fs-core` lib plus 113 in `agent`, 21 `security_gate`, 16 `sql_server_removed`, 3 `makefile_targets` and 1 `crate_split` (`make check` of 2026-11-19: 4483 passed, 0 failed). **Test command:** `MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` with `docker start mcp-fs-rust-postgres-1`.
**Total:** 94 files, 655 occurrences, all recounted.

## 3. Class sweep
Not applicable (`Security: n/a`).

## 4. Requirements (EARS)
### 4.1 Structural
#### DR-001 [EARS-U]: packages, binary and crate
> The workspace SHALL name its application package and binary `fsa`, its library package `fsa-core` with library crate `fsa_core`, in directory `crates/fsa` for the application, and SHALL keep the directory `crates/core` and the package `agent`.
- **Old form:** package and binary `mcp-fs` in `crates/mcp-fs`; package `mcp-fs-core`; crate `mcp_fs_core`.
- **New form:** package and binary `fsa` in `crates/fsa`; package `fsa-core`; crate `fsa_core`; features `postgres`, `rag`, `all-backends`, `rar` forwarded unchanged.
- **Applies to:** manifests, `use` paths, `-p` arguments, binary paths in `Makefile` (line 23), `run.sh`, `agent.sh`, `test.sh`, `build.sh`, the functional scripts and the tests, `deny.toml`, the `docker-compose.test.yml` comment, `specs/audit.py:29,164,171`, `scripts/pty_check.py:136,151,157,266`, `scripts/doc_service_fake.py:199`.
- **Casing convention:** kebab case for packages and directories, snake case for the crate, as in the codebase today.

#### DR-002 [EARS-U]: names advertised to clients
> The server SHALL use the new names of the list below in every client facing name.
- **Old form to new form, exactly:** MCP `serverInfo.name` `mcp-fs` to `fsa`; OpenAPI title `mcp-fs REST API` to `FSA REST API`; Swagger page title `mcp-fs API` to `FSA API`; clap command name `mcp-fs` to `fsa`; startup line `Serving mcp-fs {} on` to `Serving fsa {} on`; the `ABOUT` text begins `fsa:`; git `agent=mcp-fs/0.1.0` to `agent=fsa/0.1.0`; `WWW-Authenticate` realm `mcp-fs` to `fsa`; the author written by the `seed_commit` helper `mcp-fs <mcp-fs@example.test>` to `fsa <fsa@example.test>` (`git/odb.rs:528-529`, a `pub` helper called only by tests today); the OpenAPI tag `mcp-fs` to `fsa` (`api/openapi.rs:110,211,281`); the Context7 header `X-Context7-Source: mcp-fs` to `fsa` (`tools/context7.rs:104,146`); the MCP client name sent by the agent `mcp-fs-agent` to `fsa-agent` (`crates/agent/src/mcp.rs:155`), the agent about text and its hint `mcp-fs token` to `fsa token` (`crates/agent/src/main.rs:27,433`); the S3 provider name `mcp-fs-config` to `fsa-config` (`storage/blob/s3.rs:66`); the git reflog messages that begin `mcp-fs ` to `fsa ` for new entries (`tools/git_tags.rs:727,1212`, `tools/git.rs:2399,3734`); outbound `User-Agent` `mcp-fs/<version>` to `fsa/<version>` (`git/provider/mod.rs:37`, `git/oauth/device_flow.rs:29`), `mcp-fs-doc-service/1.0` to `fsa-doc-service/1.0` (`docs/service.rs:534`), `mcp-fs-web/1.0` to `fsa-web/1.0` (`tools/web.rs:302`), `mcp-fs/1.0` to `fsa/1.0` (`tools/context7.rs:90`).
- **Applies to:** the files listed by `rg 'mcp-fs' crates/*/src`.

#### DR-003 [EARS-U]: environment variable names
> The server SHALL read these environment variables under the `FSA_` prefix.
- **Old form to new form, exactly:** `MCP_FS_CONFIG` to `FSA_CONFIG`; `MCP_FS_CONFIG_DIR` to `FSA_CONFIG_DIR`; `MCP_FS_CONFIG_NAME` to `FSA_CONFIG_NAME`; `MCPFS_TOKEN_KEY` to `FSA_TOKEN_KEY`; the default of `extract.ocr.api_key_env` `MCP_FS_OCR_KEY` to `FSA_OCR_KEY`; the default of `git.github_client_secret_env` `MCPFS_GITHUB_CLIENT_SECRET` to `FSA_GITHUB_CLIENT_SECRET`. The shipped templates and documented examples rename `${MCPFS_MINIO_ENDPOINT}`, `${MCPFS_MINIO_ACCESS_KEY}`, `${MCPFS_MINIO_SECRET_KEY}`, `${MCPFS_DOC_SERVICE_TOKEN}`, `${MCPFS_META_DSN}` and `${MCPFS_PUBLIC_URL}` to `${FSA_...}` with the same suffix; the documented examples `MCPFS_EMBEDDING_KEY` and `MCPFS_RERANK_KEY` (`.agent_docs/search.md:18,25`) to `FSA_EMBEDDING_KEY` and `FSA_RERANK_KEY`. The variables `MCPFS_MINIO_ENDPOINT`, `MCPFS_MINIO_ACCESS_KEY` and `MCPFS_MINIO_SECRET_KEY` read directly by tests (`storage/blob/s3.rs:269` read, `:455` doc comment, bucket names from `:459`; `crates/core/tests/full_stack_e2e.rs:83-84` read, `:28` doc comment) and named in `Makefile:20` and `.env.example` become `FSA_MINIO_ENDPOINT`, `FSA_MINIO_ACCESS_KEY` and `FSA_MINIO_SECRET_KEY`, with no fallback (read by developer tests and operator templates only; a test that finds the variable missing fails loudly).
- **Casing convention:** upper snake case, as today.

#### DR-004 [EARS-UB]: persisted and developer identifiers keep their spelling
> The server SHALL NOT rename the identifiers listed in Section 5 as retained.

#### DR-010 [EARS-U]: other spellings of the old name
> The repository SHALL rename the in memory PDF marker names `McpFsObjStm`, `McpFsFileType` and `McpFsFilter` (`docs/pdf_bound.rs:46,50,58`) to `FsaObjStm`, `FsaFileType` and `FsaFilter`, and every lowercase `mcpfs` or `mcp_fs` in test code, except the retained literals of Section 5, to `fsa` keeping the rest of the string (`mcpfs-test` to `fsa-test`, `mcpfs_e2e_` to `fsa_e2e_`, `mcp_fs_2` at `storage/rel/postgres.rs:505` to `fsa_2`, the fixtures at `docs/service.rs:1436` (keep the fixture's `/Length` equal to its stream length), `tools/git.rs:9817`, `cli.rs:548,554`). The repository SHALL also rename, in test code (`#[cfg(test)]` modules and `tests/` directories), every remaining `mcp-fs` to `fsa` and `MCPFS_PUBLIC_URL` to `FSA_PUBLIC_URL`, keeping the rest of the string: `Cli::parse_from(["mcp-fs", ...])` becomes `["fsa", ...]`, `@mcp-fs.test` becomes `@fsa.test`, `mcp-fs-fetch-unreachable-test.invalid` becomes `fsa-fetch-unreachable-test.invalid`, the function name `mcp_fs_src_contains_only_main_rs` (`crates/mcp-fs/tests/crate_split.rs:8`) becomes `fsa_src_contains_only_main_rs`, `CARGO_BIN_EXE_mcp-fs` becomes `CARGO_BIN_EXE_fsa`; the retained spellings of Section 5 are excepted. A lowercase `mcpfs` is retained only when it spells the bucket prefix `mcpfs-` and the assertions on it, the cookie `mcpfs_token`, the DSN segments `mcpfs:mcpfs@.../mcpfs` and the compose test database, the schema `mcpfs` asserted as a valid name (`postgres.rs:505`), `refs/mcpfs/` or `mcpfs_migrate:`.
- **Retained:** the doc comment at `git/mod.rs:5` that cites the C# path `src/McpFs/Git/*` (lineage, `.agent_docs/lineage.md`), and the shell helper `mcp fs.<tool>` in `FUNCTIONAL_TESTS.md` and `tests/functional/` (48 hits of lowercase `mcp fs`: the command `mcp` followed by a tool name, not the product name).

### 4.2 Invariance (MANDATORY)
#### DR-005 [EARS-U]: behavior invariance
> The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.

### 4.3 Compatibility
#### DR-006 [EARS-E]: old environment variable names keep working
> WHEN one of the old variables `MCP_FS_CONFIG`, `MCP_FS_CONFIG_DIR`, `MCP_FS_CONFIG_NAME`, `MCPFS_TOKEN_KEY` is set and its new variable is unset THE server SHALL use the old value, and SHALL print on standard error, once per process and per variable, the line `warning: environment variable <OLD> is deprecated, use <NEW>`.
- **Precedence:** the new variable wins when both are set; the warning is still printed for the old one.
- **Blank values:** a variable whose value is empty or only whitespace is unset for DR-006, DR-007 and DR-008, as today (`cli.rs:536-552`, `git/oauth/store.rs:113`): a blank `FSA_X` falls back to a set `MCP_FS_X` with the DR-006 warning, and a blank old variable prints no warning.

#### DR-007 [EARS-E]: old default secret names keep working
> WHEN the setting `git.github_client_secret_env` or `extract.ocr.api_key_env` is absent from the configuration, the new default variable is unset and the old default variable is set THE server SHALL use the old variable's value and print the DR-006 line for it.
- **Business rule:** a configuration that writes either setting, including a value equal to a default name (`api_key_env: FSA_OCR_KEY`), is used as written, with no fallback and no warning. Only an absent key triggers the fallback. The two settings therefore SHALL be distinguishable as absent or written (DEC-011).

#### DR-008 [EARS-E]: the old user configuration directory keeps working
> WHEN no file exists at `~/.config/fsa/config.yaml` and a file exists at `~/.config/mcp-fs/config.yaml` THE server SHALL load the second one and print `warning: configuration directory <HOME>/.config/mcp-fs is deprecated, move it to <HOME>/.config/fsa`.
- **Business rule:** `$XDG_CONFIG_HOME` replaces `<HOME>/.config` exactly as it does today.
- **Exact order:** (1) `--config`; (2) `FSA_CONFIG`, else `MCP_FS_CONFIG` with the DR-006 warning; (3) `<base>/fsa/config.yaml`; (4) `<base>/mcp-fs/config.yaml` with the DR-008 warning; (5) `<dir>/<name>.yaml` where `<dir>` is `FSA_CONFIG_DIR`, else `MCP_FS_CONFIG_DIR` with the DR-006 warning, else `config`, and `<name>` is `FSA_CONFIG_NAME`, else `MCP_FS_CONFIG_NAME` with the DR-006 warning, else `local`. The tried list of the boot error is the candidates of (3), (4), (5) in that order, and its text says `set FSA_CONFIG`.

#### DR-009 [EARS-UB]: no old name is left behind
> The repository SHALL NOT contain, case sensitive, `mcp-fs`, `mcp_fs`, `MCP-FS`, `MCP FS`, `MCPFS_`, `MCP_FS_`, `McpFs` or `mcpfs` outside the retained identifiers of Section 5 and DR-010, the fallback code of DR-006 to DR-008 with its tests, and the history folders `sdlc/` and `plan/`.

## 5. Compatibility plan
| Contract | Kind | Public | Plan | Deprecation window | Removal condition |
|---|---|---|---|---|---|
| binary and package `mcp-fs` | rename | yes | break, no shim (DEC-006) | none | n/a |
| MCP `serverInfo.name` | rename | yes | break, no alias possible (one value) | none | n/a |
| six environment variables (DR-003) | rename | yes | dual maintenance (DR-006, DR-007) | until release 1.0.0 | the 1.0.0 release deletes the fallback and its tests |
| `~/.config/mcp-fs` | rename | yes | dual maintenance (DR-008) | until release 1.0.0 | same |
| git `agent`, `realm` | rename | yes (wire) | break | none | n/a |
| OpenAPI and page titles, screens, banner | rename | yes | break | none | n/a |
| **Retained** (not renamed): bucket prefix `mcpfs-`, trash directory `.mcp_trash`, refs `refs/mcpfs/`, ref `refs/mcp-fs/rebase-head` (`tools/git.rs:4535`, tests `:27633,27638`), stash trailer `mcp-fs-stash: ` (`tools/git.rs:7151`), advisory lock key `mcpfs_migrate:`, cookie `mcpfs_token`, DSN segments and the compose test database, user and password `mcpfs` (`docker-compose.test.yml:4,23-30`), schema name `mcpfs`, existing commit objects and reflog entries | persisted or external | yes | keep | until BL-0173 | revisit with BL-0173 |
| **Retained spellings**: the C# lineage comment `src/McpFs/Git/*` (`git/mod.rs:5`), the shell helper `mcp fs.<tool>` | documentation | no | keep (DR-010) | none | none |
| **Retained developer variables**: `MCPFS_TEST_*`, `MCPFS_REWRITE_*`, `MCPFS_CHILD_CASE*`, `MCPFS_PRINT_RECORD` | tooling | no | keep (DEC-005) | until the owner's shell is updated | a later cleanup |

**Dual maintenance semantics:** precedence new over old; the warning line is exactly the text of DR-006 or DR-008, on standard error, once per process and per variable (once per process for the directory); emitted at the first read.

## 6. Decisions
- **DEC-001:** names are `fsa`, `fsa-core`, `fsa_core`; the directory `crates/mcp-fs` becomes `crates/fsa`; `crates/core` and the package `agent` keep their names (the package is a client of the server and `agent` carries no product name; its strings that name the product are renamed by DR-002). **Alternatives considered:** keep the directory name `crates/mcp-fs` (rejected: the old name would survive in a path), rename `crates/core` (rejected: the directory name was chosen for the skill layout, `AGENTS.md` Overview). **Rationale:** smallest set of renames that leaves no old name. **Implemented by:** DR-001. **Evidence:** `Cargo.toml:3`, `crates/core/Cargo.toml:2,12`.
- **DEC-002:** stepwise migration, one green step at a time (Section 9), never one global replace. **Alternatives considered:** one global replacement (rejected: it would also rename the retained identifiers and break bisecting), a second crate keeping the old names for a cycle (rejected: duplicates the workspace). **Implemented by:** DR-001 to DR-009.
- **DEC-003:** dual maintenance of the six environment variables and of `~/.config/mcp-fs`, removal at release 1.0.0. **Alternatives considered:** break at once (rejected: the default for a public contract is dual maintenance), warn on every read (rejected: noise). **Implemented by:** DR-006, DR-007, DR-008.
- **DEC-004:** identifiers stored in existing deployments keep their old spelling. **Rationale:** renaming the bucket prefix orphans buckets, renaming `.mcp_trash` loses trashed files, renaming the refs and the lock key breaks mixed fleets. **Alternatives considered:** migrate them (rejected: irreversible data change, no gain). **Implemented by:** DR-004. **Evidence:** `config.rs:244,364`, `git/remote.rs:1453,1456`, `storage/rel/postgres.rs:386`, `tools/git.rs:4535,7151` (an in flight rebase and existing stashes would be orphaned).
- **DEC-005:** `MCPFS_TEST_*`, `MCPFS_REWRITE_*`, `MCPFS_CHILD_CASE*`, `MCPFS_PRINT_RECORD` keep their names for now. **Rationale:** a test that reads `MCPFS_TEST_PG_DSN` skips silently when it is unset (`.agent_docs/testing.md`, "opt in suites"), so renaming it while the owner's shell exports the old name would turn the PostgreSQL suites green by skipping. **Alternatives considered:** rename with a fallback in 56 places (rejected: cost, no user value). **Exit condition:** rename together with the shell update, in a later cleanup. **Implemented by:** DR-004.
- **DEC-006:** no shim binary named `mcp-fs`. **Alternatives considered:** a second `[[bin]]` forwarding to `fsa` (rejected: the repository ships no install target and nothing in `~/.pi/agent` or `~/.config` calls the binary). **Authorized by:** the owner's rename request of 2026-11-19. **Implemented by:** DR-001, Section 7.
- **DEC-007:** `serverInfo.name`, git `agent`, `realm`, the `seed_commit` author, the outbound `User-Agent` strings, the OpenAPI tag, the Context7 header, the agent client name and the S3 provider name are renamed without alias. **Alternatives considered:** keep the old wire names (rejected: the product name would stay visible to every client). **Authorized by:** the same request. **Implemented by:** DR-002, Section 7.
- **DEC-008:** the history folders `sdlc/` and `plan/` keep the old name. **Rationale:** they are the record of what was decided and shipped. **Implemented by:** DR-009.
- **DEC-009:** the actions outside the repository (GitHub repository rename, remote URL, local folder move, the other repositories listed in Section 2) are an operator runbook (Section 9), run only on the owner's yes, never by a story. **Implemented by:** n/a, no code impact (operational).
- **DEC-010:** the version bump is decided at landing: the rule gives a PATCH for a DEBT; Section 7 lists breaks, so the landing report flags "breaking change, consider `--major`". **Implemented by:** n/a, no code impact.
- **DEC-011:** the two secret name settings (`extract.ocr.api_key_env`, `git.github_client_secret_env`) keep an absent or written distinction in the configuration, so a written value equal to a default name is not mistaken for an absent one. **Form:** both fields become `Option<String>` with default `None`; the `Debug` snapshots print `api_key_env: None` and `github_client_secret_env: None`, and the assertion near `config.rs:1294` becomes `is_none()`. **Alternatives considered:** compare the value to the default string (rejected: it cannot tell "written" from "absent", two agents would differ for `api_key_env: FSA_OCR_KEY`). **Implemented by:** DR-007. **Evidence:** the defaults are applied at deserialization, `config.rs:452`, `config.rs:503`, and read as plain strings at `git/oauth/device_flow.rs:280` and `docs/ocr.rs:83`.
- **Assumption:** the cookie `mcpfs_token` is set outside this repository (ASSUMED: no place in `crates/` sets it; it is only read at `trash_screen.rs:245` and `deleted_projects_screen.rs:212`), so it is retained.

## 7. Declared breaks
| Contract | Who is affected | What they must do | Authorized by |
|---|---|---|---|
| binary and package `mcp-fs` | anyone running `cargo run -p mcp-fs` or `target/release/mcp-fs` | use `fsa` | DEC-006 |
| MCP `serverInfo.name` | an MCP client that keys on the name `mcp-fs` | none for clients that register by URL; others update the key | DEC-007 |
| git `agent`, `realm` | git clients logging the server agent; readers of new commit authors | none | DEC-007 |
| OpenAPI and Swagger titles, banner | readers | none | DEC-007 |
| `RUST_LOG` directives naming `mcp_fs_core` (`logging.rs:20,40`) | operators | use `fsa_core` | DEC-006 |
| shipped templates and `.env.example`, variables `MCPFS_MINIO_*`, `MCPFS_META_DSN`, `MCPFS_PUBLIC_URL`, `MCPFS_DOC_SERVICE_TOKEN` | operators copying a template | export the `FSA_` name | DEC-007 |
| OpenAPI tag `mcp-fs`, Context7 `X-Context7-Source`, agent `clientInfo.name`, git reflog messages of new entries | consumers of `swagger.json` that group by tag; Context7; MCP servers logging the client name | none | DEC-007 |
| S3 credentials provider name, agent `--help` about text, clap command name in usage lines | readers of AWS SDK debug logs, agent users | none | DEC-007 |
| outbound `User-Agent` of the git provider, OAuth, doc service, web and context7 calls | operators of GitHub, GitLab, Context7, the web fetch targets and the converter | none | DEC-007 |
This is not pure debt: names a client can read change. Observable behavior is otherwise unchanged.

## 8. Tests
### 8.1 Non regression: the success criterion
The existing test suite passes with only the rename edits listed in 8.4:
    `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check`
(the container keeps its name; DR-009 does not reach docker names outside the repository).

### 8.2 Compatibility tests
#### DT-001: old environment variable falls back with one warning
- **Driver:** the compiled binary through `assert_cmd` with `env_clear` and a temporary `XDG_CONFIG_HOME`, asserting standard error; the helper that reads a variable also has a unit test with a resettable counter (the once per process rule needs process state, which the in process resolver `cli.rs:353` cannot isolate).
- **Validates:** DR-006 for each of `MCP_FS_CONFIG`, `MCP_FS_CONFIG_DIR`, `MCP_FS_CONFIG_NAME`, `MCPFS_TOKEN_KEY`.
- Given only the old variable set to a valid value; When the server resolves it twice in one process; Then the value is used, and standard error holds exactly one line `warning: environment variable <OLD> is deprecated, use <NEW>`.
- Given both set to different values; Then the new value is used and the warning is still printed once.
- Given only the new set; Then standard error holds no warning.

#### DT-002: old default secret variable falls back
- **Validates:** DR-007. Given a configuration without `git.github_client_secret_env`, `FSA_GITHUB_CLIENT_SECRET` unset, `MCPFS_GITHUB_CLIENT_SECRET=old`; Then the secret read is `old` with the DR-006 line. Same for `extract.ocr.api_key_env` and `MCP_FS_OCR_KEY` then `FSA_OCR_KEY`. Given the setting written as `MY_SECRET`; Then only `MY_SECRET` is read and no old name is consulted. Given `api_key_env: FSA_OCR_KEY` written explicitly, `FSA_OCR_KEY` unset and `MCP_FS_OCR_KEY=old`; Then the key read is empty and standard error holds no warning.

#### DT-003: old user configuration directory
- **Validates:** DR-008. Given `XDG_CONFIG_HOME` pointing to a temp directory holding only `mcp-fs/config.yaml`; Then that file is loaded and standard error holds `warning: configuration directory <dir> is deprecated, move it to <dir with fsa>`. Given both directories exist; Then the `fsa` file wins and no warning is printed.

#### DT-004: advertised names
- **Validates:** DR-002. `initialize` over HTTP answers `serverInfo.name == "fsa"`; `GET /api/swagger.json` has `info.title == "FSA REST API"`; the git info refs advertisement carries `agent=fsa/0.1.0`; the `WWW-Authenticate` header of an unauthenticated git request carries `realm="fsa"`; a commit made by `seed_commit` has author `fsa <fsa@example.test>`; each outbound `User-Agent` constant equals its new value of DR-002; `fsa --help` first line starts `fsa:`.

### 8.3 Structural tests
#### DT-010: no old name left
- **Asserts:** DR-009. **Allow list:** a constant table of `(path, literal)` pairs naming the walker's own file, the lineage record of step 5, the deprecation notes of `README.md` and `.agent_docs/config.md`, the tests DT-001 to DT-003, the fallback code, and each retained literal of Section 5 and DR-010. **Method:** a test lists the files returned by `git ls-files` (tracked files only, so ignored or untracked local files such as `config/local.yaml` and `.v.log` are never scanned), drops paths under `sdlc/` and `plan/` and the file `Cargo.lock`, fails when `git` is unavailable (it does not skip), and fails, case sensitive, when `mcp-fs`, `mcp_fs`, `MCP-FS`, `MCP FS`, `MCPFS_`, `MCP_FS_`, `McpFs` or `mcpfs` appears outside an explicit allow list: the retained identifiers of Section 5 and DR-010, the fallback code and tests of DR-006 to DR-008. Red until the last step of Section 9.

#### DT-011: packages and binary
- **Asserts:** DR-001. `cargo metadata --no-deps` lists packages `fsa`, `fsa-core` and `agent`, none named `mcp-fs*`; `cargo build -p fsa` produces `target/debug/fsa`; `fsa --version` prints a line starting `fsa `; `fsa version` prints the bare version, unchanged.

#### DT-012: retained identifiers unchanged
- **Asserts:** DR-004. The default of `infra.blob.bucket_prefix` is `mcpfs-`; the default of `safety.trash_dir` is `.mcp_trash`; the refs prefix is `refs/mcpfs/`; the advisory lock key starts `mcpfs_migrate:`; the cookie read by the trash screen is `mcpfs_token`.

### 8.4 Existing tests to modify
Only rename edits, never an assertion weakened. Each is a declared exception.
| Test file | Test | Change | Justification |
|---|---|---|---|
| `crates/core/src/app.rs:577` | initialize test | `"mcp-fs"` to `"fsa"` | DR-002 |
| `crates/core/src/git/http/mod.rs` | tests at `:641,645,669,689` and the realm assertion `:1245`; `crates/core/tests/git_negotiation_e2e.rs:1781` (`OFFERED` agent) | `agent=mcp-fs/0.1.0` to `agent=fsa/0.1.0` | DR-002 |
| `crates/core/src/git/odb.rs:883` | author assertion | `mcp-fs` to `fsa` | DR-002 |
| `crates/core/src/tools/contract_golden.rs`, `tool-contract-golden.json:3` | golden regenerate hint | `-p mcp-fs-core` to `-p fsa-core` | DR-001; the tool schemas themselves do not change |
| `crates/core/tests/full_stack_e2e.rs` (20), `git_negotiation_e2e.rs` (10), `schema_golden.rs` (5) | `use mcp_fs_core::` and paths | to `fsa_core` | DR-001 |
| the whole directory `crates/mcp-fs/tests/` (`sql_server_removed.rs` 13, `crate_split.rs` 3, `makefile_targets.rs`, `security_gate.rs`, `toolchain_pin.rs`, `fixtures/`), moved with `git mv` to `crates/fsa/tests/` | package and binary names, `CARGO_BIN_EXE_mcp-fs` to `CARGO_BIN_EXE_fsa` | DR-001 |
| `crates/core/src/config.rs:1724-1726` test injecting `MCPFS_MINIO_*` and the other template placeholders | `MCPFS_MINIO_*` to `FSA_MINIO_*` | DR-003 |
| `crates/core/src/cli.rs` tests of config resolution and help | variable and directory names | DR-003, DR-008 |
| `crates/core/src/config.rs` `Debug` snapshots (`:1732-1733`) and the assertion near `:1294` on the two secret name settings | the default string becomes the absent form | DR-007, DEC-011 |
| `crates/core/src/cli.rs` `nothing_found_reports_every_path_tried` (about `:568-590`) | the expected tried list gains `<HOME>/.config/fsa/config.yaml` first: `fsa`, `mcp-fs`, `config/local.yaml` | DR-008, a declared change of the asserted list, not a weakening |
| `crates/core/src/git/oauth/device_flow.rs:422` | `github_client_secret_env: Some(gh_env.into())` | DEC-011 type change |
| `crates/core/src/docs/ocr.rs:163` | `api_key_env: Some("MCPFS_TEST_OCR_KEY".into())` | DEC-011 type change |
| every other literal of `OcrConfig` or `GitConfig` that sets the two fields | wrapped in `Some(..)` | DEC-011 |
| `crates/core/src/api/openapi.rs:1676` (title), `:1679` (tag), `:2003` (`<title>mcp-fs API</title>`) | title, tag and page title assertions `"mcp-fs"` to `"fsa"` | DR-002 |
| `crates/agent/src/main.rs:627` | assertion `mcp-fs token` to `fsa token` | DR-002 |
| tests that read or inject `MCPFS_MINIO_*` (`s3.rs:269,455-505`, `full_stack_e2e.rs:83,88,90`, `config.rs:1726`) | variable names | DR-003 |
| `cli.rs` and `tools/web_access_boot_e2e.rs` argv[0] literals, `@mcp-fs.test` fixtures (`full_stack_e2e.rs:66`, `git_negotiation_e2e.rs:26,27,245,339`), `tools/git.rs:11084,11093,11572`, `git/http/mod.rs:780`, `config.rs:1898-1899` | literal `mcp-fs` to `fsa`, `MCPFS_PUBLIC_URL` to `FSA_PUBLIC_URL` | DR-010 |
| every other test file reported by the baseline `rg` of Section 2 | edits limited to the strings of DR-001, DR-002, DR-003 and DR-010 | DR-001 to DR-003, DR-010 |
The exact list is the output of the baseline `rg` of Section 2 run at the start of step 1 and recorded in the first story; DT-010 gates that nothing else changed.

## 9. Implementation order
Each step leaves `make check` green (DT-010 is added at step 5, when it can pass).
1. **Crates.** `git mv crates/mcp-fs crates/fsa`, rename packages and the library crate, fix `use`, `-p`, `Cargo.toml` `members`, `deny.toml`, `Makefile`, `run.sh`, `agent.sh`, `test.sh`, `build.sh`, the functional scripts and `specs/audit.py`, `scripts/pty_check.py`, `scripts/doc_service_fake.py` (none of these is exercised by `make check`, so the step's check is `make check` plus `python3 scripts/pty_check.py --help`, `./run.sh` up to the build line, and `./agent.sh --help`). DR-001, DT-011. Green: the compiler and the suite prove it.
2. **Advertised names.** DR-002 and its test edits and goldens. DT-004.
3. **Environment variables.** DR-003, DR-006, DR-007 with the single helper that reads a new variable then the old with the warning; includes the live `${...}` lines of `config/minio.yaml.template` and `config/local.yaml.template` and the `config.rs:1724-1726` test, so that step 3 stays green (step 5 handles comments and prose only). DT-001, DT-002.
4. **User configuration directory.** DR-008. DT-003.
5. **Scripts, templates, docs.** Replace the remaining old names in `README.md` (clone line `git clone git@github.com:smorand/fsa.git && cd fsa`), `AGENTS.md`, `.agent_docs/*.md`, `FUNCTIONAL_TESTS.md`, `tests/functional/`, templates, `docker-compose.test.yml`, `.env.example`, `Cargo.toml` `repository = "https://github.com/smorand/fsa"` (valid after runbook step a; until then GitHub has no such repository); keep the retained identifiers; add DT-010 and DT-012; record the change in `.agent_docs/lineage.md`. DR-004, DR-009.

**Operator runbook, not stories (DEC-009), each on the owner's explicit yes:**
a. `gh repo rename fsa -R smorand/mcp-fs-rust`, then `git remote set-url origin git@github.com:smorand/fsa.git` (GitHub keeps a redirect from the old name).
b. The owner updates `os-config/bash/bashrc` (`FSA_TOKEN_KEY`, and the credential file name if wanted) and `os-config/kitty/tabs.json:26`, and fixes the two stale references in `mcp-exec` and `config-a2a`.
c. Last: move the local folder `/Users/sebastien/projects/perso/mcp-fs-rust` to `/Users/sebastien/projects/perso/fsa`, by the owner, outside any session working in it. The test container name `mcp-fs-rust-postgres-1` comes from the folder name through the compose project, so after the move it is `fsa-postgres-1`; update the `docker start` line of 8.1.
d. Rename the test PostgreSQL container and the developer variables in a later cleanup (DEC-005).

## 10. Implementability gate
| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 3 | 3 | NOT-IMPLEMENTABLE (secret name presence, user agents, `McpFs` and `mcpfs` spellings) |
| 2 | 6 | 8 | NOT-IMPLEMENTABLE (OpenAPI tag, git ref and stash trailer, MinIO variables, product strings in wire values, fixture rule, screens) |
| 3 | 3 | 6 | NOT-IMPLEMENTABLE (undeclared test edits, DT-010 walker scope, config probe order) |
| 4 | 2 | 11 | NOT-IMPLEMENTABLE (test literals of `mcp-fs` outside DR-010, blank environment values); both amended, A1 to A11 amended |
| 5 | | | waived by the owner on 2026-11-19; DT-010 is the mechanical gate on the perimeter, and every finding of rounds 1 to 4 is merged |

**Amendments applied:** every F and A finding of rounds 1 to 3 was merged into Sections 2 to 9 (DR-010, DEC-011, the 8.4 rows, the exact probe order of DR-008, the `git ls-files` method of DT-010).

## Design additions (2026-11-19)

### A.1 Target tier and slicing
Target tier 2 (default: no `--tier`, no constitution field, no `.spec.json`). Every story is `min_tier` 2: the tier 3 and 4 models have no default mapping in `/sdlc-implement`, and a story on one of them would be blocked. Several stories touch many files (US-0256, US-0262, US-0263); they are mechanical renames, where decision density, not file count, sets the tier. The slicing follows Section 9: one story per step, except step 2 (two stories: names clients read, names sent to others), step 3 (two stories: the helper and the plain variables, then the secret name settings and the templates) and step 5 (two stories: test code spellings, then scripts, docs and the guard tests).

### A.2 File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/mcp-fs/` to `crates/fsa/` (with `tests/`, `src/main.rs`, `Cargo.toml`) | move and rename | DR-001 | US-0256 |
| `Cargo.toml`, `crates/core/Cargo.toml`, `crates/agent/Cargo.toml`, `deny.toml`, `Cargo.lock` | modify | DR-001 | US-0256 |
| `Makefile`, `run.sh`, `agent.sh`, `test.sh`, `build.sh`, `specs/audit.py`, `scripts/pty_check.py`, `scripts/doc_service_fake.py`, `tests/functional/**` (binary and package names only) | modify | DR-001 | US-0256 |
| `crates/core/src/mcp/server.rs`, `app.rs`, `api/openapi.rs`, `cli.rs`, `git/http/mod.rs`, `git/odb.rs` | modify | DR-002 | US-0257 |
| `crates/core/src/git/provider/mod.rs`, `git/oauth/device_flow.rs`, `docs/service.rs`, `tools/web.rs`, `tools/context7.rs`, `storage/blob/s3.rs`, `tools/git_tags.rs`, `tools/git.rs`, `crates/agent/src/mcp.rs`, `crates/agent/src/main.rs` | modify | DR-002 | US-0258 |
| `crates/core/src/env_compat.rs` | create | DR-006 | US-0259 |
| `crates/core/src/lib.rs`, `cli.rs`, `git/oauth/store.rs`, `git/oauth/cipher.rs` | modify | DR-003, DR-006 | US-0259 |
| `crates/fsa/tests/env_compat_e2e.rs` | create | DT-001, DT-002, DT-003 | US-0259, US-0260, US-0261 |
| `crates/core/src/config.rs`, `docs/ocr.rs`, `git/oauth/device_flow.rs`, `config/minio.yaml.template`, `config/local.yaml.template`, `storage/blob/s3.rs` (test reads), `crates/core/tests/full_stack_e2e.rs`, `Makefile`, `.env.example` | modify | DR-003, DR-007 | US-0260 |
| `crates/core/src/cli.rs` | modify | DR-008 | US-0261 |
| `crates/core/src/docs/pdf_bound.rs` and every test module and test file carrying an old spelling | modify | DR-010 | US-0262 |
| `README.md`, `AGENTS.md`, `.agent_docs/*.md`, `FUNCTIONAL_TESTS.md`, `TOOL_CONTRACT.txt`, `specs/AUDIT.md`, `docker-compose.test.yml`, `config/agent_test.yaml`, `.gitignore`, `Cargo.toml` (`repository`), `tests/functional/**` (prose) | modify | DR-009, DR-004 | US-0263 |
| `crates/fsa/tests/old_name_gone.rs`, `crates/fsa/tests/retained_identifiers.rs`, `crates/fsa/tests/names.rs` | create | DT-010, DT-012, DT-011 | US-0263, US-0263, US-0256 |
Affected existing tests: Section 8.4 is the list; each row is carried by the story that makes the rename it declares. Affected documentation: the docs row of US-0263 and a lineage entry in `.agent_docs/lineage.md`.

### A.3 Test strategy
| Test | Level | Driver | Location | Story |
|---|---|---|---|---|
| DT-011 | integration | `cargo metadata --no-deps` and the built binary through `std::process::Command` | `crates/fsa/tests/names.rs` | US-0256 |
| DT-001 | functional e2e | the compiled binary, `env_clear`, temporary `XDG_CONFIG_HOME`, standard error read; unit test of the helper with a resettable counter | `crates/fsa/tests/env_compat_e2e.rs`, `crates/core/src/env_compat.rs` | US-0259 |
| DT-002 | functional e2e | same | same | US-0260 |
| DT-003 | functional e2e | same | same | US-0261 |
| DT-004 | functional e2e and unit | the existing in module tests that assert each name (`app.rs`, `api/openapi.rs`, `git/http/mod.rs`, `git/odb.rs`, `tools/context7.rs`, `crates/agent`), plus `fsa --help` on the binary | those modules, `crates/fsa/tests/names.rs` | US-0258 |
| DT-012 | integration | public defaults of `fsa_core::config` and source constants | `crates/fsa/tests/retained_identifiers.rs` | US-0263 |
| DT-010 | structural | `git ls-files`, case sensitive search with an allow list | `crates/fsa/tests/old_name_gone.rs` | US-0263 |
Test data: generated (temporary directories, environment of a child process). No new dependency: `std::process::Command` with `CARGO_BIN_EXE_fsa`, as `sql_server_removed.rs:11` does today.

### A.4 Added decisions
- **DEC-012:** the compatibility tests drive the compiled binary through `std::process::Command` with `env_clear`, as `crates/mcp-fs/tests/sql_server_removed.rs:11` does, and read standard error. **Obliged by:** DR-006, DR-007, DR-008. **Rationale:** the once per process warning is process state, and `resolve_config_path_with` (`cli.rs:366`) takes an injected environment so it cannot show it. **Alternatives considered:** `assert_cmd` (rejected: a new dev dependency, category 2 of ASK ONLY FOR). **Source:** existing code `crates/mcp-fs/tests/sql_server_removed.rs:11`. **Evidence:** `cli.rs:343-366`.
- **DEC-013:** one module `crate::env_compat` reads a variable by its new name then its old name, with a process wide set of names already warned, a function to read a name given an environment closure (so `resolve_config_path_with` stays testable) and a test only reset. Every reader in DR-003 goes through it. **Obliged by:** DR-006, DR-007. **Rationale:** one place owns precedence, blank handling and the warning text, so DT-001 tests one implementation. **Alternatives considered:** a fallback at each of the six call sites (rejected: six copies of the warning and the blank rule). **Source:** design. **Evidence:** the readers at `cli.rs:404-405`, `git/oauth/store.rs:112`, `git/oauth/device_flow.rs:280`, `docs/ocr.rs:83`.
- **DEC-014:** the two secret name accessors keep returning `String`, so `device_flow.rs` and `ocr.rs` call sites do not change shape; the `Option` lives in the configuration struct only (DEC-011). **Obliged by:** DR-007. **Rationale:** smallest change at the call sites. **Alternatives considered:** `Option` returned to callers (rejected: spreads the fallback to every call site). **Source:** design. **Evidence:** `device_flow.rs:280`, `docs/ocr.rs:83`.
- **DEC-015:** stories use the order of Section 9 and depend on each other in a line (`US-0256` to `US-0263`), because `cli.rs`, `config.rs` and `s3.rs` are edited by several of them and one writer works on the branch. **Obliged by:** n/a (no code impact): ordering of work.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0256 | rename the packages, the crate and the binary to fsa | P1 | DR-001, DR-005 | main | 2 | no |
| US-0257 | the names clients read are fsa | P1 | DR-002, DR-005 | US-0256 | 2 | no |
| US-0258 | the names sent to other systems are fsa | P1 | DR-002, DR-005 | US-0257 | 2 | no |
| US-0259 | environment variable fallback helper and the config and token variables | P1 | DR-003, DR-005, DR-006 | US-0258 | 2 | no |
| US-0260 | secret name settings, templates and MinIO variables | P1 | DR-003, DR-005, DR-007 | US-0259 | 2 | no |
| US-0261 | the user configuration directory fsa with fallback | P1 | DR-005, DR-008 | US-0259 | 2 | no |
| US-0262 | test code and fixture spellings | P2 | DR-005, DR-010 | US-0261 | 2 | no |
| US-0263 | scripts, docs, repository URL and the guard tests | P1 | DR-004, DR-005, DR-009 | US-0262 | 2 | yes |
