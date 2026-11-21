---
Id: BL-0176
Title: A converted document is rebuilt after its source changes, with the settle delay
Nature: FEAT
Source: cut from BL-0174 by the owner on 2026-11-20 (SPEC-0100 gate)
Created: 2026-11-20
---

## What
Add the kind `conversion` to the rebuild mechanism of BL-0174: when a source document that was converted is replaced, its companion Markdown, images and tables are rebuilt after the settle delay, and can be rebuilt by hand. Open points for its spec: how to tell a companion made by conversion from a hand-written `.md` at the same path (a recorded origin), what an upload with the conversion flag or `fs.documentize` does to a waiting rebuild, who is charged the quota of a background conversion, and what a companion edited by hand becomes.

## Why
The converted metadata (document, images, tables, then chunks) must follow its source file, as asked for conversion by CLI or API.

## Evidence
- A companion path replaces the extension, so `/a.pdf` and `/a.docx` share `/a.md` (`crates/core/src/docs/extract.rs:488-497`).
- An upload with the conversion flag converts inside the request and charges quota (`crates/core/src/core/fs_ops.rs:668,871-886`, `crates/core/src/tools/write.rs:119,150`).
- `land_conversion` already drops a conversion overtaken by a newer write (`fs_ops.rs:867`).

## Notes
- Depends on BL-0174. Needs BL-0165 and BL-0166 for the converter settings and metadata.
