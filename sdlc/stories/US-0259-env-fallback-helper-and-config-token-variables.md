# US-0259: environment variable fallback helper and the config and token variables

> Spec: SPEC-0099 (sdlc/specs/SPEC-0099-rename-the-project-to-fsa/)
> Nature: DEBT
> Priority: P1
> Depends on: US-0258
> min_tier: 2
> Files touched: 6
> Closes spec: no

## Objective
Add one module that reads an environment variable by its new name then its old name and warns once per process, and move the four plain variables onto it: `FSA_CONFIG`, `FSA_CONFIG_DIR`, `FSA_CONFIG_NAME` (old `MCP_FS_*`) and `FSA_TOKEN_KEY` (old `MCPFS_TOKEN_KEY`). A blank value counts as unset.

## Requirements covered
- SPEC-0099/DR-003 (partial: the variables `FSA_CONFIG`, `FSA_CONFIG_DIR`, `FSA_CONFIG_NAME`, `FSA_TOKEN_KEY` and the config path error text): The server SHALL read these environment variables under the `FSA_` prefix.
- SPEC-0099/DR-006: WHEN one of the old variables `MCP_FS_CONFIG`, `MCP_FS_CONFIG_DIR`, `MCP_FS_CONFIG_NAME`, `MCPFS_TOKEN_KEY` is set and its new variable is unset THE server SHALL use the old value, and SHALL print on standard error, once per process and per variable, the line `warning: environment variable <OLD> is deprecated, use <NEW>`.
- SPEC-0099/DR-005 (partial: only the variable names and their fallback change): The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.

## Independent Test
DT-001 passes: the binary run with only the old variable uses its value and prints exactly `warning: environment variable <OLD> is deprecated, use <NEW>` once on standard error; with both set the new value wins; with only the new set there is no warning; a blank new value falls back to a set old one.

## Design excerpt
### Decisions
- DEC-003 (SPEC-0099): dual maintenance of the six environment variables and of `~/.config/mcp-fs`, removal at release 1.0.0. **Alternatives considered:** break at once (rejected: the default for a public contract is dual maintenance), warn on every read (rejected: noise). **Implemented by:** DR-006, DR-007, DR-008.
- DEC-012 (SPEC-0099): the compatibility tests drive the compiled binary through `std::process::Command` with `env_clear`, as `crates/mcp-fs/tests/sql_server_removed.rs:11` does, and read standard error. **Obliged by:** DR-006, DR-007, DR-008. **Rationale:** the once per process warning is process state, and `resolve_config_path_with` (`cli.rs:366`) takes an injected environment so it cannot show it. **Alternatives considered:** `assert_cmd` (rejected: a new dev dependency, category 2 of ASK ONLY FOR). **Source:** existing code `crates/mcp-fs/tests/sql_server_removed.rs:11`. **Evidence:** `cli.rs:343-366`.
- DEC-013 (SPEC-0099): one module `crate::env_compat` reads a variable by its new name then its old name, with a process wide set of names already warned, a function to read a name given an environment closure (so `resolve_config_path_with` stays testable) and a test only reset. Every reader in DR-003 goes through it. **Obliged by:** DR-006, DR-007. **Rationale:** one place owns precedence, blank handling and the warning text, so DT-001 tests one implementation. **Alternatives considered:** a fallback at each of the six call sites (rejected: six copies of the warning and the blank rule). **Source:** design. **Evidence:** the readers at `cli.rs:404-405`, `git/oauth/store.rs:112`, `git/oauth/device_flow.rs:280`, `docs/ocr.rs:83`.

### Data model
None.

### Contracts
Warning line, exactly: `warning: environment variable <OLD> is deprecated, use <NEW>`, on standard error, once per process and per variable, at the first read. Precedence: new over old. A value that is empty or only whitespace is unset (as `cli.rs:536-552` and `git/oauth/store.rs:113` do today). The old names are `MCP_FS_CONFIG`, `MCP_FS_CONFIG_DIR`, `MCP_FS_CONFIG_NAME`, `MCPFS_TOKEN_KEY`.

