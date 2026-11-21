---
Id: BL-0168
Title: Chunking engine with fixed size and structural strategies
Nature: FEAT
Source: owner roadmap and benchmark note of 2026-11-19 (rag-benchmark-chunking-retrieval-2026-11); structural algorithm specified in /Users/sebastien/projects/perso/search-doc-corpora/specs/paragraph-v3a.md (version 2.0, 2026-11-20)
Created: 2026-11-19
Decision: owner chose `fixed` and `structural` only; the per sentence embedding strategy is out
---

## What
An internal module that cuts a document into chunks with a strategy and its parameters.
- `fixed`: size in tokens (default 2048), overlap (default 10%), cut at the nearest sentence end.
- `structural`: cut at heading levels 1 and 2, merge neighbouring sections up to a target and a ceiling (default 2048 tokens), split an over long section at paragraph then sentence ends, keep a table whole when it fits and otherwise split it by rows with the header repeated, split lists in groups of 40 items with their introduction, keep a code block whole, never emit tiny chunks, and prefix each chunk with a context header "document title > heading path".
- A project default is `structural` with a fall back to `fixed` for a document with no headings.

## Why
The current chunker is a character window that cuts in words and tables (`crates/core/src/search/chunker.rs`). The benchmark shows size matters more than method, and that structural chunks cite and summarize better at equal retrieval score.

## Evidence
- `chunk(text, size, overlap)` over characters, 1000 and 100 by default (`.agent_docs/search.md`, "Chunking"; `crates/core/src/search/chunker.rs`).
- Benchmark: Fix2048 hybrid 78.5, structural (headings 1 and 2, up to 2048) 77.1, with rerank 80.2; chunks near 250 tokens lose 10 points; strict semantic split gave no gain.

## Notes
- The size unit is tokens: the spec fixes which count (a model independent estimate, since an embedding model's own tokenizer counts formulas and digits very differently).
- Pure function, no storage: BL-0169 stores and exposes the chunks.
- Authority for `structural`: `/Users/sebastien/projects/perso/search-doc-corpora/specs/paragraph-v3a.md`, sections 3 to 5 (parameters, size ruler, chunking algorithm) and 6.1 (indexed text with its contextual header). Where it differs from the lines above, the spec wins:
  - size ruler: `cl100k_base` BPE tokens, independent of the embedding model (spec 3.2);
  - no overlap between chunks (spec 1); the 10% overlap stays for `fixed` only;
  - a heading cuts a new section when at most 2 headings are open, not by `#` level, so a document starting at `###` still cuts (spec 5.2, `HEADING_CUT_DEPTH` 2);
  - `MERGE_LIMIT`, `UNIT_LIMIT` and `TABLE_LIMIT` 2048, `LIST_MAX_ITEMS` 40, `HEADER_LINES` 2 (spec 3.1);
  - figures are tracked as references (spec 5.6) and metadata tables are excluded from all chunks (spec 5.2).
- The retrieval part of that spec (section 7: hybrid, `RRF_K` 60, 50 candidates per leg, top 10) belongs to BL-0172, not to this entry.
