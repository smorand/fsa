---
Id: BL-0172
Title: Search reads the chunk records and its hits cite chunk, image and table ids
Nature: FEAT
Source: owner roadmap of 2026-11-19; folds in BL-0036
Created: 2026-11-19
---

## What
The BM25 and vector indexes are built from the chunk records (BL-0169) and from image captions, OCR text and tables. Each hit names the document and the chunk, image or table it comes from, in one ranked hybrid list.

## Why
Hits today point at a file and a text window, not at a chunk or an artifact.

## Evidence
- Indexing reads the whole file text (`crates/core/src/search/indexer.rs:156`, `read_utf8`) and chunks it inside the index.

## Notes
- Depends on BL-0171.
- Hybrid fusion stays as is (RRF, k 60); reranking stays optional.
