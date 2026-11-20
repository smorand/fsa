# US-0257: the names clients read are fsa

> Spec: SPEC-0099 (sdlc/specs/SPEC-0099-rename-the-project-to-fsa/)
> Nature: DEBT
> Priority: P1
> Depends on: US-0256
> min_tier: 2
> Files touched: 8
> Closes spec: no

## Objective
Rename the names a client reads: the MCP server name, the OpenAPI title, tag and page title, the command line name, its about text and startup banner, the git agent capability and realm, and the author written by the `seed_commit` test helper. Each name is asserted by an existing test that is edited first, so the story is red before the code.

## Requirements covered
- SPEC-0099/DR-002 (partial: the names clients read: MCP `serverInfo.name`, OpenAPI title, tag and page title, clap name, `ABOUT`, startup banner, git `agent` and `realm`, the `seed_commit` author): The server SHALL use the new names of the list below in every client facing name.
- SPEC-0099/DR-005 (partial: only the strings listed above change): The server SHALL produce, for every input covered by the existing test suite, byte identical observable output before and after this change, except the strings that DR-002 and DR-003 rename.

## Independent Test
`fsa --help` first line starts `fsa:`; an `initialize` request answers `serverInfo.name == "fsa"`; `GET /api/swagger.json` has `info.title == "FSA REST API"` and the tag `fsa`; the git advertisement carries `agent=fsa/0.1.0`.

## Design excerpt
### Decisions
- DEC-007 (SPEC-0099): `serverInfo.name`, git `agent`, `realm`, the `seed_commit` author, the outbound `User-Agent` strings, the OpenAPI tag, the Context7 header, the agent client name and the S3 provider name are renamed without alias. **Alternatives considered:** keep the old wire names (rejected: the product name would stay visible to every client). **Authorized by:** the same request. **Implemented by:** DR-002, Section 7.

### Data model
None.

### Contracts
Exact strings: MCP `serverInfo.name` `fsa`; OpenAPI `info.title` `FSA REST API`, tag name `fsa`; Swagger page `<title>FSA API</title>`; clap command `fsa`; `ABOUT` text `fsa: filesystem MCP server (SQLite metadata, object store or local blobs)`; banner `Serving fsa {} on {}:{} (config={})`; git capability string `agent=fsa/0.1.0` (appended to the existing capability lists unchanged otherwise); `WWW-Authenticate: Bearer realm="fsa"`; author `fsa <fsa@example.test>`.

### Paths
| Path | Action |
|---|---|
| `crates/core/src/mcp/server.rs:39` | `SERVER_NAME = "fsa"` |
| `crates/core/src/api/openapi.rs:91,110,211,281,1580` | title `FSA REST API`, tag `fsa`, page title `FSA API` |
| `crates/core/src/cli.rs:33,36-38,43,322` | `ABOUT` begins `fsa:`, clap `name = "fsa"`, banner `Serving fsa {} on {}:{} (config={})` |
| `crates/core/src/git/http/mod.rs:68,70,256` | `agent=fsa/0.1.0`, `Bearer realm="fsa"` |
| `crates/core/src/git/odb.rs:528-529` | `seed_commit` author `fsa <fsa@example.test>` |

### Existing patterns
The server name is the constant `SERVER_NAME` at `server.rs:39`, asserted at `app.rs:577` as `assert_eq!(v["result"]["serverInfo"]["name"], "mcp-fs");`. The git capability strings are literals at `git/http/mod.rs:68,70`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

The success criterion of the debt, restated: the existing suite passes unmodified except the rename edits declared for this story: `app.rs:577`, `api/openapi.rs:1676,1679,2003`, `git/http/mod.rs:641,645,669,689,1245`, `git_negotiation_e2e.rs:1781`, `git/odb.rs:883` (string rename, DR-002). Command: `docker start mcp-fs-rust-postgres-1; MCPFS_TEST_PG_DSN=postgres://mcpfs:mcpfs@127.0.0.1:55432/mcpfs make check` (the container and the developer variable keep their names, DEC-005).

### Edited existing tests (they are the acceptance tests of this story; edit first, observe red, then change the code)
- `crates/core/src/app.rs:577`: `serverInfo.name` expected `"fsa"`.
- `crates/core/src/api/openapi.rs:1676` expects title `FSA REST API`, `:1679` expects tag `fsa`, `:2003` expects `<title>FSA API</title>`.
- `crates/core/src/git/http/mod.rs` tests at `:641,645,669,689` expect `agent=fsa/0.1.0`; `:1245` expects `realm="fsa"`; `crates/core/tests/git_negotiation_e2e.rs:1781` (`OFFERED`) expects `agent=fsa/0.1.0`.
- `crates/core/src/git/odb.rs:883` expects author `fsa`.
- Red before the change because: each of these assertions names the old string.

## Constraints
- Do not change outbound `User-Agent` strings, the Context7 header, the S3 provider name, reflog messages or the agent client name (US-0258).
- Do not touch the retained identifiers of Section 5 (`realm` is the only auth string that changes).
- The version in `agent=fsa/0.1.0` stays `0.1.0`.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (a test that pins behavior which already holds says so instead).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: every test of the suite not named in the exceptions passes unchanged; `tool-contract-golden.json` schemas and descriptions do not change.
- [ ] DEBT: the existing suite passes unmodified, exceptions: `app.rs:577`, `api/openapi.rs:1676,1679,2003`, `git/http/mod.rs:641,645,669,689,1245`, `git_negotiation_e2e.rs:1781`, `git/odb.rs:883` (string rename, DR-002) (Section 8.4 of the design).
- [ ] Docs updated: prose that names what this story renames is left to US-0263, except where a documented command stops working (then fixed here).
- [ ] Self review (full): the diff renames only what the requirements name, the retained identifiers of Section 5 are untouched, no behavior moved.
- [ ] One commit `US-0257: the names clients read are fsa` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0099` and the requirement ids.
