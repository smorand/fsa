---
Id: BL-0172
Title: Search by text and by image: three tools, several inputs, document reconciliation
Nature: FEAT
Source: owner roadmap of 2026-11-19; scope set by the owner on 2026-11-20; folds in BL-0036
Created: 2026-11-19
---

## What
Three search tools over the stored chunks and images, each taking one or several labeled inputs (S1, S2, I1):
- **Search chunks** with texts and/or images. An image is converted (caption and OCR from the converter) and used as text. A hit line is (input, document, chunk, similarity = 1 minus cosine distance), grouped by input. Optional chunks before and after, marked as context with their own similarity.
- **Search images** with images, on the image vectors; a text input is allowed when the project's image model embeds text and images in one space.
- **Search documents** with texts and/or images: documents ranked by a reconciliation score, with the best chunk per input as proof. Per input q: `S_q = m + b(1 - m)(1 - product(1 - s_i))`, `m` the best similarity, `s_i` the other chunks above a floor f (default 0.5), b default 0.5; the document score is the weighted mean of `S_q` (equal weights, a missed input scores 0). f, b and the weights are parameters.
- Ordering follows paragraph-v3a section 7: a dense leg and a keyword leg, 50 candidates each, `1/(60 + rank)` fusion, ties by best rank then a stable chunk id, one chunk per unit.

## Why
One question must find one complete document even when the idea appears many times in text and once in an image. Scoring by the best passage (MaxP) is the robust baseline; summing weak matches would let four poor chunks beat one very close one.

## Evidence
- Today `search.query` returns `{path, score, chunk, rank}` over character windows (`crates/core/src/tools/search_semantic.rs:174-188`).
- Passage score aggregation: FirstP, MaxP and SumP (Dai and Callan, https://link.springer.com/chapter/10.1007/978-3-030-72240-1_11).

## Notes
- Depends on BL-0168 (chunks), BL-0166 (captions and OCR), BL-0171 (vectors of text and images).
