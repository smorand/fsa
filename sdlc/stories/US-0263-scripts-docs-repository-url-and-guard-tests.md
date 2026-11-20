# US-0263: scripts, docs, repository URL and the guard tests

> Spec: SPEC-0099 (sdlc/specs/SPEC-0099-rename-the-project-to-fsa/)
> Nature: DEBT
> Priority: P1
> Depends on: US-0262
> min_tier: 2
> Files touched: 35
> Closes spec: yes

## Objective
Rename the product in every remaining script, template comment, document and the repository URL, record the change in the lineage notes, and add the two guard tests: no old name outside the allow list, and the retained identifiers unchanged. This closes the spec: the whole acceptance set and the coverage audit run here.

## Requirements covered
- SPEC-0099/DR-004: The server SHALL NOT rename the identifiers listed in Section 5 as retained.
- SPEC-0099/DR-005: The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.
- SPEC-0099/DR-009: The repository SHALL NOT contain, case sensitive, `mcp-fs`, `mcp_fs`, `MCP-FS`, `MCP FS`, `MCPFS_`, `MCP_FS_`, `McpFs` or `mcpfs` outside the retained identifiers of Section 5 and DR-010, the fallback code of DR-006 to DR-008 with its tests, and the history folders `sdlc/` and `plan/`.

## Independent Test
DT-010 and DT-012 pass and the full set DT-001 to DT-004, DT-010 to DT-012 is green through `make check`.

## Design excerpt
### Decisions
- DEC-004 (SPEC-0099): identifiers stored in existing deployments keep their old spelling. **Rationale:** renaming the bucket prefix orphans buckets, renaming `.mcp_trash` loses trashed files, renaming the refs and the lock key breaks mixed fleets. **Alternatives considered:** migrate them (rejected: irreversible data change, no gain). **Implemented by:** DR-004. **Evidence:** `config.rs:244,364`, `git/remote.rs:1453,1456`, `storage/rel/postgres.rs:386`, `tools/git.rs:4535,7151` (an in flight rebase and existing stashes would be orphaned).
- DEC-005 (SPEC-0099): `MCPFS_TEST_*`, `MCPFS_REWRITE_*`, `MCPFS_CHILD_CASE*`, `MCPFS_PRINT_RECORD` keep their names for now. **Rationale:** a test that reads `MCPFS_TEST_PG_DSN` skips silently when it is unset (`.agent_docs/testing.md`, "opt in suites"), so renaming it while the owner's shell exports the old name would turn the PostgreSQL suites green by skipping. **Alternatives considered:** rename with a fallback in 56 places (rejected: cost, no user value). **Exit condition:** rename together with the shell update, in a later cleanup. **Implemented by:** DR-004.
- DEC-008 (SPEC-0099): the history folders `sdlc/` and `plan/` keep the old name. **Rationale:** they are the record of what was decided and shipped. **Implemented by:** DR-009.
- DEC-009 (SPEC-0099): the actions outside the repository (GitHub repository rename, remote URL, local folder move, the other repositories listed in Section 2) are an operator runbook (Section 9), run only on the owner's yes, never by a story. **Implemented by:** n/a, no code impact (operational).
- DEC-010 (SPEC-0099): the version bump is decided at landing: the rule gives a PATCH for a DEBT; Section 7 lists breaks, so the landing report flags "breaking change, consider `--major`". **Implemented by:** n/a, no code impact.

### Data model
None.

### Contracts
No code contract changes. Allow list of DT-010: a constant table of `(path, literal)` pairs naming the test's own file, the lineage record, the deprecation notes of `README.md` and `.agent_docs/config.md`, the tests and code of the fallbacks (US-0259 to US-0261), the C# lineage comment `git/mod.rs:5`, the `mcp fs.<tool>` shell helper lines, and each retained literal of Section 5 and DR-010.

### Paths
| Path | Action |
|---|---|
| `README.md` (28), `AGENTS.md` (16), `.agent_docs/*.md` (88 over 8 files), `FUNCTIONAL_TESTS.md` (47), `TOOL_CONTRACT.txt:727`, `specs/AUDIT.md:36`, `tests/functional/README.md` | prose, commands, examples; clone line `git clone git@github.com:smorand/fsa.git && cd fsa`; deprecation notes for the old variables and the old config directory in `README.md` and `.agent_docs/config.md` |
| `Cargo.toml:10` | `repository = "https://github.com/smorand/fsa"` (valid after runbook step a) |
| `docker-compose.test.yml`, `config/agent_test.yaml`, `config/*.template` (comments), `.gitignore:8`, `.env.example` (comments), `tests/functional/**` (prose and temp names `/tmp/fsa-*`) | comments and names, retained database `mcpfs` kept |
| .agent_docs/lineage.md | entry dated 2026-11-19 recording the rename, the retained identifiers and the removal of the fallbacks at 1.0.0 |
| `crates/fsa/tests/old_name_gone.rs` | create (DT-010) |
| `crates/fsa/tests/retained_identifiers.rs` | create (DT-012) |

