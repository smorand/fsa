# Search documents with reconciliation: functional specification

> Id: SPEC-0110
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 3 areas (reconciliation formula, the search.documents tool, honest keyword mode), 10 requirements, no new persisted concept, no new dependency. Auto run, part 4 of 5 (BL-0178, the part 2/2 of BL-0172); part 5 (file metadata screen) is out of this spec.
> Status: implemented
> Generated on: 2026-11-26
> Project: existing
> From backlog: BL-0178
> Depends on: SPEC-0109
> Security: none
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
`search.chunks` answers chunks per input, but a question usually wants one complete document: the same idea may sit in many passages of a text and once in an image. This increment adds `search.documents`: it takes the same labeled inputs, finds for each input the chunks of the project with the engine of `search.chunks`, scores every document per input with a reconciliation formula (the best chunk, lifted a little by the other good chunks of the same document), averages the per input scores with weights, and answers the documents best first with, for each input, the best chunk as proof. It also makes the `mode_used` of `search.chunks` honest when its keyword list fails.

## 2. Current State (MANDATORY, every depth)
`search.query` ranks documents with one text and a fused path level score. `search.chunks` (SPEC-0109) takes 1 to 10 labeled inputs and answers, per input, up to 50 chunk lines with a similarity (1 minus cosine distance clamped to 0 to 1, null for a chunk without a vector at the width in force), from a dense list and a keyword list of 50 candidates each fused by `1/(60 + rank)`. Nothing combines the chunk similarities of one document or several inputs into one document score. When the keyword list of `search.chunks` fails, the failure is logged and the call goes on with the dense list only, but the answer still says `mode_used: both`.

### 2.1 What users can do today
- Read chunk lines per input, and group them by hand (14.1 row 1).
- Not rank documents over several inputs (14.1 row 1).

### 2.2 Specs governing this area
- SPEC-0109: the chunk engine (inputs, image to text, dense and keyword lists, fusion, similarity). Reused unchanged except 6.2.
- SPEC-0108, SPEC-0107, SPEC-0101, SPEC-0106: vectors, profile, chunks, converter, through SPEC-0109.

### 2.3 What is verified today
`search.chunks` by `e2e_0109_` tests; no test pins the `mode_used` of a call whose keyword list failed (14.1 row 2).

## 3. Scope
### 3.1 In scope
- `search.documents`, MCP only, config gated like the other `search.*` tools.
- The reconciliation formula and its parameters (floor, boost, weights).
- Reuse of the chunk pool of `search.chunks` (one implementation).
- A truthful `mode_used` and a warning on `search.chunks` and `search.documents` when the keyword list failed.
- PostgreSQL tests.
### 3.2 Out of scope, and why
- A REST route or a screen: not asked (YAGNI).
- A new candidate depth: the pool is the 100 chunks the fusion of `search.chunks` keeps at most (50 dense, 50 keyword); a document whose chunks are all outside it is not seen. Documented, not changed.
- Reranking: not in the approved ordering.
- `search.query` (unchanged).
### 3.3 Constraints imposed by the context
- `tool-contract-golden.json` holds the 152 frozen tools; the seven `search.*` tools are config gated and outside it (AUTO-015). The golden is regenerated and shows no diff.
- Vector features compile with `--features rag` (AUTO-009); the default build refuses the tool at call time with `ERR_NOT_SUPPORTED` as `search.chunks`.
- No secret in any answer or log; a query vector is never returned.

## 4. Actors
| Actor | Role |
|---|---|
| Member | searches a project; image inputs are files it can read |
| Outsider and platform administrator without membership | refused |

## 4.5 Bounded contexts
Search (engines, fusion, reconciliation), chunking (text of the proof chunk), converters (image to text).

## 5. Usage scenarios
### SC-001: Search one text, get documents
Alice sends one text; she gets documents best first, each with its score and, as proof, its best chunk.
### SC-002: Search with several inputs
Alice sends two texts and a picture of a chart; a document that answers all three beats one that answers one; a document that misses an input scores 0 for it.
### SC-003: Tune the reconciliation
Alice raises the floor, changes the boost, or weights the picture twice as much; the ranking follows.
### SC-004: Check a score by hand
Alice reads the per input score and the number of chunks counted; the same chunks seen by `search.chunks` give the same number by hand.
### SC-005: Refusals
Parameters out of range, weights that do not match the inputs, an empty input list, a missing converter, a model service that is down, a project without vectors, search off, an outsider: each refused with a plain reason, nothing written.
### SC-006: Repeat and concurrency
The same call twice, or several at once, gives the same answer, ties included.

