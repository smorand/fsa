# US-0256: rename the packages, the crate and the binary to fsa

> Spec: SPEC-0099 (sdlc/specs/SPEC-0099-rename-the-project-to-fsa/)
> Nature: DEBT
> Priority: P1
> Depends on: main
> min_tier: 2
> Files touched: 30
> Closes spec: no

## Objective
Rename the application package and binary `mcp-fs` to `fsa` (directory `crates/mcp-fs` to `crates/fsa`), the library package `mcp-fs-core` to `fsa-core` and its crate `mcp_fs_core` to `fsa_core`, and every build file, script and test that names them. Mechanical, about 30 files, no behavior change; the tier follows decision density, not the wide reach. Strings that clients read, environment variables and prose belong to later stories.

## Requirements covered
- SPEC-0099/DR-001: The workspace SHALL name its application package and binary `fsa`, its library package `fsa-core` with library crate `fsa_core`, in directory `crates/fsa` for the application, and SHALL keep the directory `crates/core` and the package `agent`.
- SPEC-0099/DR-005 (partial: this story keeps the suite green; each later story carries the same invariance for its own renames): The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.

## Independent Test
`make check` is green with the renamed packages, `cargo build -p fsa` writes `target/debug/fsa`, and `crates/fsa/tests/names.rs` (DT-011) passes.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0099): names are `fsa`, `fsa-core`, `fsa_core`; the directory `crates/mcp-fs` becomes `crates/fsa`; `crates/core` and the package `agent` keep their names (the package is a client of the server and `agent` carries no product name; its strings that name the product are renamed by DR-002). **Alternatives considered:** keep the directory name `crates/mcp-fs` (rejected: the old name would survive in a path), rename `crates/core` (rejected: the directory name was chosen for the skill layout, `AGENTS.md` Overview). **Rationale:** smallest set of renames that leaves no old name. **Implemented by:** DR-001. **Evidence:** `Cargo.toml:3`, `crates/core/Cargo.toml:2,12`.
- DEC-002 (SPEC-0099): stepwise migration, one green step at a time (Section 9), never one global replace. **Alternatives considered:** one global replacement (rejected: it would also rename the retained identifiers and break bisecting), a second crate keeping the old names for a cycle (rejected: duplicates the workspace). **Implemented by:** DR-001 to DR-009.

### Data model
None.

### Contracts
Package `fsa`, binary `fsa`, package `fsa-core`, crate `fsa_core`, directory `crates/fsa`; the package `agent` and the directory `crates/core` keep their names. Cargo features forwarded unchanged.

### Paths
| Path | Action |
|---|---|
| `git mv crates/mcp-fs crates/fsa` | move (src, tests, fixtures, Cargo.toml) |
| `crates/fsa/Cargo.toml` | package and `[[bin]]` name `fsa`; dependency `fsa-core`; features `postgres`, `rag`, `all-backends`, `rar` forwarded unchanged |
| `crates/core/Cargo.toml` | package `fsa-core`, `[lib] name = "fsa_core"` |
| `Cargo.toml` (`members`), `crates/agent/Cargo.toml:32`, `deny.toml:31`, `Cargo.lock` | package paths and names; `Cargo.lock` refreshed by cargo |
| every `use mcp_fs_core::` and `mcp_fs_core::` path (9 files, 23 occurrences) | `fsa_core` |
| `Makefile:23`, `run.sh:21,25`, `agent.sh`, `test.sh:7,10`, `build.sh`, `specs/audit.py:29,164,171`, `scripts/pty_check.py:136,151,157,266`, `scripts/doc_service_fake.py:199`, `tests/functional/**` (binary path and `-p` only), `docker-compose.test.yml:15` (comment) | `-p fsa`, `target/*/fsa`, `crates/fsa` |
| `crates/fsa/tests/names.rs` | create (DT-011) |

### Existing patterns
`crates/mcp-fs/tests/sql_server_removed.rs:11` reads the binary as `const BIN: &str = env!("CARGO_BIN_EXE_mcp-fs");`, which becomes `env!("CARGO_BIN_EXE_fsa")`. `crates/mcp-fs/src/main.rs:8` is `mcp_fs_core::cli::run().await` and becomes `fsa_core::cli::run().await`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

The success criterion of the debt, restated: the existing suite passes unmodified except the rename edits declared for this story: the moved tests under `crates/fsa/tests/` (`sql_server_removed.rs`, `crate_split.rs`, `makefile_targets.rs`, `security_gate.rs`, `toolchain_pin.rs`, `fixtures/`) with `CARGO_BIN_EXE_fsa`; the `use mcp_fs_core::` paths of `crates/core/tests/full_stack_e2e.rs` (5), `git_negotiation_e2e.rs` (3), `schema_golden.rs` (3) and `crates/agent/src/mcp.rs` (6); the golden regenerate hint `-p mcp-fs-core` to `-p fsa-core` (`tools/contract_golden.rs`, `tool-contract-golden.json:3`). Command: `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` (the container and the developer variable keep their names, DEC-005).

### SPEC-0099/DT-011: packages and binary
- Level / Driver / Location: integration / `crates/fsa/tests/names.rs`
- **Asserts:** DR-001. `cargo metadata --no-deps` lists packages `fsa`, `fsa-core` and `agent`, none named `mcp-fs*`; `cargo build -p fsa` produces `target/debug/fsa`; `fsa --version` prints a line starting `fsa `; `fsa version` prints the bare version, unchanged.
- Exact assertions: the package names from `cargo metadata --no-deps --format-version 1` are exactly `{agent, fsa, fsa-core}`; `target/debug/fsa --version` stdout starts with `fsa `.

## Constraints
- Do not rename `crates/core` or the package `agent` (DEC-001).
- Do not touch strings clients read, environment variables or prose (later stories).
- Do not touch the retained identifiers of Section 5.
- `Cargo.lock` is regenerated by cargo, never edited by hand.
- The function name `mcp_fs_src_contains_only_main_rs` and the argv[0] literals in tests are left to US-0262.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (a test that pins behavior which already holds says so instead).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: every test of the suite not named in the exceptions passes unchanged; `tool-contract-golden.json` schemas and descriptions do not change.
- [ ] DEBT: the existing suite passes unmodified, exceptions: the moved tests under `crates/fsa/tests/` (`sql_server_removed.rs`, `crate_split.rs`, `makefile_targets.rs`, `security_gate.rs`, `toolchain_pin.rs`, `fixtures/`) with `CARGO_BIN_EXE_fsa`; the `use mcp_fs_core::` paths of `crates/core/tests/full_stack_e2e.rs` (5), `git_negotiation_e2e.rs` (3), `schema_golden.rs` (3) and `crates/agent/src/mcp.rs` (6); the golden regenerate hint `-p mcp-fs-core` to `-p fsa-core` (`tools/contract_golden.rs`, `tool-contract-golden.json:3`) (Section 8.4 of the design).
- [ ] Docs updated: prose that names what this story renames is left to US-0263, except where a documented command stops working (then fixed here).
- [ ] Self review (full): the diff renames only what the requirements name, the retained identifiers of Section 5 are untouched, no behavior moved.
- [ ] Smoke of what `make check` does not run: `python3 scripts/pty_check.py --help`, `./agent.sh --help`, `./run.sh` up to its build line, `./test.sh` first command.
- [ ] One commit `US-0256: rename the packages, the crate and the binary to fsa` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0099` and the requirement ids.
