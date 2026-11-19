---
Id: BL-0162
Title: Extract concepts from documents against a concept list
Nature: FEAT
Source: owner request of 2026-11-19, during the document metadata and chunking roadmap
Created: 2026-11-19
Decision: owner asked for a dedicated entry only; the capability already exists in Science Next, so the first question of its spec is reuse versus rebuild
---

## What
Given a concept list, find which concepts each document (and each chunk) carries, and keep the answer as document metadata readable through the tools, the REST plane and the web app.

## Why
Concept tagging is done today in Science Next. FSA needs it next to the document, image, table and chunk metadata so a document is found and cited by concept, not only by words.

## Evidence
- `sdlc/backlog/BL-0034-chunk-summaries-and-concepts.md` already holds a per project concept dictionary idea (tag each chunk with concepts). This entry takes the concept part of it; BL-0034 keeps the summaries.
- No concept code exists in this repository: `rg -n -i 'concept' crates/core/src` finds none that extract.

## Notes
- Open questions for its spec: where the concept list lives (per project, shared, imported from Science Next); who extracts (the converter, a model call made by the server, or Science Next called as a service); whether a concept hit is a chunk tag, a document tag, or both; and how a changed list triggers a recalculation.
- Depends on the chunk records entry of the chunking roadmap when tags are kept per chunk.