## 6. Functional requirements
### 6.1 New
- FR-NEW-001: `search.documents(mount_id, inputs, top_k, floor, boost, weights)` SHALL take `inputs` exactly as `search.chunks` (1 to 10 objects, exactly one of `text` and `image`, labels `S1`.. `I1`..); `top_k` 1 to 50 (default 10) is the number of documents answered; `floor` is a number from 0 to 1 (default 0.5); `boost` is a number from 0 to 1 (default 0.5); `weights` is an optional list of numbers, one per input in the order sent.
- FR-NEW-002: For an input q and a document, THE platform SHALL take the chunks of that document found for q (the fused pool of `search.chunks` for q, at most 100 chunks) that have a similarity (FR-NEW-008), let `m` be the highest similarity (ties: lowest chunk number is the best chunk), and let `s_i` be the similarities of every other of those chunks that are at or above `floor` (a chunk exactly at the floor counts). The input score is `S_q = m + boost * (1 - m) * (1 - product(1 - s_i))`; with no `s_i` it is `m`; a document with no chunk for q has `S_q = 0`.
- FR-NEW-003: The document score SHALL be the weighted mean `sum(w_q * S_q) / sum(w_q)` over the inputs; without `weights` every weight is 1.
- FR-NEW-004: Each answered document SHALL carry `document` (path), `score`, and `inputs`, one object per input in the order sent: `label`, `score` (`S_q`), `counted` (number of chunks that entered the formula, 0 when missed) and `best` (the best chunk of that document for that input: `chunk`, `text`, `heading_path`, `similarity`, or null when missed). Numbers are printed with six decimals. The answer carries `mode_used`, `floor`, `boost` and `weights` (the weights as used, before normalisation), and `warning` when FR-NEW-010 applies. A document whose score is 0 at six decimals is not answered.
- FR-NEW-005: Documents SHALL be ordered by score (compared at six decimals, as printed) descending, then by path ascending; the first `top_k` are answered.
- FR-NEW-006: BEFORE any conversion or embedding call, `floor` or `boost` outside 0 to 1 or not a number, `weights` whose length differs from the number of inputs, a weight negative or not a number, weights summing to 0, and every refusal of shape of `search.chunks` (no input, more than 10, an input with both or neither of text and image, a text empty or over 8000 characters, `top_k` out of 1 to 50) SHALL be `ERR_INVALID_ARGUMENT` with a plain reason.
- FR-NEW-007: Membership, search off, a project mode without vectors, image path and converter refusals, and a failing embedding call SHALL be answered exactly as `search.chunks` answers them (SPEC-0109 FR-NEW-006 to 008, 012).
- FR-NEW-008: A chunk with no similarity (no vector at the width in force) SHALL NOT be counted, neither as `m` nor as an `s_i`; a document with no counted chunk for q has `S_q = 0` for it.
- FR-NEW-009: A call SHALL change nothing in any store, and the same inputs on the same data SHALL give the same answer, a ranking tie included.
### 6.2 Modified
- FR-MOD-010 (of SPEC-0109 FR-NEW-002): WHEN the keyword list of any input of `search.chunks` or `search.documents` fails, THE call SHALL still answer from the dense list, report `mode_used: rag` and carry `warning: "the keyword list failed, the answer holds the vector list only"`; when no keyword list failed, the answer is what it was and carries no `warning` key.
### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
Per input: what `search.chunks` costs without context (one conversion for an image, one embedding call, one dense read, one full text read, one similarity read of at most 100 entries) plus one window read per answered best chunk. Inputs run one after the other.
### 7.2 Security and access
Membership first; images read as the member could read them.
### 7.3 Privacy and compliance
A query vector is never returned or logged.
### 7.4 Usability and accessibility
Refusals are plain sentences.
### 7.5 Reliability
A failing input fails the whole call; no partial answer.
### 7.6 Operability
No migration, no setting.

## 8. Edge cases
- Rule of the floor: a chunk whose similarity is greater than or equal to `floor` is counted; with `floor = 1` only chunks at exactly 1 are counted beside `m`; with `floor = 0` every other chunk with a similarity counts.
- The best chunk is never counted twice: it is `m` and is excluded from the `s_i`; two chunks with the same highest similarity give one `m` and one `s_i` equal to it.
- With one input and `boost = 0`, `S_q = m`, the best passage score (MaxP): the order is the order of the best chunks. With the default boost the score is at least `m` and at most 1, so a document with many good chunks can pass one with a slightly better single chunk; that is the purpose.
- A single chunk per document gives `S_q = m` whatever `boost`.
- A file built before SPEC-0101 has one entry at chunk 0: one chunk, `S_q = m`.
- A model change without a recalculation: the dense list is empty at the new width, keyword hits have a null similarity, nothing is counted, the answer has no document (FR-NEW-008).
- Weights are relative: `[1,1]` and `[5,5]` give the same scores.
- Ties: equal printed score, path order.

