# US-0258: the names sent to other systems are fsa

> Spec: SPEC-0099 (sdlc/specs/SPEC-0099-rename-the-project-to-fsa/)
> Nature: DEBT
> Priority: P1
> Depends on: US-0257
> min_tier: 2
> Files touched: 10
> Closes spec: no

## Objective
Rename the product names the server and the agent send to other systems: outbound `User-Agent` strings, the Context7 source header, the S3 credentials provider name, the git reflog message prefix, and the agent client name, about text and hint. DT-004 pins every renamed name of DR-002, those of US-0257 included.

## Requirements covered
- SPEC-0099/DR-002 (partial: the names sent to other systems: `User-Agent` of the git provider, OAuth, doc service, web and context7 calls, `X-Context7-Source`, the S3 provider name, the git reflog messages of new entries, the agent `clientInfo.name`, about text and hint): The server SHALL use the new names of the list below in every client facing name.
- SPEC-0099/DR-005 (partial: only the strings listed above change): The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.

## Independent Test
DT-004 passes: each renamed constant equals its new value and `fsa --help` starts `fsa:`.

## Design excerpt
### Decisions
- DEC-007 (SPEC-0099): `serverInfo.name`, git `agent`, `realm`, the `seed_commit` author, the outbound `User-Agent` strings, the OpenAPI tag, the Context7 header, the agent client name and the S3 provider name are renamed without alias. **Alternatives considered:** keep the old wire names (rejected: the product name would stay visible to every client). **Authorized by:** the same request. **Implemented by:** DR-002, Section 7.

### Data model
None.

### Contracts
Exact strings: `fsa/<version>`, `fsa-doc-service/1.0`, `fsa-web/1.0`, `fsa/1.0`, `X-Context7-Source: fsa`, provider name `fsa-config`, reflog prefix `fsa ` (for example `fsa git.commit`), MCP client name `fsa-agent`, hint `fsa token <person>`.

### Paths
| Path | Action |
|---|---|
| `crates/core/src/git/provider/mod.rs:37`, `git/oauth/device_flow.rs:29` | `User-Agent` `fsa/<version>` |
| `crates/core/src/docs/service.rs:534` | `fsa-doc-service/1.0` |
| `crates/core/src/tools/web.rs:302` | `fsa-web/1.0` |
| `crates/core/src/tools/context7.rs:90,104,146` | `User-Agent` `fsa/1.0`, header `X-Context7-Source: fsa` |
| `crates/core/src/storage/blob/s3.rs:66` | provider name `fsa-config` |
| `crates/core/src/tools/git_tags.rs:727,1212`, `tools/git.rs:2399,3734` | reflog message prefix `fsa ` for new entries (existing entries are not rewritten) |
| `crates/agent/src/mcp.rs:155`, `crates/agent/src/main.rs:27,433,627` | `clientInfo.name` `fsa-agent`, about text, hint `fsa token` |
| `crates/fsa/tests/names.rs` | add the `fsa --help` first line assertion (DT-004) |

### Existing patterns
Constants carrying a name are plain string literals; each is asserted by an in module test or is made visible by a small `pub(crate)` constant if none exists.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

The success criterion of the debt, restated: the existing suite passes unmodified except the rename edits declared for this story: `crates/agent/src/main.rs:627` (`mcp-fs token` to `fsa token`) and any reflog test found by `rg 'mcp-fs git\.'` (string rename, DR-002). Command: `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` (the container and the developer variable keep their names, DEC-005).

### SPEC-0099/DT-004: advertised names
- Level / Driver / Location: functional e2e and unit / in module tests named in Section A.3 of the design, plus `crates/fsa/tests/names.rs`
- **Validates:** DR-002. `initialize` over HTTP answers `serverInfo.name == "fsa"`; `GET /api/swagger.json` has `info.title == "FSA REST API"`; the git info refs advertisement carries `agent=fsa/0.1.0`; the `WWW-Authenticate` header of an unauthenticated git request carries `realm="fsa"`; a commit made by `seed_commit` has author `fsa <fsa@example.test>`; each outbound `User-Agent` constant equals its new value of DR-002; `fsa --help` first line starts `fsa:`.
- Exact assertions added by this story: the `User-Agent` constants of `git/provider/mod.rs`, `device_flow.rs`, `docs/service.rs`, `tools/web.rs`, `tools/context7.rs` equal `fsa/<version>`, `fsa-doc-service/1.0`, `fsa-web/1.0`, `fsa/1.0`; the Context7 header value is `fsa`; the S3 provider name is `fsa-config`; a reflog entry written after the change begins `fsa `; the agent sends `clientInfo.name == "fsa-agent"`; `crates/agent/src/main.rs:627` expects the hint `fsa token`.
- Red before the change because: each assertion names the new string and the code still holds the old one.

## Constraints
- Do not change the names of US-0257 again.
- Do not rewrite existing reflog entries or stored data.
- Do not touch the retained identifiers of Section 5 (`refs/mcp-fs/rebase-head`, `mcp-fs-stash: `).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (a test that pins behavior which already holds says so instead).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: every test of the suite not named in the exceptions passes unchanged; `tool-contract-golden.json` schemas and descriptions do not change.
- [ ] DEBT: the existing suite passes unmodified, exceptions: `crates/agent/src/main.rs:627` (`mcp-fs token` to `fsa token`) and any reflog test found by `rg 'mcp-fs git\.'` (string rename, DR-002) (Section 8.4 of the design).
- [ ] Docs updated: prose that names what this story renames is left to US-0263, except where a documented command stops working (then fixed here).
- [ ] Self review (full): the diff renames only what the requirements name, the retained identifiers of Section 5 are untouched, no behavior moved.
- [ ] One commit `US-0258: the names sent to other systems are fsa` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0099` and the requirement ids.
