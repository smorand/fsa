# Document conversion reads no resource the source names: Bug Specification

> Generated on: 2026-11-18
> Id: SPEC-0098
> Nature: BUG
> Depth: S
> Depth evidence: 1 module (`tools/doc.rs`), 5 requirements, no public contract change, no data model change
> Status: implemented
> Verdict: no authority
> Defect class: absent
> Severity: High
> From backlog: BL-0159
> Security: internal finding
> CVSS: not scored
> Affected: every build since `doc.to_docx` and `doc.to_pptx` exist, on installations where `doc.enabled` is true and pandoc is installed
> Fixed in: unreleased

## 1. Defect

### BUG-001: pandoc resolves every resource the member's document names
- **Observed behavior:** `doc.to_docx` and `doc.to_pptx` write the member's `.md`, `.markdown`, `.html` or `.htm` file to a temp directory and run pandoc on it with no restriction (`crates/core/src/tools/doc.rs:289-294`). Pandoc then loads each image or frame the document names. Verified with pandoc 3.14 on 2026-11-18: `![x](http://127.0.0.1:8765/remote.png)` produced `GET /remote.png` on a stand in server; `<img src=...>` and `<iframe src=...>` in HTML did the same; `![h](/etc/hosts)` embedded the file as `word/media/rId9.so` in the `.docx`; `![f](file:///tmp/pt/sec.txt)` and a bare absolute path embedded it as `word/media/rId9.txt`.
- **Expected per authority:** none: no authority. `SPEC-0006-documents/spec.md:281` (FR-019) says "convert the named Markdown or HTML file in the volume" and is silent on resources. `SPEC-0097-web-access-lists/spec.md:61` defers this case to BL-0159.
- **Reproduction:** Given pandoc installed and `doc.enabled: true`, a member writes `/a.md` containing `![x](http://127.0.0.1:PORT/a.png)` and calls `doc.to_docx` with `src_path=/a.md`, `dst_path=/a.docx`. Expected: no connection to `127.0.0.1:PORT`. Actual: one connection.
- **Root cause:** `crates/core/src/tools/doc.rs:290-294` builds the pandoc command from the input, `-o`, and an optional `--reference-doc`, and no option that stops pandoc reading resources. Pandoc's default is to fetch URLs and read local paths relative to the server's working directory.
- **Contributing factors:** the source is member authored content; the conversion tests use sources with no resource (`doc.rs:437-476`, `588-626`); the only network control, SPEC-0097, covers `web.*` and not subprocesses.
- **Impact:** a member makes the server request any address (loopback, private network, cloud metadata, CWE-918) and read any file of the server host that pandoc can embed (CWE-73), the bytes landing in a `.docx` the member owns. Exposed only where the document tools are enabled (off by default, `config.rs:826-833`). No workaround except leaving them off.
- **Rollback posture:** a member whose docx relied on a fetched image can still convert it: the image becomes its alt text, or is embedded through a `data:` URI. The `.docx` and `.pptx` already written are untouched.

## 1bis. Class Sweep

**Class:** content supplied by a project member makes a program the server launches, or the server itself, read a network address or a host file that the content names (server side request forgery, CWE-918, and uncontrolled resource path, CWE-73).

**Searches run (2026-11-18):**
1. `rg -n 'tokio::process::Command|std::process::Command|Command::new' crates --glob '!**/*test*' --glob '!**/tests/**'`: subprocess idiom. Hits: `storage/blob/s3.rs:291,348,384,408` (inside tests), `tools/converter_bound_e2e.rs:129` (test), `tools/editor.rs:595,597`, `docs/service.rs:394`, `tools/doc.rs:289`.
2. `rg -n 'reqwest::|Client::builder|\.send\(\)' crates --glob '!**/*test*' -l`: in process HTTP idiom. 19 files, listed in the table where they take a member address.
3. `rg -n -i 'pandoc|wkhtml|libreoffice|soffice|chromium|ffmpeg|rsvg' --glob '!sdlc/**' --glob '!target/**' --glob '!*.md' -l .`: converter names. Only `doc.rs`, `config.rs`, `all.rs`, `tool_surface_e2e.rs` and functional scripts name pandoc.
Not searched: Python and shell under `scripts/` (test fakes, not shipped as server code).