## 9. Acceptance tests (the contract)
### 9.1 Summary
Fourteen end to end tests named `e2e_0110_NNN_slug` in `crates/core/src/settings/documents_e2e.rs` (tools, SQLite and one on PostgreSQL), plus unit tests of the formula in `search/reconcile.rs`. Run: `cargo test -p fsa-core --lib --features rag -- e2e_0110_ reconcile`. Driver: the real `McpServer` tool methods on the SPEC-0109 `zoo` fixture (keyed fake embedding service, stub converter); PostgreSQL tests run with `MCPFS_TEST_PG_DSN` (pgvector) and skip without it.

### 9.2 New tests
Happy (4): E2E-001 one text, documents with proof (SC-001, FR-NEW-001, 002, 004); E2E-002 two texts and an image, a missed input scores 0 (SC-002, FR-NEW-002, 003, 004); E2E-003 floor, boost and weights change the ranking (SC-003, FR-NEW-001, 003, 005); E2E-004 a score equals the formula on the chunk similarities of `search.chunks` for several chunks per document (SC-004, FR-NEW-002).
Failures (4): E2E-005 invalid parameters and weights, refused before any call (SC-005, FR-NEW-006); E2E-006 missing converter, failing converter, service down (SC-005, FR-NEW-007); E2E-007 mode `none`, search off, outsider, administrator without membership (SC-005, FR-NEW-007); E2E-008 repeat 20 times and 8 concurrent calls give one answer, no write (SC-006, FR-NEW-009).
Extra (5 plus the carried fix and PostgreSQL): E2E-009 floor edge at exactly the floor, floor 0 and 1 (FR-NEW-002); E2E-010 single input with boost 0 equals the MaxP order, a single chunk per document equals `m` for any boost (FR-NEW-002); E2E-011 stable ties by path (FR-NEW-005); E2E-012 a chunk without similarity is not counted (FR-NEW-008); E2E-013 the keyword list fails: `mode_used` rag and the warning on both tools, the same call without failure has no warning (FR-MOD-010); E2E-014 `search.documents` on PostgreSQL, two widths (FR-NEW-002, 009).
Unit: hand values (`m = 0.9` alone gives 0.9; `m = 0.9` with three chunks at 0.6 gives 0.9468; three chunks at 0.6 give 0.768), floor at 0.5 exactly, weights normalisation, parameter validation.
### 9.3 Exploit tests
None: no new injection surface (inputs go through the SPEC-0109 path).
### 9.4 Modified tests
The count of `search.*` tools (6 to 7) and of listed tools (158 to 159) in `tools/tool_surface_e2e.rs`; the golden contract is unchanged.
### 9.5 Removed tests
None.

## 10. Traceability
| Requirement | Tests |
|---|---|
| FR-NEW-001 | E2E-001, 003 |
| FR-NEW-002 | E2E-001, 002, 004, 009, 010, 014, unit |
| FR-NEW-003 | E2E-002, 003, unit |
| FR-NEW-004 | E2E-001, 002 |
| FR-NEW-005 | E2E-003, 011 |
| FR-NEW-006 | E2E-005 |
| FR-NEW-007 | E2E-006, 007 |
| FR-NEW-008 | E2E-012 |
| FR-NEW-009 | E2E-008, 014 |
| FR-MOD-010 | E2E-013 |

## 11. Open questions
None. Assumptions (S-03, S-05): the word "above" of the backlog is read as "at or above" (AUTO-017); the default `boost` name `boost` stands for the backlog's `b`; a document with total score 0 is not answered.

## 12. Glossary (MANDATORY)
| Term | Meaning |
|---|---|
| Pool | the fused chunks of one input (at most 100), as `search.chunks` builds them |
| Floor (f) | minimum similarity for a chunk other than the best to count |
| Boost (b) | how much of the gap `1 - m` the other good chunks may fill, 0 to 1 |
| `S_q` | the score of one document for one input |
| Proof | the best chunk of a document for an input |
| MaxP | the best passage score, here `S_q` with `boost = 0` |

## 13. Implementability gate (MANDATORY)
Round 1: every requirement has a test; every test names scenario and requirement; no open question; 10 requirements and 14 tests plus unit tests are inside the depth M budget. The single input equals MaxP claim of the backlog is exact only with `boost = 0` or one chunk per document; the spec states it (section 8) and E2E-010 pins both. Pass.

## 14. Evidence appendix
### 14.1 Current State evidence
| Row | Evidence |
|---|---|
| 1 | `tools/search_chunks.rs` `chunks`, `chunk_lines` (per input lines only) |
| 2 | `tools/search_chunks.rs` `chunk_lines`: `unwrap_or_else` logs the keyword failure, `chunks` returns `"mode_used": if keyword { "both" }` |
### 14.2 Class sweep
Callers of the keyword leg: `chunk_lines` only (`rg "query_bm25" crates/core/src/tools`: `search_semantic.rs` is `search.query`, unchanged by request).
### 14.3 Design inputs
Reuse `fuse_units`, `embed_input`, `nearest`, `similarities`, `default_set`, `get_chunk_window`, `prepare`, `text_of`.
