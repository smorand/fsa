# US-0287: search reranks with the default reranker

> Spec: SPEC-0102 (sdlc/specs/SPEC-0102-platform-settings-and-model-registry/)
> Nature: FEAT
> Priority: P2
> Depends on: US-0286
> min_tier: 2
> Files touched: 5
> Closes spec: no

## Objective
Wire reranking into `search.query`: when the mode is `both`, reranking is asked for and a default reranker exists, the fused results go to the reranker and the answer is cut to the configured `top_n`; otherwise nothing is called, as today.

## Requirements covered
- SPEC-0102/FR-NEW-016: WHILE a server default reranker is set THE platform SHALL rerank search results with that model, through the endpoint, model name and authentication of its provider, instead of the reranking settings of the configuration.
- SPEC-0102/FR-NEW-017 (partial: the reranker part: no call without a default): WHILE no server default is set for a kind THE platform SHALL behave for that kind exactly as it did before the registry existed, using the search embedding and reranking settings of the configuration.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0102_040 e2e_0102_102 e2e_0102_107 e2e_0102_108 e2e_0102_114 search::`. A stub reranker records `POST /rerank` with `model`, `query`, `documents`, `top_n`; the tests ask 20 results with `top_n` 10, check the 10 returned in the stub order, the bearer header, an HTTP 503 answer keeping the order, and the absence of any call when no default exists or `rerank` is false.

## Design excerpt
### Decisions
- DEC-005 (SPEC-0102): A `SecretText` newtype (no `Display`, `Debug` prints `Secret(set)`) carries secrets in memory; answer structs have `has_secret` only; refusal texts are built from names and numbers, never from request bodies; the registry tools log nothing of their arguments. **Obliged by:** SPEC-0102/FR-NEW-004. **Rationale:** no output path can format a secret by accident. **Alternatives considered:** discipline only. **Source:** design. **Evidence:** `config.rs:31,51` (`Dsn` redaction precedent).
- DEC-007 (SPEC-0102): Search resolves the defaults at call time through `EmbedTargets` and `RerankTargets` (one admin query per index or query call, no cache); the YAML block is the embedding fallback; the vector engines take `Arc<dyn EmbedTargets>` instead of an `EmbeddingConfig`; the URL is the provider endpoint plus `/embeddings` or `/rerank`. **Obliged by:** SPEC-0102/FR-NEW-015, FR-NEW-016, FR-NEW-017. **Rationale:** "applies at once" forbids a cache or a boot snapshot. **Alternatives considered:** a TTL cache, restart on change, an `Arc<RwLock<EmbeddingConfig>>` refreshed on change (needs cross process invalidation). **Source:** existing code. **Evidence:** `search/mod.rs:271-351`, `vector_pg.rs:32,99,203`, `vector_sqlite.rs:35,112,271`.
- DEC-008 (SPEC-0102): `search_semantic::query` gains `rerank`; reranking runs only for mode `both`, `rerank` true and a default reranker, with no YAML fallback because nothing calls a configured reranker today; the documents are the fused results up to `top_k` and the answer is cut to the configured `top_n`. **Obliged by:** SPEC-0102/FR-NEW-016, FR-NEW-017. **Rationale:** the spec states these rules and says no reranker is called without a default. **Alternatives considered:** wiring the configured block too (proposal, not asked). **Source:** spec. **Evidence:** `tools/search_semantic.rs:119,267`, `mcp/server.rs:3318`, `search/rerank.rs:21-47`.
### Data model
No schema change.
### Contracts
`search::rerank::{RerankTarget{url, model, bearer}, RerankTargets}`; `rerank(client, &RerankTarget, query, results, top_n)`; `settings::RegistryTargets` also implements `RerankTargets` (default reranker joined with its provider, `url = endpoint + "/rerank"`); `search_semantic::query(state, person, mount, query, mode, top_k, rerank: bool)`; the MCP `search.query` passes its `rerank` argument (today discarded at `mcp/server.rs:3318`).
### Paths
| Path | Action |
|---|---|
| `crates/core/src/search/rerank.rs` | modify: target based call |
| `crates/core/src/settings/registry.rs` | modify: reranker target |
| `crates/core/src/tools/search_semantic.rs`, `mcp/server.rs` | modify: `rerank` wired |
| `crates/core/src/settings/e2e.rs` | modify: stub reranker and these tests |
### Existing patterns
`search/rerank.rs:21-47` (HTTP error keeps the order), `tools/search_semantic.rs:119-150` (mode dispatch and fusion), US-0286 `EmbedTargets` for the shape.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0102/E2E-040: the default reranker drives reranking
- Level / Driver / Location: functional e2e / search query handler, stub reranker / `crates/core/src/settings/e2e.rs` (`e2e_0102_040_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; project with search; stub reranker at `http://localhost:9011/v1`; provider and model `bge-reranker` set as the default reranker; the configuration has reranking off
- **When** the search mode is `both`; a search with reranking asks for 20 results
- **Then** the stub received the query with the model name `bge-reranker`, 20 documents and `top_n` 10; the answer holds 10 results in the stub's order
- **And** clearing the default reranker returns to the behavior without reranking (no call)
- **Priority:** High

### SPEC-0102/E2E-102: a registry without defaults changes nothing
- Level / Driver / Location: functional e2e / search query handler, stub reranker / `crates/core/src/settings/e2e.rs` (`e2e_0102_102_*`)
- **Given** a platform with providers and models but no default, configuration embedding on stub C and reranking on stub R
- **When** a document is indexed and a reranked search is made
- **Then** stub C received the calls and stub R none
- **And** the search results equal those of the platform without the registry
- **Priority:** Medium

### SPEC-0102/E2E-107: a bearer reranker
- Level / Driver / Location: functional e2e / search query handler, stub reranker / `crates/core/src/settings/e2e.rs` (`e2e_0102_107_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; provider `Rerank Cloud` (bearer, secret `rk-1`) with the default reranker; a stub requiring `Authorization: Bearer rk-1`
- **When** a reranked search is made (mode `both`)
- **Then** the stub accepted the call and the answer order follows its scores
- **And** the secret appears in no answer
- **Priority:** Medium

### SPEC-0102/E2E-108: a reranker that answers with an error
- Level / Driver / Location: functional e2e / search query handler, stub reranker / `crates/core/src/settings/e2e.rs` (`e2e_0102_108_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; the default reranker's service answers HTTP 503
- **When** a reranked search is made (mode `both`)
- **Then** the search answers with the results in their unreranked order and no error
- **And** the failure does not change any setting
- **Priority:** High

### SPEC-0102/E2E-114: reranking not asked for
- Level / Driver / Location: functional e2e / search query handler, stub reranker / `crates/core/src/settings/e2e.rs` (`e2e_0102_114_*`)
- **Given** a platform with Alice (`admin@example.com`) as platform administrator and Bob (`bob@example.com`) as an ordinary member, a master key configured, an empty registry; a default reranker is set; search mode `both`
- **When** a search is made with reranking turned off
- **Then** the stub reranker received no call
- **And** the results are in their unreranked order
- **Priority:** Medium

## Constraints
- No YAML fallback for the reranker.
- Reranking only for mode `both`.
- The answer length is at most the configured `top_n`.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/search.md` (reranking).
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no secret value in any output; no dash used as punctuation in code, comments or docs.

- [ ] One commit `US-0287: search reranks with the default reranker` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0102` and the requirement ids.
