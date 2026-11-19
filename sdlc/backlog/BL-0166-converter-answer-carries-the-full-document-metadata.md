---
Id: BL-0166
Title: The converter answer carries the full document metadata, stored as metadata
Nature: FEAT
Source: owner roadmap of 2026-11-19
Created: 2026-11-19
Decision: the owner will test with new converter tools; the contract is the same for the program form (output folder) and the API form (JSON bundle)
---

## What
Define and ingest a manifest of the converter answer that holds, per document: the Markdown; every image with its caption, its OCR text, its page and its raw file; every table in Markdown and in CSV; and the heading structure of the document. All of it is stored as metadata of the document and is the source the read tools use (BL-0167). The `.md` file next to the source stays as a regular file.

## Why
Today only a caption and a page are kept per image, tables are rebuilt from pipe tables with an approximate CSV, and no structure or OCR text exists.

## Evidence
- Manifest today: `{"pictures": {"figures/f1.png": {"caption", "caption_failed", "page"}}}` (README, "Document artifacts"); no OCR field.
- Images and tables are kept as metadata rows, the Markdown as a file (`AGENTS.md`, `docs/artifacts/`).
- Pipe tables of the Markdown are kept with `CSV quality: approximate` (`TOOL_CONTRACT.txt`, `fs.list_tables` example).

## Notes
- Version the manifest so an old converter keeps working.
- Both forms (folder and bundle) carry the same fields.
- Decide in the spec what happens to a converter that sends less (missing OCR is an empty field, as for caption today).