| Occurrence | Verdict | Disposition |
|---|---|---|
| `crates/core/src/tools/doc.rs:289-294` | vulnerable | BR-001 to BR-005 |
| `crates/core/src/docs/service.rs:394` (CLI `doc_service` converter on member documents) | not verifiable: an external program | P1, owner decision of 2026-11-18: the CLI mode is legacy and is to be removed |
| `crates/core/src/docs/service.rs:560` (API mode `doc_service`) | safe: the address is `doc_service.api.url`, an operator setting (`config.rs:899`), never read from a member document | none |
| `crates/core/src/tools/web.rs` | safe: member address, controlled by SPEC-0097 lists (`tools/web_access.rs`) | SPEC-0097 |
| `crates/core/src/git/remote.rs:246` (member remote URL) | safe: https only, host must be in the operator's `git.hosts` (`git/remote.rs:182-187`) | none |
| `crates/core/src/tools/editor.rs:595,597` | safe: `open`/`xdg-open` of the server's own URL, not member content | none |
| other `reqwest` users (`search/*`, `docs/ocr.rs`, `git/provider`, `git/oauth`, `storage/blob/s3.rs`, `app.rs`, `cli.rs`) | safe: endpoints come from the operator's configuration | none |
| HTML text extraction (`docs/extract.rs:10`, hand rolled tag stripper) | safe: strips tags, opens nothing | none |

## 2. Expected Behavior

#### BR-001 [EARS-UB]: no network request for a named resource
> The server SHALL NOT open a network connection to an address named inside the source file of `doc.to_docx` or `doc.to_pptx`.

- **Source:** decided here: owner statement in BL-0159 ("a member can therefore make the server request any address"), and the SPEC-0097 deferral.
- **Inputs:** a `.md`, `.markdown`, `.html` or `.htm` source with `http://` or `https://` images, `<img src>`, `<iframe src>`, `<link href>` or `<svg><image href>`.
- **Outputs:** zero connections, whatever the address.
- **Exact names:** tools `doc.to_docx`, `doc.to_pptx`.

#### BR-002 [EARS-UB]: no host file embedded
> The server SHALL NOT copy into the output any file of the server host that the source names, by absolute path, relative path or `file:` URL.

- **Source:** decided here: same class sweep, same line of code. Owner approval of 2026-11-18 ("Ok").
- **Outputs:** the `.docx` and `.pptx` hold no `word/media/` or `ppt/media/` entry coming from such a name.

#### BR-003 [EARS-E]: docx keeps going without the resource
> WHEN the source of `doc.to_docx` names a resource BR-001 or BR-002 forbids THE server SHALL write the `.docx` with the image replaced by its alt text and return the usual answer.

- **Source:** decided here: ASSUMED, the least change from today (pandoc already replaces an image it cannot load with its description).
- **Outputs:** `{"path": <dst_path>, "bytes_written": <n>, "overwritten": <bool>}`, same fields and types as today; the call succeeds.

#### BR-004 [EARS-E]: pptx refuses a source that names such a resource
> WHEN the source of `doc.to_pptx` names a resource BR-001 or BR-002 forbids THE server SHALL answer `ERR_INTERNAL_ERROR` with the message `pandoc exited with exit status: 99: File '<resource as written>' not found in resource path` and write no file.

- **Source:** decided here: ASSUMED. Pandoc's pptx writer treats an unresolved image as an error (exit 99, verified 2026-11-18); the pass through of the pandoc failure is FR-012 behavior (`SPEC-0006-documents/spec.md:358`, `doc.rs:309`).
- **Business Rules:** `dst_path` is not created and an existing `dst_path` is not changed.

