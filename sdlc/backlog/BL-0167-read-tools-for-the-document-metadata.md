---
Id: BL-0167
Title: Read tools and routes for the document metadata: OCR, raw images, text view, structure
Nature: FEAT
Source: owner roadmap of 2026-11-19
Created: 2026-11-19
---

## What
Make the metadata of BL-0166 readable: OCR text next to each caption in the image list; the raw image file (PNG, JPEG and so on) as well as its base64 form; a text only view and a full view (images with caption and OCR, tables, structure); a tool that returns the heading structure; tables in Markdown and CSV with exact CSV when the converter supplied it. Each tool has its REST twin with the same payload.

## Why
The five tools of SPEC-0019 return captions and base64 only, and no tool returns the structure.

## Evidence
- `fs.list_images` returns `{id, caption, page}`; `fs.get_image` returns `base64`, `format`, `caption`, `page` (`TOOL_CONTRACT.txt`, lines 699 and 700). No raw download exists.
- `fs.get_document_view` returns the whole text with an image line and the tables in a chosen mode (`TOOL_CONTRACT.txt`, line 703).

## Notes
- Depends on BL-0166.
- Raw bytes are a REST download; the MCP tool keeps base64.