### Existing patterns
`crates/fsa/tests/sql_server_removed.rs` shows how a test in this crate locates the repository root and runs a command (`CARGO_MANIFEST_DIR`).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

The success criterion of the debt, restated: the existing suite passes unmodified except the rename edits declared for this story: prose and comment edits only; the two new test files are additions. Command: `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` (the container and the developer variable keep their names, DEC-005).

### SPEC-0099/DT-010: no old name left
- Level / Driver / Location: structural / `crates/fsa/tests/old_name_gone.rs`
- **Asserts:** DR-009. **Allow list:** a constant table of `(path, literal)` pairs naming the walker's own file, the lineage record of step 5, the deprecation notes of `README.md` and `.agent_docs/config.md`, the tests DT-001 to DT-003, the fallback code, and each retained literal of Section 5 and DR-010. **Method:** a test lists the files returned by `git ls-files` (tracked files only, so ignored or untracked local files such as `config/local.yaml` and `.v.log` are never scanned), drops paths under `sdlc/` and `plan/` and the file `Cargo.lock`, fails when `git` is unavailable (it does not skip), and fails, case sensitive, when `mcp-fs`, `mcp_fs`, `MCP-FS`, `MCP FS`, `MCPFS_`, `MCP_FS_`, `McpFs` or `mcpfs` appears outside an explicit allow list: the retained identifiers of Section 5 and DR-010, the fallback code and tests of DR-006 to DR-008. Red until the last step of Section 9.

- Exact assertion: `git ls-files` exits 0, and the set of `(path, literal)` pairs found, case sensitive, for `mcp-fs`, `mcp_fs`, `MCP-FS`, `MCP FS`, `MCPFS_`, `MCP_FS_`, `McpFs`, `mcpfs` minus the allow list is empty.

### SPEC-0099/DT-012: retained identifiers unchanged
- Level / Driver / Location: integration / `crates/fsa/tests/retained_identifiers.rs`
- **Asserts:** DR-004. The default of `infra.blob.bucket_prefix` is `mcpfs-`; the default of `safety.trash_dir` is `.mcp_trash`; the refs prefix is `refs/mcpfs/`; the advisory lock key starts `mcpfs_migrate:`; the cookie read by the trash screen is `mcpfs_token`.

- Exact assertions: `ServerConfig::default().infra.blob.bucket_prefix == "mcpfs-"`; `ServerConfig::default().safety.trash_dir == ".mcp_trash"`; the source constants for the refs prefix `refs/mcpfs/`, the lock key `mcpfs_migrate:` and the cookie `mcpfs_token` are unchanged.

## Constraints
- Do not rename the retained identifiers of Section 5 or the developer variables `MCPFS_TEST_*` (DEC-005).
- `sdlc/` and `plan/` are history and stay as they are.
- The external actions (GitHub rename, remote URL, local folder move, other repositories) are the operator runbook, not part of this story.
- The test container name `mcp-fs-rust-postgres-1` in docs becomes the command `docker compose -f docker-compose.test.yml up -d` where it is a command to run.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (a test that pins behavior which already holds says so instead).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: every test of the suite not named in the exceptions passes unchanged; `tool-contract-golden.json` schemas and descriptions do not change.
- [ ] DEBT: the existing suite passes unmodified, exceptions: prose and comment edits only; the two new test files are additions (Section 8.4 of the design).
- [ ] Docs updated: prose that names what this story renames is left to US-0263, except where a documented command stops working (then fixed here).
- [ ] Self review (full): the diff renames only what the requirements name, the retained identifiers of Section 5 are untouched, no behavior moved.
- [ ] Closes spec: the full acceptance set (DT-001, DT-002, DT-003, DT-004, DT-010, DT-011, DT-012) is green, and every requirement DR-001 to DR-010 of SPEC-0099 is cited by at least one story with status done or landed.
- [ ] The landing report flags `--major` (declared breaks of Section 7) without deciding it.
- [ ] One commit `US-0263: scripts, docs, repository URL and the guard tests` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0099` and the requirement ids.
