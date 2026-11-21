---
Id: BL-0175
Title: Web screen to follow the metadata of a file and recalculate it
Nature: FEAT
Source: owner request of 2026-11-20
Created: 2026-11-20
---

## What
A browser screen, next to the existing token, trash and deleted projects screens, shows for a file its document text, images with caption and OCR, tables, and chunk sets with their strategy and parameters, and lets the user step through chunks and trigger a recalculation (BL-0174).

## Why
Chunks and the other metadata are meant to be followed, tested and debugged by a person, not only read by a tool.

## Evidence
- Three screens exist today (`crates/core/src/token_screen.rs`, `trash_screen.rs`, `deleted_projects_screen.rs`); none shows file metadata.

## Notes
- Depends on BL-0166, BL-0167, BL-0168 and BL-0174. Same session cookie and CSRF convention as the existing screens.
