---
Id: BL-0171
Title: Chunk and image vectors in PostgreSQL pgvector, a vector read tool and a full recalculation API
Nature: FEAT
Source: owner roadmap of 2026-11-19
Created: 2026-11-19
---

## What
Compute and store the vector of every chunk and every image with the project's models (BL-0170), in PostgreSQL pgvector (SQLite keeps sqlite-vec for local runs). A tool returns the vectors of a document. An admin call recalculates every vector of a project, or of one document, after a model change or an incorrect result, and reports its progress.

## Why
Vectors must be reproducible and replaceable when a model changes, which the current wipe and full index transition does silently.

## Evidence
- Vector backends exist for both engines: `crates/core/src/search/vector_pg.rs` and `vector_sqlite.rs`; a mode change wipes and reindexes (`.agent_docs/search.md`, "Transitions").
- The PostgreSQL search backend is wired (`crates/core/src/search/mod.rs:146-182`) although `.agent_docs/search.md` still says it is future.

## Notes
- Depends on BL-0169 and BL-0170.
- The recalculation is a long running job: start, status, cancel.
- Vectors are not file metadata: they live in the search index. Chunks (BL-0168) are the metadata they are computed from.