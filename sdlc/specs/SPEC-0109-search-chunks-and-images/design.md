# SPEC-0109: Search chunks and search images, design

> Spec: sdlc/specs/SPEC-0109-search-chunks-and-images/spec.md
> Nature: FEAT
> Depth: M
> Security: none
> Designed on: 2026-11-26
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22): `rusqlite` with sqlite-vec (`vec_distance_cosine`), `sqlx` with pgvector (`<=>`), `base64`, all present.

## 2. Approaches
### Chosen: three engine reads on `SearchBackend`, the pipeline in one tool module
`embed_input(volume, kind, input) -> Vec<f32>` (project target, width in force, `check_vector`), `nearest(volume, kind, vector, limit) -> Vec<Near>` and `similarities(volume, kind, vector, entries) -> Vec<Option<f32>>`. Engines never cut text, never fuse. `search/fusion.rs` gains the pure `fuse_units`; `tools/search_chunks.rs` owns validation, conversion, legs, fusion, context and output for both tools. Pros: both engines share one contract, the pure parts are unit testable, `search.query` is untouched. Cons: three trait methods with a refusing default.
### Rejected: extend `query_vector` to return similarities
It returns path level `SearchResult`s with an engine specific score (SQLite `1 - d`, PostgreSQL `1 - d/2`), and `search.query` must stay byte identical (DEC-001).
### Rejected: compute cosine in Rust over `vectors_of` pages
Reads every vector of the project per query; the engines already index them.

## 3. Architecture
```
 mcp/server.rs (2 #[tool]) -> tools/search_chunks.rs (auth, shape, inputs)
   image -> VolumeClient.read_bytes -> state.doc_service_for -> text
   text  -> backend.embed_input -> backend.nearest (dense) | backend.query_bm25 (keyword, words only)
         -> fusion::fuse_units -> chunk store (text, heading, neighbours) -> backend.similarities
   images-> backend.embed_input(Image) -> backend.nearest(Image)
```

## 4. Data model
None.

## 5. Contracts and interfaces
- `search.chunks(mount_id, inputs[{text?|image?}], top_k?, context_before?, context_after?)`, `search.images(mount_id, inputs[], top_k?)`.
- `Near { path, number, text, similarity }`; `similarity_of_distance(d) = clamp(1 - d, 0, 1)`.
- `fusion::fuse_units(dense: &[(String, u32)], keyword: &[(String, u32)]) -> Vec<Fused { path, number, score, best_rank }>`.

## 6. Decisions
- DEC-001 (S-10): `search.query` and `rrf_merge` are not touched; E2E-016 pins them. Obliged by the byte identity request.
- DEC-002 (S-2B): the engine reads live on `SearchBackend` with a refusing default and are delegated by `ScheduledBackend` and `CombinedBackend`. Obliged by FR-NEW-002, 009.
- DEC-003 (S-05): the keyword leg searches the letters and digits of the input only (raw Tantivy syntax would fail on a colon or a quote of converted text). Obliged by FR-NEW-002.
- DEC-004 (S-05): similarity is computed in one pass for exactly the entries shown, so a keyword only line has one too; the dense list only orders. Obliged by FR-NEW-003.
- DEC-005 (AUTO-011): an image input is a project path; its text is the converter's Markdown (caption and OCR). Rejected: base64 bytes in the call, no size rule exists for a tool argument. Obliged by FR-NEW-004.
- DEC-006 (AUTO-012): negative cosine reads 0, a chunk without vector reads null. Obliged by FR-NEW-003.
- DEC-007 (AUTO-013): a text sent to `search.images` reaches the image model as is; whether the model embeds text is the administrator's choice at registration, a failing model is surfaced. Obliged by FR-NEW-009.
- DEC-008 (S-05): ties on the fused score order by best rank then (path, number). Obliged by FR-NEW-002.

## 7. File structure impact
| Path | Action | Story |
|---|---|---|
| `search/mod.rs`, `search/vector_sqlite.rs`, `search/vector_pg.rs`, `search/fusion.rs` | modify: three reads, clamp, `fuse_units` | US-0327 |
| `search/vector_store_e2e.rs` | modify: engine tests | US-0327 |
| `tools/search_chunks.rs`, `mcp/server.rs`, `tools/mod.rs`, `tools/all.rs` | create and modify: `search.chunks` | US-0328 |
| `settings/search_e2e.rs`, `settings/vectors_e2e.rs` | create and modify: tests, keyed stub | US-0328 |
| `tools/search_chunks.rs` (images), `settings/profile_pg_e2e.rs` | modify: `search.images`, PostgreSQL tests | US-0329 |
| README, AGENTS, `.agent_docs`, `TOOL_CONTRACT.txt`, golden | modify | US-0329 |

## 8. Stories
| Story | Title | Tier | Depends |
|---|---|---|---|
| US-0327 | engine reads by vector, fusion and clamp | 1 | none |
| US-0328 | the `search.chunks` tool: inputs, legs, context | 1 | US-0327 |
| US-0329 | `search.images`, the regression, PostgreSQL, docs and contract (closes spec) | 2 | US-0328 |

## 9. Risks
sqlite-vec `vec_distance_cosine` on a `vec0` column read; hnsw ordering ties on PostgreSQL (the order is re-sorted in Rust); tool count and golden assertions.
