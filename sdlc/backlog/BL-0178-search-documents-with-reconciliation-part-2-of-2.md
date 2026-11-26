---
Id: BL-0178
Title: Search by text and by image: search documents with reconciliation (part 2/2)
Nature: FEAT
Source: auto cut of BL-0172
Created: 2026-11-26
Depends on: BL-0172 (part 1/2: search.chunks and search.images, which supply the per input chunk hits and their similarities)
---

## What
One tool, **search documents**, with texts and/or images (labeled S1, S2, I1, same input rules as search chunks). Documents are ranked by a reconciliation score, with the best chunk per input as proof.
- Per input q: `S_q = m + b(1 - m)(1 - product(1 - s_i))`, `m` the best chunk similarity of the document for q, `s_i` the other chunks of the document above a floor f (default 0.5), b default 0.5.
- The document score is the weighted mean of the `S_q` (equal weights); an input the document does not match scores 0.
- f, b and the weights are parameters of the tool. A single input is the best passage score (MaxP).

## Why
One question must find one complete document even when the idea appears many times in text and once in an image. Scoring by the best passage (MaxP) is the robust baseline; summing weak matches would let four poor chunks beat one very close one.

## Approved scenarios and tests (copied from the C1 and C2 of the auto run, part 4 of 5)
- 4 happy scenarios, 4 failure scenarios (invalid input, missing dependency, concurrent or repeated action), 5 extra tests.
- Extra tests: formula hand values (m = 0.9 and three further chunks at 0.6), floor edge exactly 0.5, single input equals MaxP, stable ties, zero sum weights refused.
- The chunk hits come from the engine of BL-0172; the dense plus keyword legs, 50 candidates each, `1/(60 + rank)` fusion, ties by best rank then stable chunk id, one chunk per unit apply unchanged.

## Evidence
- Passage score aggregation: FirstP, MaxP and SumP (Dai and Callan, https://link.springer.com/chapter/10.1007/978-3-030-72240-1_11).

## Notes
- Depends on BL-0172 part 1/2, landed before this part starts.
