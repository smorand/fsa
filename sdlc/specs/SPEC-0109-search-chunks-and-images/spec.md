# Search chunks and search images: functional specification

> Id: SPEC-0109
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 3 areas (engine reads by vector, chunk search, image search), 12 requirements, no new persisted concept. Auto run, part 3 of 5 (BL-0172 part 1/2); part 4 (search documents, BL-0178) and part 5 (file metadata screen) are out of this spec.
> Status: implemented
> Generated on: 2026-11-26
> Project: existing
> From backlog: BL-0172
> Depends on: SPEC-0108
> Security: none
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
A project stores vectors of its chunks and of its images with the models of its profile (SPEC-0107, SPEC-0108), but the only search, `search.query`, takes one text and returns documents with a score that is not a similarity. This increment adds two read tools. `search.chunks` takes one or several labeled inputs, texts and images, and answers for each input its best chunks as lines (input, document, chunk, similarity) with optional chunks before and after marked as context. `search.images` takes texts or images and answers the closest stored images. `search.query` is not touched.

## 2. Current State (MANDATORY, every depth)
`search.query(mount_id, query, mode, top_k, rerank)` runs one query on the BM25 engine, the vector engine or both and merges whole documents with a reciprocal rank fusion keyed by path. Its vector score is `1 - distance` on SQLite and `1 - distance / 2` on PostgreSQL. No tool takes an image as input, returns a similarity of a chunk, returns neighbour chunks of a hit, or searches the image vectors that SPEC-0108 stores. The vector engines expose no read by vector; `query_vector` embeds the text itself and returns a path level result.

### 2.1 What users can do today
- Search one text over the whole project (14.1 row 1).
- Read the vectors of a file (`fs.get_vectors`) but not search them by an image (14.1 row 2).

### 2.2 Specs governing this area
- SPEC-0108: vectors per project dimension and kind; `vectors_of`; marker and recalculation. Reused unchanged.
- SPEC-0107: profile models used to embed the input.
- SPEC-0101: chunks of the default set; the text and neighbours of a hit.
- SPEC-0103 and SPEC-0106: the converter that turns an image into text.
- SPEC-0100: unchanged.

### 2.3 What is verified today
`search.query` is covered by `search/e2e.rs` and `settings/e2e.rs`; nothing pins its exact answer byte for byte (14.1 row 3).

## 3. Scope
### 3.1 In scope
- `search.chunks` and `search.images`, MCP only.
- Engine reads: embed an input at the project width, nearest entries of a kind, similarity of given entries.
- Fusion of a dense and a keyword list, per input.
- Conversion of an image input to text by the project converter.
- A regression test that pins `search.query`.
- PostgreSQL tests.
### 3.2 Out of scope, and why
- Search documents, reconciliation formula and parameters: part 4 (BL-0178).
- Any change of `search.query`, its scores or its REST twin: the owner asked for byte identity.
- A REST route or a screen: not asked (YAGNI).
- Reranking: not in the approved ordering.
### 3.3 Constraints imposed by the context
- The six `search.*` tools are config gated and outside the 152 tools of the frozen contract: `tool-contract-golden.json` is regenerated and shows no change, `TOOL_CONTRACT.txt` documents the two new tools.
- Vector features compile with `--features rag`; the default build refuses both tools at call time with `ERR_NOT_SUPPORTED` when search has no vectors.
- No secret in any answer or log; a query vector is never returned.

## 4. Actors
| Actor | Role |
|---|---|
| Member | searches a project; its image inputs are files it can read |
| Outsider and platform administrator without membership | refused |

## 4.5 Bounded contexts
Search (engines, fusion), chunking (text and neighbours of a hit), converters (image to text).

## 5. Usage scenarios
### SC-001: Search one text
Alice sends one text input; she gets the best chunks as lines (S1, document, chunk, similarity) with their text, best first.
### SC-002: Search with several inputs, texts and an image
Alice sends two texts and a picture of a chart; the picture is turned into text by the converter; she gets three groups S1, S2, I1, each with its own lines.
### SC-003: Read around a hit
Alice asks for one chunk before and after; each hit carries its neighbours, marked as context, each with its own similarity.
### SC-004: Search images
Alice sends a picture, or a text, and gets the closest images of the project with the text stored beside them (caption and OCR) and the similarity.
### SC-005: Refusals
An empty or malformed input list, an unreadable or missing image, a missing converter or image model, a project whose mode has no vectors, search off, an outsider, an embedding service that is down: each is refused with a plain reason and nothing is written.
### SC-006: Repeat
The same call twice gives the same answer, ties included.

