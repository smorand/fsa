---
Id: BL-0187
Title: File explorer in the browser: browse, upload, download, move, rename, delete, preview
Nature: FEAT
Source: owner request of 2026-11-29, gap analysis item 4
Created: 2026-11-29
---

## What
A project page lets a member browse folders, upload files (several at once, drag and drop, size limit shown), download a file or a folder as a zip, create a folder, rename, move, copy, delete to the trash, restore from the trash screen, preview text, Markdown, images and PDF, and search the project (names and content, with the search tools when enabled). Errors from quota, access or conflicts show as the design's alerts with the existing messages.

## Why
The operations exist as routes and tools but a person has no way to use them from the browser.

## Evidence
- Routes: `/api/fs/{mount_id}/list`, `mkdir`, `move`, `copy`, `delete`, `download`, `download-zip`, `write-bytes`, `tree`, `glob`, `grep`, `extract-text` (`api/dataplane.rs:154-206`).
- The trash and file metadata screens exist (`trash_screen.rs`, `file_metadata_screen.rs`).

## Notes
- Depends on BL-0186.
- It must call the existing `/api/fs` routes (a person's session as identity), never a second implementation.
- Open points for its spec: upload by multipart or by the existing write routes, large files, folder upload, concurrent edit conflict, keyboard use, how the file metadata page is reached from a file.
- Scripts on the pages come from the embedded assets, no CDN, no npm (S-25).
