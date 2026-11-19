# US-0255: pandoc conversion reads no resource the source names

> Spec: SPEC-0098 (sdlc/specs/SPEC-0098-pandoc-conversion-reads-no-resources/)
> Nature: BUG
> Priority: P1
> Depends on: main
> min_tier: 2
> Files touched: 2
> Closes spec: yes

## Objective
A member's Markdown or HTML converted by `doc.to_docx` or `doc.to_pptx` must not make the server request an address or embed a host file. The reproduction and exploit tests are written first and shown red, then the single fix at `run_pandoc` makes them green.

## Requirements covered
- SPEC-0098/BR-001: The server SHALL NOT open a network connection to an address named inside the source file of `doc.to_docx` or `doc.to_pptx`.
- SPEC-0098/BR-002: The server SHALL NOT copy into the output any file of the server host that the source names, by absolute path, relative path or `file:` URL.
- SPEC-0098/BR-003: WHEN the source of `doc.to_docx` names a resource BR-001 or BR-002 forbids THE server SHALL write the `.docx` with the image replaced by its alt text and return the usual answer.
- SPEC-0098/BR-004: WHEN the source of `doc.to_pptx` names a resource BR-001 or BR-002 forbids THE server SHALL answer `ERR_INTERNAL_ERROR` with the message `pandoc exited with exit status: 99: File '<resource as written>' not found in resource path` and write no file.
- SPEC-0098/BR-005: The server SHALL NOT change the output for a source that names no forbidden resource, for a `data:` image, or for a conversion with `template_path`.

## Independent Test
`cargo test -p mcp-fs-core --lib -- tools::doc::tests::` passes with pandoc 2.15 or later on `PATH`; BT-001 asserts a `TcpListener` stand in accepted 0 connections, which no mock of the tool can fake.

## Design excerpt
### Decisions
- DEC-001 (SPEC-0098): run pandoc with `--sandbox`, no version probe. Pandoc then does no IO besides reading the files on its command line.
- DEC-003 (SPEC-0098): tests bind `std::net::TcpListener` on `127.0.0.1:0`, count accepted connections, and read the output with the existing `zip` and `tempfile` dependencies.
### Data model
None.
### Contracts
Success answer unchanged `{"path","bytes_written","overwritten"}`. pptx failure: `ERR_INTERNAL_ERROR` from the existing branch `doc.rs:309` (`format!("pandoc exited with {}: {stderr}", out.status)`).
### Paths
| Path | Action |
|---|---|
| `crates/core/src/tools/doc.rs` | `--sandbox` in `run_pandoc` (line 290); tests |
| `.agent_docs/tools.md` | paragraph on what `doc.*` may read, pandoc 2.15 floor |
### Existing patterns
Tests in `doc.rs` `mod tests` (`doc_to_docx_converts_markdown`, `doc.rs:437`): `harness_with_extra(register_with_pandoc_extra).await`, `h.seed(path, text).await`, `h.call("doc.to_docx", json!({"mount_id": MOUNT, "src_path": ..., "dst_path": ...})).await`, `h.client().await.read_bytes(path).await`, early return when `!pandoc_available()`.
Stand in: `let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); l.set_nonblocking(true)`; after the call, drain `l.accept()` until `WouldBlock` and count `Ok`.

## Acceptance tests
> 100% must pass, run through `make check`. Never run test files ad hoc.

### SPEC-0098/BT-001: a remote image in the Markdown causes no request
- Level / Driver / Location: functional e2e / `doc.to_docx` through the harness / `doc.rs` tests
- Given `/a.md` = `![x](http://127.0.0.1:PORT/a.png)`, the stand in listening on PORT.
- When `doc.to_docx` with `src_path=/a.md`, `dst_path=/a.docx`.
- Then the call succeeds with `path == "/a.docx"`, `overwritten == false`, **and** the stand in accepted 0 connections.
- Red before the fix because: the listener counts 1 connection.