#### BR-005 [EARS-UB]: nothing else moves
> The server SHALL NOT change the output for a source that names no forbidden resource, for a `data:` image, or for a conversion with `template_path`.

- **Source:** decided here: non regression of FR-019 (`SPEC-0006-documents/spec.md:281`).
- **Business Rules:** a `data:image/png;base64,...` image is embedded as today; `template_path` is applied as today for docx and pptx.

## 3. Edge Cases

| Case | Expected behavior | Requirement |
|---|---|---|
| Source names `http://169.254.169.254/...` | no request; docx alt text, pptx error | BR-001, BR-003, BR-004 |
| `.html` with `<iframe src="http://...">` | no request | BR-001 |
| Source names `file:///etc/hosts` | not embedded | BR-002 |
| Relative name `secret.png` | not read from the server's working directory | BR-002 |
| Name that does not exist | same as a forbidden one: alt text (docx), error (pptx) | BR-003, BR-004 |
| Overwrite of an existing `dst_path` with a pptx that fails | existing file unchanged | BR-004 |

## 4. Tests

### 4.1 End to end reproduction (mandatory)
#### BT-001: a remote image in the Markdown causes no request
- **Reproduces:** BUG-001
- **Driver:** the `doc.to_docx` tool call through the test harness (`h.call`, as `doc_to_docx_converts_markdown`, `doc.rs:437`), a `TcpListener` stand in on `127.0.0.1:0`; skipped when pandoc is absent, as the neighbours.
- **Fails today because:** `doc.rs:290-294` lets pandoc fetch the image, so the stand in counts 1 connection.
- Given `/a.md` = `![x](http://127.0.0.1:PORT/a.png)` and the stand in listening. When the member calls `doc.to_docx` with `src_path=/a.md`, `dst_path=/a.docx`. Then the call succeeds with `path == "/a.docx"` and `overwritten == false`, **and** the stand in accepted exactly 0 connections.

### 4.1bis Exploit tests
#### BT-002: metadata address in HTML, `.docx`
- **Covers:** `doc.rs:289-294`, network class. **Red before the fix because:** pandoc requests `/h.png` and `/i` on the stand in (verified 2026-11-18).
- Given `/p.html` = `<p>x</p><img src="http://127.0.0.1:PORT/h.png"><iframe src="http://127.0.0.1:PORT/i"></iframe>`. When `doc.to_docx` runs. Then it succeeds. **And** the stand in counts 0 connections (read from the listener, not from the tool answer).

#### BT-003: host file by absolute path and `file:` URL, `.docx`
- **Covers:** `doc.rs:289-294`, file class. **Red before the fix because:** the docx holds `word/media/rIdN.txt` (verified 2026-11-18).
- Given a file `SECRET.txt` containing `SECRET-MARKER-0098` in a temp dir, and `/f.md` = `![a](<abs path of SECRET.txt>)` then `![f](file://<abs path of SECRET.txt>)`. When `doc.to_docx` runs. Then it succeeds. **And** the output zip, read through a zip reader, has no entry starting `word/media/` and no entry holds `SECRET-MARKER-0098`.

#### BT-004: remote and file names, `.pptx`
- **Covers:** `doc.rs:289-294`, both classes through `doc.to_pptx`. **Red before the fix because:** today the pptx call contacts the stand in (success or exit 61), and embeds the file.
- Given `/p.md` = `# T` then `![x](http://127.0.0.1:PORT/a.png)`, and a stand in. When `doc.to_pptx` runs. Then the answer is `ERR_INTERNAL_ERROR` whose message starts `pandoc exited with exit status: 99: File 'http://127.0.0.1:PORT/a.png' not found in resource path`. **And** `/p.pptx` does not exist and the stand in counts 0 connections.

### 4.2 New behavior
#### BT-010: docx with a forbidden image keeps the alt text
- **Validates:** BR-003. Given `/a.md` = `![ALT-0098](http://127.0.0.1:1/a.png)`. When `doc.to_docx`. Then the answer has `bytes_written > 0`, and `word/document.xml` of the output contains `ALT-0098`.

