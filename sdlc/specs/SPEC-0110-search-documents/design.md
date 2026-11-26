# SPEC-0110: Search documents, design

> Spec: sdlc/specs/SPEC-0110-search-documents/spec.md
> Nature: FEAT
> Depth: M
> Security: none
> Designed on: 2026-11-26
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22).

## 2. Approaches
### Chosen: a pure formula module and the chunk pool of `search.chunks` extracted, one tool function beside the others
`search/reconcile.rs` holds the pure formula (`input_score`, `document_score`, parameter checks), unit tested with hand values. `tools/search_chunks.rs` keeps validation, conversion and the legs; its `chunk_lines` is split into `pool` (embed, dense list, keyword list, fusion, one similarity read, keyword failure flag) and the window and render steps, so `search.documents` calls the same `pool`. Pros: one engine (S-2B), the formula is testable with no database. Cons: a refactor of a function SPEC-0109 tests pin (its e2e tests are the safety net).
### Rejected: call `chunks()` and regroup its JSON
It cuts to `top_k` lines and renders context, so the pool would be capped by the line count and re parsed from JSON.
### Rejected: a second similarity read in a new engine method
`similarities` already answers any entries; the pool reads it once for the whole fused list.

## 3. Architecture
```
mcp/server.rs (#[tool] search.documents) -> tools/search_chunks.rs::documents
   prepare (shared) -> reconcile::check_params
   per input: text_of -> pool (embed_input, nearest, query_bm25, fuse_units, similarities)
   per document: reconcile::input_score(similarities >= f) ; reconcile::document_score(weights)
   answered documents: window (default set chunk text) for the best chunk of each input
```

## 4. Data model
None.

## 5. Contracts and interfaces
- `search.documents(mount_id, inputs[{text?|image?}], top_k?, floor?, boost?, weights?[])`.
- `reconcile::input_score(sims: &[f64], floor: f64, boost: f64) -> InputScore { score, counted, best_index }` (sims unordered, ties on the best by lowest index).
- `reconcile::document_score(scores: &[f64], weights: &[f64]) -> f64`; `reconcile::check_params(floor, boost, weights, inputs) -> Result<Vec<f64>>`.
- `Pool { vector, fused, stored, keyword_failed }` (the similarity read is the caller's: `chunk_lines` reads lines and context, `documents` the whole pool).

## 6. Decisions
- DEC-001 (S-2B): `search.documents` reuses the pool of `search.chunks`, one implementation. Obliged by FR-NEW-002.
- DEC-002 (AUTO-017, S-05): "above the floor" reads "at or above". A floor is a minimum. Obliged by FR-NEW-002.
- DEC-003 (S-05): the score is compared at six decimals, as printed, then by path. Obliged by FR-NEW-005.
- DEC-004 (S-10): the pool is at most 100 chunks, no deeper read. Obliged by FR-NEW-002.
- DEC-005 (S-05): a keyword failure degrades the answer to `rag` with a `warning`. Obliged by FR-MOD-010.

## 7. File structure impact
| Path | Action | Story |
|---|---|---|
| `search/reconcile.rs`, `search/mod.rs` | create and register: formula, parameter checks, unit tests | US-0330 |
| `tools/search_chunks.rs` | modify: `pool`, `window`, honest `mode_used` | US-0330 |
| `settings/documents_e2e.rs`, `settings/mod.rs` | create: tests with a failing keyword engine | US-0330, US-0331 |
| `tools/search_chunks.rs` (`documents`), `mcp/server.rs` | modify: the tool | US-0331 |
| `tools/tool_surface_e2e.rs`, PostgreSQL test, README, AGENTS, `.agent_docs`, `TOOL_CONTRACT.txt`, golden | modify | US-0332 |

## 8. Stories
| Story | Title | Tier | Depends |
|---|---|---|---|
| US-0330 | the reconciliation formula and an honest keyword mode | 1 | none |
| US-0331 | the search.documents tool | 1 | US-0330 |
| US-0332 | PostgreSQL, docs and contract (closes spec) | 2 | US-0331 |

## 9. Risks
The `chunk_lines` refactor changing `search.chunks` output (guarded by `e2e_0109_`); float ties; the failing keyword engine wrapper must delegate every read the pipeline uses.
