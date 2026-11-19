---
Id: BL-0169
Title: Chunk records and tools: list, get and create with parameters
Nature: FEAT
Source: owner roadmap of 2026-11-19; supersedes BL-0033
Created: 2026-11-19
---

## What
Keep the chunks of a document as records (id, position, text, strategy and parameters used, the images and tables inside it). Tools and REST routes list the chunks of a document, get one, and create them with a chosen strategy and parameters (BL-0168).

## Why
Today a chunk exists only inside the search index and cannot be listed, cited or reused.

## Evidence
- Chunks are cut and indexed inside `search.index`, with no list or get tool (`TOOL_CONTRACT.txt`, `search.index` with `chunk_size` and `chunk_overlap`).

## Notes
- Depends on BL-0166 (structure, tables, images) and BL-0168.
- Creating chunks again for a document replaces its previous set and its vectors (BL-0171).
- Chunks follow their document through file operations, like BL-0038 for artifacts.
