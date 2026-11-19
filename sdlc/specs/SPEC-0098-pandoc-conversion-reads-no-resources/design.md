# SPEC-0098: Document conversion reads no resource the source names, design

> Spec: sdlc/specs/SPEC-0098-pandoc-conversion-reads-no-resources/spec.md
> Nature: BUG
> Depth: S
> Security: internal finding
> Designed on: 2026-11-18
> Target tier: 2 (default, no `--tier`, no constitution field, no `.spec.json`)

## 1. Stack and skills
| Element | Choice | Version | DEC |
|---|---|---|---|
| Language | Rust, edition 2024 (existing) | per `Cargo.toml` | none |
| Converter | pandoc option `--sandbox` | pandoc 2.15 or later (option introduced in 2.15, release notes https://newreleases.io/project/github/jgm/pandoc/release/2.15); 3.14 verified on this host | DEC-001 |
| Test helpers | `zip` and `tempfile` (already dependencies of `crates/core`, `Cargo.toml:68,96,123`), `std::net::TcpListener` | existing | DEC-003 |

Skills loaded: `rust`. Effect: tests live in the module's `#[cfg(test)] mod tests`, no `unwrap` in production code, quality gate is the project's `make check` (`AGENTS.md` Quality gate).

## 2. Approaches
### Chosen: stop pandoc reading resources at the command line (root cause site)
Pass `--sandbox` on every run. Pandoc then does no IO besides reading the files named on its command line, so no URL is fetched and no host file is embedded. Pros: one argument, closes the network and file classes together, no address parsing to get wrong. Cons: a docx image that was fetched before is now alt text; a pptx naming an image fails (BR-003, BR-004). Scope: `tools/doc.rs`. Best suited: the converter must not reach outside its input.
### Rejected: apply the SPEC-0097 black and white lists to every image address
Needs a Markdown and HTML parser of our own to find the addresses, plus a second resolution that pandoc would repeat, so a rebind between the two defeats it; and it leaves host files readable. Rejected (DEC-002).
### Rejected: strip images from the source before pandoc
Our own parse of Markdown and HTML, easy to bypass with an exotic syntax, and a new capability. Rejected (DEC-002).

## 3. Architecture
`convert` (`doc.rs:192`) writes the source to a temp directory and calls `run_pandoc` (`doc.rs:283`). Only `run_pandoc` changes: the argument list gains `--sandbox` before the input. `doc.to_docx` and `doc.to_pptx` both go through it, so one site fixes both tools, Markdown and HTML sources. The REST plane has no `doc.*` route (`tools/doc.rs` is registered only through the tool registry), so there is no second implementation to align.

## 4. Data model
None.

## 5. Contracts and interfaces
No tool schema or description changes (`tool-contract-golden.json` untouched). Answers:
- success: `{"path": <dst_path>, "bytes_written": <n>, "overwritten": <bool>}` as today.
- pptx naming a resource: `ERR_INTERNAL_ERROR`, message `pandoc exited with exit status: 99: File '<resource as written>' not found in resource path`, produced by the existing branch at `doc.rs:309`; no change to it.

## 6. File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/src/tools/doc.rs` | modify: `--sandbox` in `run_pandoc` (`doc.rs:290`), update the doc comment of `run_pandoc`, add tests | BR-001 to BR-005 | US-0255 |
| `.agent_docs/tools.md` | modify: one paragraph on what `doc.*` conversion may read and the pandoc 2.15 floor | BR-001, BR-002 | US-0255 |
Affected existing tests: the sixteen `doc_to_docx_*` and `doc_to_pptx_*` tests (`doc.rs:437-736`) and `tests/functional/scenarios/11_pandoc.sh`: kept, their sources name no resource.
Affected documentation: `.agent_docs/tools.md`. `README.md` and `AGENTS.md`: no change (no doc tool section exists, `rg` of both found none).

## 7. Compatibility and risks
- A docx whose Markdown linked a remote image loses the image (alt text instead); a pptx doing so now fails. Declared by BR-003 and BR-004.
- Pandoc older than 2.15 rejects `--sandbox`: the conversion fails closed with pandoc's own message through `doc.rs:309`. No version probe is added (nothing requires one, DEC-001).
- Rollback: revert one argument; no data is touched.
- No dependency added or removed.

## 8. Security design
- Threat surface: member authored Markdown or HTML (untrusted source) reaching pandoc's resource loader (sink): network (CWE-918) and host files (CWE-73).
- Occurrence `doc.rs:289-294`: fixed at that site, mechanism `--sandbox`, DEC-001. It is the occurrence that triggered the spec.
- Occurrence `docs/service.rs:394`: out of this lot by owner decision, BL-0161.
- Exploit tests: BT-002, BT-003, BT-004 each carry the literal payload (`<img src="http://127.0.0.1:PORT/h.png">`, `![a](<abs path>)` and `![f](file://<abs path>)`, `![x](http://127.0.0.1:PORT/a.png)`). Absence of effect is read through a second channel: the stand in listener's accept count, and the output zip's entries, never the tool answer. Each is red today for the root cause (pandoc runs without `--sandbox`).
- Nothing secret is logged: pandoc's stderr tail is already the error text (`doc.rs:309`) and holds only the resource name written by the member.

## 9. Test strategy
| Test | Level | Driver | Location | Story |
|---|---|---|---|---|
| BT-001 | functional e2e | tool call `doc.to_docx` through the harness plus a `TcpListener` | `crates/core/src/tools/doc.rs` tests | US-0255 |
| BT-002 | exploit | same | same | US-0255 |
| BT-003 | exploit | same plus `zip` reader | same | US-0255 |
| BT-004 | exploit | tool call `doc.to_pptx` plus listener | same | US-0255 |
| BT-010 | functional e2e | `doc.to_docx` plus `zip` reader | same | US-0255 |
| BT-011 | functional e2e | `doc.to_pptx` with `overwrite=true` | same | US-0255 |
| BT-020 | functional e2e | `doc.to_docx` plus `zip` reader | same | US-0255 |
| BT-021 | functional e2e | `doc.to_docx` and `doc.to_pptx`, with and without template | same | US-0255 |
Each test skips with `eprintln!("skipped: pandoc not found")` when pandoc is absent, like its neighbours (`doc.rs:441`). Red before the fix: BT-001 (listener count 1, expected 0), BT-002 (count 2), BT-003 (`word/media/` entries present), BT-004 (success or exit 61 instead of exit 99 and the listener count 1). BT-010, BT-011, BT-020, BT-021 are pins that already hold today and must keep holding. Test data: generated in the test (temp files, in memory png data URI).

## 10. Decisions log
- **DEC-001:** run pandoc with `--sandbox`, no version probe. **Obliged by:** SPEC-0098/BR-001, BR-002, BR-003, BR-004. **Rationale:** it is the converter's own switch for "no IO beyond the command line files", it closes network and file classes with one argument at the root cause site, and an older pandoc fails closed. **Alternatives considered:** address lists (DEC-002), stripping images (DEC-002), a startup version probe (nothing obliges it; the failure is already clear). **Source:** design. **Evidence:** `crates/core/src/tools/doc.rs:290`; behavior verified with pandoc 3.14 on 2026-11-18 (no request, `rc=0` docx, `rc=99` pptx); release notes of 2.15.
- **DEC-002:** reject address list filtering and source rewriting. **Obliged by:** SPEC-0098/BR-001, BR-002. **Rationale:** both need our own parsing of the document, leave a resolution gap, and the second adds a capability. **Alternatives considered:** the two above. **Source:** design. **Evidence:** `tools/web_access.rs` is built for `web.*` requests the server itself sends, not for a subprocess.
- **DEC-003:** tests use `std::net::TcpListener` bound to `127.0.0.1:0` and count accepted connections without answering, plus the existing `zip` and `tempfile` dependencies. **Obliged by:** SPEC-0098/BR-001, BR-002. **Rationale:** the effect is read through a channel other than the tool answer; no new dependency. **Alternatives considered:** a mock HTTP server crate (new dependency, category 2). **Source:** existing code `crates/core/src/tools/web_testkit.rs` pattern of stand in servers. **Evidence:** `crates/core/Cargo.toml:68,96,123`.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0255 | pandoc conversion reads no resource the source names | P1 | BR-001, BR-002, BR-003, BR-004, BR-005 | main | 2 | yes |
