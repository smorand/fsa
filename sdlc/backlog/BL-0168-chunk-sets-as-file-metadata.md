---
Id: BL-0168
Title: Chunk sets as file metadata: fixed and structural strategies, tools to list, read, navigate and recalculate
Nature: FEAT
Source: owner roadmap and benchmark note of 2026-11-19; structural algorithm in /Users/sebastien/projects/perso/search-doc-corpora/specs/paragraph-v3a.md (version 2.0, 2026-11-20); parameters of the reference implementation /Users/sebastien/projects/perso/search-doc/src/search_doc/chunking/params.py; scope widened by the owner on 2026-11-20 (absorbs BL-0169)
Created: 2026-11-19
Decision: owner chose `fixed` and `structural` only (no per sentence embedding split); chunks are file metadata, vectors are not; approved split of 2026-11-20
---

## What
A file owns chunk sets, kept as metadata next to its document, text, images and tables. A set is made by one strategy with one parameter set; a file can hold several sets (several algorithms or settings). Tools list the sets of a file, list and read their chunks, return a chunk with N chunks before and M after (repeatable to widen the context), create a set with chosen parameters, and recalculate one. The search index reads the chunks; it no longer cuts files itself.
- `structural` is the default for every document (Markdown parsing, algorithm of paragraph-v3a): a heading cuts when at most `HEADING_CUT_DEPTH` (default 2, configurable) headings are open; small units merge up to a limit and big units split (defaults 2048 tokens, configurable); tables keep their header on every row group; lists are cut at a maximum of items (40); code blocks keep their fences; no overlap.
- `fixed`: size in tokens (default 2048) and an overlap in tokens, no sentence rule. It is the fallback for a document with no heading at all.
- Sizes are `cl100k_base` tokens, independent of the embedding model. A chunk keeps its text, heading path, order number, unit number, pages, token count and figure references.

## Why
The character window cuts words and tables and keeps nothing about structure (`crates/core/src/search/chunker.rs`). Chunks must be navigable (context expansion, tests, debugging) and visible in the UI, so they are metadata, not index internals.

## Evidence
- Character window 1000 and 100, chunked twice, once per backend: `crates/core/src/search/chunker.rs:11`, `crates/core/src/search/mod.rs:257-262`; nothing about headings or positions is stored (`bm25_sqlite.rs:123-135`).
- Benchmark (note of 2026-11): Fix2048 hybrid 78.5, structural 77.1, 80.2 with rerank; chunks near 250 tokens lose 10 points.
- Superseded behavior: SPEC-0008 FR-002, FR-012 and FR-020 (character windows, auto indexing 1000 and 100).

## Notes
- Recalculation after the file stops changing, and by hand, is BL-0174. Per project choice of strategy and models is BL-0170. The UI is BL-0175. Search over chunks is BL-0172; vectors are BL-0171.
- The spec states its deviation from the superseded FRs and the upgrade rule: existing indexed chunks are replaced when their file is next recalculated.
- Needs a tokenizer with its vocabulary shipped offline (a design input).
