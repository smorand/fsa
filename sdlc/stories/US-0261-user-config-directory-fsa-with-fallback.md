# US-0261: the user configuration directory fsa with fallback

> Spec: SPEC-0099 (sdlc/specs/SPEC-0099-rename-the-project-to-fsa/)
> Nature: DEBT
> Priority: P1
> Depends on: US-0259
> min_tier: 2
> Files touched: 3
> Closes spec: no

## Objective
Probe `~/.config/fsa/config.yaml` before `~/.config/mcp-fs/config.yaml`, warn when only the old one exists, and fix the order of the whole config resolution as DR-008 states.

## Requirements covered
- SPEC-0099/DR-008: WHEN no file exists at `~/.config/fsa/config.yaml` and a file exists at `~/.config/mcp-fs/config.yaml` THE server SHALL load the second one and print `warning: configuration directory <HOME>/.config/mcp-fs is deprecated, move it to <HOME>/.config/fsa`.
- SPEC-0099/DR-005 (partial: only the probe order and the warning change): The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.

## Independent Test
DT-003 passes: with `XDG_CONFIG_HOME` pointing to a temp directory holding only `mcp-fs/config.yaml`, the binary loads it and prints the warning; with both directories the `fsa` file wins and nothing is printed.

## Design excerpt
### Decisions
- DEC-003 (SPEC-0099): dual maintenance of the six environment variables and of `~/.config/mcp-fs`, removal at release 1.0.0. **Alternatives considered:** break at once (rejected: the default for a public contract is dual maintenance), warn on every read (rejected: noise). **Implemented by:** DR-006, DR-007, DR-008.
- DEC-012 (SPEC-0099): the compatibility tests drive the compiled binary through `std::process::Command` with `env_clear`, as `crates/mcp-fs/tests/sql_server_removed.rs:11` does, and read standard error. **Obliged by:** DR-006, DR-007, DR-008. **Rationale:** the once per process warning is process state, and `resolve_config_path_with` (`cli.rs:366`) takes an injected environment so it cannot show it. **Alternatives considered:** `assert_cmd` (rejected: a new dev dependency, category 2 of ASK ONLY FOR). **Source:** existing code `crates/mcp-fs/tests/sql_server_removed.rs:11`. **Evidence:** `cli.rs:343-366`.

### Data model
None.

### Contracts
Order: (1) `--config`; (2) `FSA_CONFIG`, else `MCP_FS_CONFIG` with warning; (3) `<base>/fsa/config.yaml`; (4) `<base>/mcp-fs/config.yaml` with warning `warning: configuration directory <HOME>/.config/mcp-fs is deprecated, move it to <HOME>/.config/fsa`; (5) `<dir>/<name>.yaml` from `FSA_CONFIG_DIR` or `MCP_FS_CONFIG_DIR` (warning) or `config`, and `FSA_CONFIG_NAME` or `MCP_FS_CONFIG_NAME` (warning) or `local`. The tried list of the boot error is (3), (4), (5) in that order.

### Paths
| Path | Action |
|---|---|
| `crates/core/src/cli.rs:30,37,343-410` | `XDG_APP_DIR = "fsa"`, an extra probe for the `mcp-fs` directory with the warning, the tried list order `fsa`, `mcp-fs`, `config/local.yaml` |
| `crates/fsa/tests/env_compat_e2e.rs` | add DT-003 |

### Existing patterns
`resolve_config_path_with` (`cli.rs:366`) already probes the XDG file then the dir and name pair; `XDG_APP_DIR` (`cli.rs:30`) holds the directory name.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

The success criterion of the debt, restated: the existing suite passes unmodified except the rename edits declared for this story: `cli.rs` `nothing_found_reports_every_path_tried` (about `:568-590`): the expected tried list gains `<HOME>/.config/fsa/config.yaml` first, so `fsa`, `mcp-fs`, `config/local.yaml` (DR-008, a declared change of the asserted list, not a weakening). Command: `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` (the container and the developer variable keep their names, DEC-005).

### SPEC-0099/DT-003: old user configuration directory
- Level / Driver / Location: functional e2e / `crates/fsa/tests/env_compat_e2e.rs`
- **Validates:** DR-008. Given `XDG_CONFIG_HOME` pointing to a temp directory holding only `mcp-fs/config.yaml`; Then that file is loaded and standard error holds `warning: configuration directory <dir> is deprecated, move it to <dir with fsa>`. Given both directories exist; Then the `fsa` file wins and no warning is printed.


## Constraints
- Do not change the config file format or its contents.
- The base directory still comes from `etcetera`/`XDG_CONFIG_HOME` exactly as today.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (a test that pins behavior which already holds says so instead).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: every test of the suite not named in the exceptions passes unchanged; `tool-contract-golden.json` schemas and descriptions do not change.
- [ ] DEBT: the existing suite passes unmodified, exceptions: `cli.rs` `nothing_found_reports_every_path_tried` (about `:568-590`): the expected tried list gains `<HOME>/.config/fsa/config.yaml` first, so `fsa`, `mcp-fs`, `config/local.yaml` (DR-008, a declared change of the asserted list, not a weakening) (Section 8.4 of the design).
- [ ] Docs updated: prose that names what this story renames is left to US-0263, except where a documented command stops working (then fixed here).
- [ ] Self review (full): the diff renames only what the requirements name, the retained identifiers of Section 5 are untouched, no behavior moved.
- [ ] One commit `US-0261: the user configuration directory fsa with fallback` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0099` and the requirement ids.