## 6. Functional requirements
### 6.1 New
- FR-NEW-001: `search.chunks(mount_id, inputs, top_k, context_before, context_after)` and `search.images(mount_id, inputs, top_k)` SHALL take `inputs`, a list of 1 to 10 objects each holding exactly one of `text` (1 to 8000 characters after trimming) and `image` (the absolute path of an image file of the project: png, jpg, jpeg, gif, webp, bmp, at most 10 MiB); `top_k` 1 to 50 (default 10) is the number of lines per input; `context_before` and `context_after` 0 to 5 (default 0). Labels are assigned in order: `S1`, `S2` for texts, `I1`, `I2` for images, each counted among its kind.
- FR-NEW-002: WHEN `search.chunks` runs an input THE platform SHALL build a dense list (the 50 nearest chunk vectors of the project, by cosine distance) and, when the project mode is `both` and the engine serves full text, a keyword list (the 50 best full text hits for the first 32 distinct words of the input, letters and digits, lower case), fuse them by `1/(60 + rank)` summed over the lists, order by fused score then best rank then stable chunk id (path, then chunk number), keep one line per chunk (path, number), and answer the first `top_k`.
- FR-NEW-003: A line of `search.chunks` SHALL carry `document`, `chunk` (number), `text` (the chunk text without its header line), `heading_path` and `similarity`, the number `1 - cosine distance` between the input vector and the chunk vector clamped to 0 to 1, or null when the chunk has no vector. Groups are listed in input order with `label`, `kind` and `lines`.
- FR-NEW-004: WHEN an input is an image THE platform SHALL read the file, convert it with the converter resolved for the project (SPEC-0106), use the Markdown it answers, cut to 8000 characters, as the text of the input, and show it as `query_text` of its group.
- FR-NEW-005: WHEN `context_before` or `context_after` is above 0 THE platform SHALL add to each line the chunks of its default set around it, up to that count, each as an object marked `role: "context"` with `position` (`before` or `after`), `chunk`, `text` and its own `similarity`; a chunk that is itself a line of the same group, or already context of an earlier line of that group, SHALL NOT be repeated.
- FR-NEW-006: IF the project index mode is not `rag` or `both`, or the engine has no vectors, THEN both tools SHALL answer `ERR_NOT_SUPPORTED` `the index mode of the project has no vectors`; IF search is off, `ERR_NOT_SUPPORTED` as the other search tools.
- FR-NEW-007: Anyone who is not a member of the project SHALL get `ERR_FORBIDDEN`, platform administrators included; an image path that is not an existing image file SHALL be `ERR_NOT_FOUND` or `ERR_INVALID_ARGUMENT` with a plain reason.
- FR-NEW-008: IF an image input meets no converter THEN `ERR_NOT_SUPPORTED` `no document converter is set up, an image cannot be turned into text`; IF the converter fails or answers no text THEN its failure, or `ERR_INVALID_ARGUMENT` `the converter found no text in <path>`.
- FR-NEW-009: `search.images` SHALL embed each input with the image model of the project profile (an image as a `data:<mime>;base64,` string, a text as is), take the 50 nearest image vectors of the project at that width, order by similarity then stable id (path, then number), and answer the first `top_k` as lines `document`, `image` (number), `text` (caption and OCR stored beside the vector) and `similarity`, grouped by input as in FR-NEW-003.
- FR-NEW-010: IF the profile names no image model THEN `search.images` SHALL answer `ERR_NOT_SUPPORTED` `the project has no image model`; IF an embedding call fails or answers an unusable vector THEN the tool SHALL answer that failure as `ERR_INTERNAL_ERROR` and nothing is stored.
- FR-NEW-011: A call SHALL change nothing in any store (no vector, no job, no marker), and the same inputs on the same data SHALL give the same answer.
- FR-NEW-012: Every refusal of shape (FR-NEW-001) SHALL come before any embedding or conversion call.
### 6.2 Modified
None. `search.query` is unchanged (pinned by E2E-016).
### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
Per input: one conversion (image only), one embedding call, one dense read of at most 50 rows, one full text read of at most 50 rows, one similarity read of the output entries. Inputs run one after the other.
### 7.2 Security and access
Membership is checked first; the image is read as the member could read it with `fs.read_bytes`.
### 7.3 Privacy and compliance
A query vector is never returned or logged; converted text is shown to the caller who sent the image only.
### 7.4 Usability and accessibility
Refusal texts are plain sentences.
### 7.5 Reliability
A failing input fails the whole call; no partial answer.
### 7.6 Operability
No migration, no new setting.

## 8. Edge cases
- A chunk found by both lists appears once.
- A file built before SPEC-0101 has entries at chunk 0: one line per file, shown with chunk 0, no context.
- A model change without a recalculation: the dense list is empty at the new width; the keyword list still answers and the similarity is null (the vectors are out of date).
- A negative cosine reads 0.
- A read at a width with no store creates no table on either engine.
- A chunk of a file with no default set shows its stored text unchanged, as `search.query` does.
- Two lines with the same fused score, best rank and equal ranks across lists: path then chunk number.