#### BT-011: pptx failure leaves an existing file
- **Validates:** BR-004. Given `/p.pptx` exists with bytes `B` and `overwrite=true`, a source naming a remote image. When `doc.to_pptx`. Then `ERR_INTERNAL_ERROR` and `/p.pptx` still equals `B`.

### 4.3 Non regression
#### BT-020: data image still embedded
- **Protects:** BR-005. Given `/d.md` = `![d](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==)`. When `doc.to_docx`. Then the zip has exactly one entry starting `word/media/` ending `.png`.

#### BT-021: template and plain sources unchanged
- **Protects:** BR-005. Given a docx made by pandoc as template and `/n.md` = `# T\n\nhello`. When `doc.to_docx` and `doc.to_pptx` run with and without `template_path`. Then each returns `bytes_written > 0` and the output starts with `PK`.

### 4.4 Existing tests asserting the current behavior
| Test file | Test name | Action | Reason |
|---|---|---|---|
| `crates/core/src/tools/doc.rs` | `doc_to_docx_converts_markdown`, `doc_to_docx_converts_html`, `doc_to_docx_no_clobber`, `doc_to_docx_overwrite_allowed`, `doc_to_docx_rejects_wrong_dst_ext`, `doc_to_docx_rejects_wrong_src_ext`, `doc_to_docx_missing_src`, `doc_to_docx_missing_template` | Kept | no resource in their sources |
| `crates/core/src/tools/doc.rs` | the eight `doc_to_pptx_*` twins (`588-736`) | Kept | same |
| `tests/functional/scenarios/11_pandoc.sh` | report, page and pres conversions | Kept | no resource in their sources |

## 5. Existing Specs Affected
- `SPEC-0006-documents/spec.md:281` (FR-019): not amended; this spec narrows what the conversion may read, which FR-019 did not state.
- `SPEC-0097-web-access-lists/spec.md:61`: the deferral it names is closed here.

## 6. Assumptions
- **Assumption:** BR-003 and BR-004 follow pandoc's own behavior (alt text for docx, exit 99 for pptx). The owner was told this on 2026-11-18 and did not object; a pptx that names a remote image fails where today it may succeed. Alternative if rejected: strip images before the pandoc run, a new capability.
- **Assumption:** BR-002 is in this spec because it shares the root cause line; owner answered "Ok".
- **Proposed backlog entries (not written):** P1: remove the CLI mode of `doc_service` (legacy per the owner; the API mode stays). `Nature: DEBT` or `FEAT`, to be set when added; Evidence `docs/service.rs:394`; Decision: owner statement of 2026-11-18 "external service using CLI is legacy and must be removed".

## 7. Implementability Checklist
| Item | Result |
|---|---|
| 1 no technical decision | PASS: the fix mechanism is not named; `doc.rs:289-294` is a fact |
| 2 no buried requirement | PASS |
| 3 observable names | PASS: tools, `ERR_INTERNAL_ERROR`, answer fields, exact pptx message |
| 4 no forced choice | PASS: docx success, pptx error, both fixed |
| 5 tests specified | PASS: BT-001 to BT-021, 4.4 complete |
| 6 end to end reproduction | PASS: BT-001 asserts the stand in count |
| 7ante / 7bis security | PASS: class stated without module; 3 searches; hits cited; exploit tests BT-002 to BT-004 carry literal payloads and read the effect from the listener or the zip |
| 8 hidden prerequisite | PASS: pandoc installed and `doc.enabled` stated in Given |
| 9 EARS | PASS |
| 10 right tool | PASS: behavior changes, no new capability |
| 11 length | PASS |
| 12 claims cited | PASS |
| 13 round trip | PASS: writer is the temp file (`doc.rs`, step 5), reader is pandoc |
| 14 verdict consistent | PASS: `no authority`, class `absent` |

| Verdict | IMPLEMENTABLE |
|---|---|
| Open F failures | 0 |
| A failures | amended in place: 0; proposed: P1 |