### SPEC-0098/BT-002: metadata style address in HTML
- Level / Driver / Location: exploit / `doc.to_docx` / `doc.rs` tests
- Given `/p.html` = `<p>x</p><img src="http://127.0.0.1:PORT/h.png"><iframe src="http://127.0.0.1:PORT/i"></iframe>`.
- When `doc.to_docx`. Then success **and** 0 connections on the stand in.
- Red before the fix because: pandoc requests `/h.png` and `/i`.

### SPEC-0098/BT-003: host file by absolute path and `file:` URL
- Level / Driver / Location: exploit / `doc.to_docx` plus `zip` / `doc.rs` tests
- Given a temp file `SECRET.txt` holding `SECRET-MARKER-0098`, and `/f.md` = `![a](<abs path>)` blank line `![f](file://<abs path>)`.
- When `doc.to_docx`. Then success **and** the output zip has no entry whose name starts `word/media/`, and no entry's bytes contain `SECRET-MARKER-0098`.
- Red before the fix because: the docx holds `word/media/rIdN.txt` entries.

### SPEC-0098/BT-004: remote image, `.pptx`
- Level / Driver / Location: exploit / `doc.to_pptx` / `doc.rs` tests
- Given `/p.md` = `# T` blank line `![x](http://127.0.0.1:PORT/a.png)`, stand in listening.
- When `doc.to_pptx` with `dst_path=/p.pptx`. Then the error code is `ERR_INTERNAL_ERROR`, the message starts `pandoc exited with exit status: 99: File 'http://127.0.0.1:PORT/a.png' not found in resource path`, `/p.pptx` does not exist **and** the stand in accepted 0 connections.
- Red before the fix because: pandoc contacts the stand in and the exit is 61 or success.

### SPEC-0098/BT-010: docx keeps the alt text
- Given `/a.md` = `![ALT-0098](http://127.0.0.1:1/a.png)`. When `doc.to_docx`. Then `bytes_written > 0` and `word/document.xml` of the output contains `ALT-0098`.

### SPEC-0098/BT-011: failed pptx leaves the existing file
- Given `/p.pptx` seeded with bytes `B` through `h.client()`, source naming `http://127.0.0.1:1/a.png`. When `doc.to_pptx` with `overwrite=true`. Then `ERR_INTERNAL_ERROR` and `/p.pptx` still equals `B`.

### SPEC-0098/BT-020: data image still embedded
- Given `/d.md` = `![d](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==)`. When `doc.to_docx`. Then the zip has exactly 1 entry starting `word/media/` and ending `.png`.

### SPEC-0098/BT-021: template and plain sources unchanged
- Given a docx produced by the test with pandoc as template, and `/n.md` = `# T\n\nhello`. When `doc.to_docx` and `doc.to_pptx` run with and without `template_path` (pptx template made the same way). Then each returns `bytes_written > 0` and the output starts with `PK`.

## Constraints
- Files not to touch: `tool-contract-golden.json`, `TOOL_CONTRACT.txt`, tool descriptions in `doc.rs` (frozen contract).
- Dependencies not to add: any HTTP mock crate.
- Patterns to avoid: parsing the Markdown or HTML ourselves; a pandoc version probe.
- Scope boundary: the CLI mode of `doc_service` (`docs/service.rs`) belongs to BL-0161.

## Definition of done
- [ ] RED evidence recorded for BT-001 to BT-004 before the production change (command and failing assertion shown).
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit.
- [ ] Non regression: the sixteen existing `doc_to_docx_*` and `doc_to_pptx_*` tests and `tool_surface_e2e` counts (142 with pandoc) unchanged; `tool-contract-golden.json` unchanged.
- [ ] Docs updated: `.agent_docs/tools.md` paragraph; the doc comment of `run_pandoc` says conversions run sandboxed.
- [ ] Self review: why `--sandbox` closes both classes, and where it breaks (pandoc older than 2.15 fails closed; a pptx naming any image fails).
- [ ] Closes spec: the full acceptance set BT-001 to BT-021 green, and BR-001 to BR-005 each cited by this story.
- [ ] One commit `US-0255: pandoc conversion reads no resource the source names` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0098` and the requirement ids.
