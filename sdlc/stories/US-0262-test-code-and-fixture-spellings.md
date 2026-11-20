# US-0262: test code and fixture spellings

> Spec: SPEC-0099 (sdlc/specs/SPEC-0099-rename-the-project-to-fsa/)
> Nature: DEBT
> Priority: P2
> Depends on: US-0261
> min_tier: 2
> Files touched: 25
> Closes spec: no

## Objective
Rename the spellings of the old name that live only in test code and in the in memory PDF markers, so that the guard test of the last story passes. Mechanical: no production behavior moves.

## Requirements covered
- SPEC-0099/DR-010: The repository SHALL rename the in memory PDF marker names `McpFsObjStm`, `McpFsFileType` and `McpFsFilter` (`docs/pdf_bound.rs:46,50,58`) to `FsaObjStm`, `FsaFileType` and `FsaFilter`, and every lowercase `mcpfs` or `mcp_fs` in test code, except the retained literals of Section 5, to `fsa` keeping the rest of the string (`mcpfs-test` to `fsa-test`, `mcpfs_e2e_` to `fsa_e2e_`, `mcp_fs_2` at `storage/rel/postgres.rs:505` to `fsa_2`, the fixtures at `docs/service.rs:1436` (keep the fixture's `/Length` equal to its stream length), `tools/git.rs:9817`, `cli.rs:548,554`). The repository SHALL also rename, in test code (`#[cfg(test)]` modules and `tests/` directories), every remaining `mcp-fs` to `fsa` and `MCPFS_PUBLIC_URL` to `FSA_PUBLIC_URL`, keeping the rest of the string: `Cli::parse_from(["mcp-fs", ...])` becomes `["fsa", ...]`, `@mcp-fs.test` becomes `@fsa.test`, `mcp-fs-fetch-unreachable-test.invalid` becomes `fsa-fetch-unreachable-test.invalid`, the function name `mcp_fs_src_contains_only_main_rs` (`crates/mcp-fs/tests/crate_split.rs:8`) becomes `fsa_src_contains_only_main_rs`, `CARGO_BIN_EXE_mcp-fs` becomes `CARGO_BIN_EXE_fsa`; the retained spellings of Section 5 are excepted. A lowercase `mcpfs` is retained only when it spells the bucket prefix `mcpfs-` and the assertions on it, the cookie `mcpfs_token`, the DSN segments `mcpfs:mcpfs@.../mcpfs` and the compose test database, the schema `mcpfs` asserted as a valid name (`postgres.rs:505`), `refs/mcpfs/` or `mcpfs_migrate:`.
- SPEC-0099/DR-005 (partial: only test literals and the three in memory PDF marker names change): The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.

## Independent Test
The command `git ls-files | rg -n 'mcp-fs|mcp_fs|MCPFS_PUBLIC_URL|McpFs|mcpfs' -- crates tests` lists only the retained literals of Section 5 and DR-010, and `make check` is green.

## Design excerpt
### Decisions
- DEC-004 (SPEC-0099): identifiers stored in existing deployments keep their old spelling. **Rationale:** renaming the bucket prefix orphans buckets, renaming `.mcp_trash` loses trashed files, renaming the refs and the lock key breaks mixed fleets. **Alternatives considered:** migrate them (rejected: irreversible data change, no gain). **Implemented by:** DR-004. **Evidence:** `config.rs:244,364`, `git/remote.rs:1453,1456`, `storage/rel/postgres.rs:386`, `tools/git.rs:4535,7151` (an in flight rebase and existing stashes would be orphaned).
- DEC-005 (SPEC-0099): `MCPFS_TEST_*`, `MCPFS_REWRITE_*`, `MCPFS_CHILD_CASE*`, `MCPFS_PRINT_RECORD` keep their names for now. **Rationale:** a test that reads `MCPFS_TEST_PG_DSN` skips silently when it is unset (`.agent_docs/testing.md`, "opt in suites"), so renaming it while the owner's shell exports the old name would turn the PostgreSQL suites green by skipping. **Alternatives considered:** rename with a fallback in 56 places (rejected: cost, no user value). **Exit condition:** rename together with the shell update, in a later cleanup. **Implemented by:** DR-004.
- DEC-008 (SPEC-0099): the history folders `sdlc/` and `plan/` keep the old name. **Rationale:** they are the record of what was decided and shipped. **Implemented by:** DR-009.

### Data model
None.

### Contracts
The retained lowercase spellings are exactly: the bucket prefix `mcpfs-` and its assertions (`config.rs:244,1281,1486`), the cookie `mcpfs_token`, the DSN segments `mcpfs:mcpfs@.../mcpfs` and the compose test database, the schema `mcpfs` accepted as a valid name (`postgres.rs:505`), `refs/mcpfs/`, `mcpfs_migrate:`.

### Paths
| Path | Action |
|---|---|
| `crates/core/src/docs/pdf_bound.rs:46,50,58` | `McpFsObjStm`, `McpFsFileType`, `McpFsFilter` to `FsaObjStm`, `FsaFileType`, `FsaFilter` |
| every `Cli::parse_from(["mcp-fs", ...])` in `crates/core/src/cli.rs` tests and `tools/web_access_boot_e2e.rs:106,151,166,242,257,276` | `"fsa"` |
| `crates/core/tests/full_stack_e2e.rs:66`, `git_negotiation_e2e.rs:26,27,245,339`, `git/http/mod.rs:780`, `tools/git.rs:11084,11093,11572`, `config.rs:1898-1899` | `mcp-fs` to `fsa`, `MCPFS_PUBLIC_URL` to `FSA_PUBLIC_URL` |
| `crates/fsa/tests/crate_split.rs:8` | function name `fsa_src_contains_only_main_rs` |
| lowercase `mcpfs` and `mcp_fs` fixtures in test code (`docs/service.rs:1436` keeping `/Length` equal to the stream length, `tools/git.rs:9817`, `cli.rs:548,554`, `storage/rel/postgres.rs:505` `mcp_fs_2` to `fsa_2`, the ephemeral names `mcpfs-test`, `mcpfs-unit`, `mcpfs-rust-it-`, `mcpfs_vecpg_*`, `mcpfs_bm25pg_*`, `mcpfs_e2e_`, `mcpfs_probe_*`, `mcpfs_pgopen*`) | `fsa` with the rest of the string kept |

### Existing patterns
A rename of a fixture string must keep every assertion on it consistent in the same file.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

The success criterion of the debt, restated: the existing suite passes unmodified except the rename edits declared for this story: every edit is a literal rename: the `cli.rs` and `web_access_boot_e2e.rs` argv[0], the `@mcp-fs.test` fixtures, `tools/git.rs:11084,11093,11572`, `git/http/mod.rs:780`, `config.rs:1898-1899`, the `crate_split.rs` function name and the lowercase fixtures (DR-010). Command: `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` (the container and the developer variable keep their names, DEC-005).

### Structural check (story local, not a DT)
- Given the tree after the change; When `git ls-files crates tests | xargs rg -n -e 'mcp-fs' -e 'mcp_fs' -e 'MCPFS_PUBLIC_URL' -e 'McpFs'`; Then the only hits are the C# lineage comment `git/mod.rs:5`, the retained spellings above, and the fallback code and tests of US-0259 to US-0261.
- And `make check` is green.

## Constraints
- Rename only inside test code and the three PDF markers.
- Do not touch the retained spellings.
- An assertion is never weakened: only the literal changes.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (a test that pins behavior which already holds says so instead).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: every test of the suite not named in the exceptions passes unchanged; `tool-contract-golden.json` schemas and descriptions do not change.
- [ ] DEBT: the existing suite passes unmodified, exceptions: every edit is a literal rename: the `cli.rs` and `web_access_boot_e2e.rs` argv[0], the `@mcp-fs.test` fixtures, `tools/git.rs:11084,11093,11572`, `git/http/mod.rs:780`, `config.rs:1898-1899`, the `crate_split.rs` function name and the lowercase fixtures (DR-010) (Section 8.4 of the design).
- [ ] Docs updated: prose that names what this story renames is left to US-0263, except where a documented command stops working (then fixed here).
- [ ] Self review (full): the diff renames only what the requirements name, the retained identifiers of Section 5 are untouched, no behavior moved.
- [ ] One commit `US-0262: test code and fixture spellings` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0099` and the requirement ids.