### Paths
| Path | Action |
|---|---|
| `crates/core/src/env_compat.rs` | create: read by new then old name, blank is unset, warn once per process and name, test only reset |
| `crates/core/src/lib.rs` | `pub mod env_compat;` |
| `crates/core/src/cli.rs:19-23,343-366,404-405,426` | constants become `FSA_CONFIG`, `FSA_CONFIG_DIR`, `FSA_CONFIG_NAME` and read through the helper; the boot error text says `set FSA_CONFIG` |
| `crates/core/src/git/oauth/store.rs:26,112-113`, `git/oauth/cipher.rs:32,35` | `TOKEN_KEY_ENV = "FSA_TOKEN_KEY"` read through the helper; messages name `FSA_TOKEN_KEY` |
| `crates/fsa/tests/env_compat_e2e.rs` | create (DT-001) |

### Existing patterns
`resolve_config_path_with` (`cli.rs:366`) takes an injected `env` closure, which the helper must keep working with: the helper takes the closure as an argument. `store.rs:112` reads `std::env::var(TOKEN_KEY_ENV)` and treats a blank value as absent.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

The success criterion of the debt, restated: the existing suite passes unmodified except the rename edits declared for this story: the config resolution tests in `crates/core/src/cli.rs` that name `MCP_FS_CONFIG*` (`:458,474` and their expected tried lists are unchanged at this step) and the messages naming `MCPFS_TOKEN_KEY` in `git/oauth/store.rs` and `cipher.rs` tests (string rename, DR-003). Command: `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` (the container and the developer variable keep their names, DEC-005).

### SPEC-0099/DT-001: old environment variable falls back with one warning
- Level / Driver / Location: functional e2e / `crates/fsa/tests/env_compat_e2e.rs` (binary through `std::process::Command`, `env_clear`) and the unit tests of `crates/core/src/env_compat.rs` (resettable counter)
- **Driver:** the compiled binary through `assert_cmd` with `env_clear` and a temporary `XDG_CONFIG_HOME`, asserting standard error; the helper that reads a variable also has a unit test with a resettable counter (the once per process rule needs process state, which the in process resolver `cli.rs:353` cannot isolate).
- **Validates:** DR-006 for each of `MCP_FS_CONFIG`, `MCP_FS_CONFIG_DIR`, `MCP_FS_CONFIG_NAME`, `MCPFS_TOKEN_KEY`.
- Given only the old variable set to a valid value; When the server resolves it twice in one process; Then the value is used, and standard error holds exactly one line `warning: environment variable <OLD> is deprecated, use <NEW>`.
- Given both set to different values; Then the new value is used and the warning is still printed once.
- Given only the new set; Then standard error holds no warning.
- Concrete data: for `MCP_FS_CONFIG` a temporary file `old.yaml` is named; the binary is run with `--help`-less `keys --dir <tmp>` or any verb that resolves the config path, and standard error is read. For `MCPFS_TOKEN_KEY` a valid 32 byte base64 key is used and the unit test of the helper asserts the warning count.

## Constraints
- Only the four variables above move here; the secret name settings, MinIO and template names are US-0260.
- The user configuration directory is US-0261; do not change its probing here.
- No new dependency (DEC-012).
- The old names stay readable until release 1.0.0.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (a test that pins behavior which already holds says so instead).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: every test of the suite not named in the exceptions passes unchanged; `tool-contract-golden.json` schemas and descriptions do not change.
- [ ] DEBT: the existing suite passes unmodified, exceptions: the config resolution tests in `crates/core/src/cli.rs` that name `MCP_FS_CONFIG*` (`:458,474` and their expected tried lists are unchanged at this step) and the messages naming `MCPFS_TOKEN_KEY` in `git/oauth/store.rs` and `cipher.rs` tests (string rename, DR-003) (Section 8.4 of the design).
- [ ] Docs updated: prose that names what this story renames is left to US-0263, except where a documented command stops working (then fixed here).
- [ ] Self review (full): the diff renames only what the requirements name, the retained identifiers of Section 5 are untouched, no behavior moved.
- [ ] One commit `US-0259: environment variable fallback helper and the config and token variables` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0099` and the requirement ids.