## 9. Acceptance tests (the contract)
### 9.1 Summary
Eighteen tests, names `e2e_0109_NNN_slug`, in `crates/core/src/settings/search_e2e.rs` (tools, SQLite), `search/vector_store_e2e.rs` (engines) and `settings/profile_pg_e2e.rs` style PostgreSQL tests, plus unit tests of the fusion, the keyword query and the similarity clamp. Run: `cargo test -p fsa-core --lib --features rag -- e2e_0109_`. Driver: the real `McpServer` tool methods on the SPEC-0108 `World` fixture, a fake embedding service whose vector follows the words of the input (so similarities and ranks are known), a stub converter; PostgreSQL ones run with `MCPFS_TEST_PG_DSN` (pgvector) and skip without it.

### 9.2 New tests
Happy (4): E2E-001 one text input (SC-001, FR-NEW-001, 002, 003); E2E-002 two texts and an image grouped S1, S2, I1 (SC-002, FR-NEW-001, 004); E2E-003 context before and after (SC-003, FR-NEW-005); E2E-004 search images by image and by text (SC-004, FR-NEW-009).
Failures (5): E2E-005 invalid arguments of both tools (FR-NEW-001, 012); E2E-006 missing converter, failing converter, empty conversion, no image model, service down (FR-NEW-008, 010); E2E-007 mode `none`, search off (FR-NEW-006); E2E-008 outsider and administrator without membership (FR-NEW-007); E2E-009 image path missing, a folder, not an image, over the size (FR-NEW-007).
Extra (6, plus the regression and two PostgreSQL tests): E2E-010 ties are deterministic, 20 identical calls (SC-006, FR-NEW-002, 011); E2E-011 one line per chunk when both lists find it (FR-NEW-002); E2E-012 keyword list alone when the dense list is empty, similarity null (FR-NEW-002, 003); E2E-013 every similarity in 0 to 1 and the clamp (FR-NEW-003); E2E-014 context never repeats a line or another context (FR-NEW-005); E2E-015 project mode `bm25` refused, mode `rag` uses the dense list only (FR-NEW-002, 006); E2E-016 `search.query` answer byte identical to the recorded one (3.2); E2E-017 engines agree: SQLite and PostgreSQL nearest and similarity on the same vectors (FR-NEW-003, 009); E2E-018 both tools on PostgreSQL, two widths (FR-NEW-002, 009).
### 9.3 Exploit tests
None: no new injection surface; the keyword query keeps letters and digits only (unit test), table names are built from an integer.
### 9.4 Modified tests
The count of `search.*` tools (4 to 6) and of all listed tools (156 to 158) in `mcp/server.rs`; the golden contract is unchanged.
### 9.5 Removed tests
None.

## 10. Traceability
| Requirement | Tests |
|---|---|
| FR-NEW-001 | E2E-001, 002, 005 |
| FR-NEW-002 | E2E-001, 010, 011, 012, 015, 018 |
| FR-NEW-003 | E2E-001, 012, 013, 017 |
| FR-NEW-004 | E2E-002, 006 |
| FR-NEW-005 | E2E-003, 014 |
| FR-NEW-006 | E2E-007, 015 |
| FR-NEW-007 | E2E-008, 009 |
| FR-NEW-008 | E2E-006 |
| FR-NEW-009 | E2E-004, 017, 018 |
| FR-NEW-010 | E2E-006 |
| FR-NEW-011 | E2E-010 |
| FR-NEW-012 | E2E-005 |
| Modified: none | E2E-016 |

## 11. Open questions
None. Assumptions (S-03, S-05): an image input is a path of the project (a tool cannot receive bytes through this contract without a size rule of its own); the converter's Markdown stands for caption plus OCR; a text sent to `search.images` goes to the image model as is, a model that cannot embed text answers an error that is returned (AUTO-011, AUTO-012).

## 12. Glossary (MANDATORY)
| Term | Meaning |
|---|---|
| Input | one text or one image of a call, labeled S1.. or I1.. |
| Line | one result: input, document, chunk or image, similarity |
| Context | a neighbour chunk of a line, marked as such, with its own similarity |
| Dense list | chunks ordered by vector distance to the input |
| Keyword list | chunks ordered by full text score for the words of the input |
| Stable chunk id | the pair (path, chunk number), compared path first |

## 13. Implementability gate (MANDATORY)
Every requirement has a test; every test names scenario and requirement; no open question; 12 requirements and 18 tests are inside the depth M budget. Pass.

## 14. Evidence appendix
### 14.1 Current State evidence
| Row | Evidence |
|---|---|
| 1 | `tools/search_semantic.rs` `query`, `search/fusion.rs` `rrf_merge` (per path) |
| 2 | `search/mod.rs` `SearchBackend` (no read by vector), `tools/vectors.rs` |
| 3 | `rg "byte" crates/core/src/search/e2e.rs` finds no pinned answer of `search.query` |
### 14.2 Class sweep
Engines implementing `SearchBackend`: `bm25_sqlite`, `bm25_pg`, `vector_sqlite`, `vector_pg`, `CombinedBackend`, `ScheduledBackend`; each new method is delegated by the two wrappers.
### 14.3 Design inputs
Reuse `EmbedTargets::target_for` and `image_target_for`, `check_vector`, `vectors_of` naming, `chunk_store().get_chunk_window`, `state.doc_service_for`.
